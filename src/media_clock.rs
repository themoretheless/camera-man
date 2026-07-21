use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use camera_man::{DeadlinePacer, monotonic_time_nanos};
use eframe::egui;

struct ClockControl {
    running: bool,
    fps: u32,
    revision: u64,
    stopping: bool,
}

struct SharedClock {
    control: Mutex<ClockControl>,
    wake: Condvar,
    generation: AtomicU64,
}

/// Autonomous media cadence. The worker wakes egui, but egui is only a
/// consumer of latest-only ticks and is no longer the stream's timer.
pub(crate) struct MediaClock {
    shared: Arc<SharedClock>,
    worker: Option<JoinHandle<()>>,
    consumed_generation: u64,
}

impl MediaClock {
    pub(crate) fn new(ctx: egui::Context, fps: u32) -> Self {
        let shared = Arc::new(SharedClock {
            control: Mutex::new(ClockControl {
                running: false,
                fps: fps.max(1),
                revision: 0,
                stopping: false,
            }),
            wake: Condvar::new(),
            generation: AtomicU64::new(0),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name(String::from("camera-man-media-clock"))
            .spawn(move || run_clock(worker_shared, ctx))
            .expect("failed to spawn media clock");

        Self {
            shared,
            worker: Some(worker),
            consumed_generation: 0,
        }
    }

    pub(crate) fn start(&mut self, fps: u32) {
        self.consumed_generation = self.shared.generation.load(Ordering::Acquire);
        self.update(true, fps);
    }

    pub(crate) fn stop(&mut self) {
        let fps = self
            .shared
            .control
            .lock()
            .expect("media clock mutex poisoned")
            .fps;
        self.update(false, fps);
        self.consumed_generation = self.shared.generation.load(Ordering::Acquire);
    }

    pub(crate) fn set_fps(&self, fps: u32) {
        let mut control = self
            .shared
            .control
            .lock()
            .expect("media clock mutex poisoned");
        let fps = fps.max(1);
        if control.fps == fps {
            return;
        }
        control.fps = fps;
        control.revision = control.revision.wrapping_add(1);
        self.shared.wake.notify_one();
    }

    /// Coalesces any number of overdue ticks into one render request. A slow
    /// UI can therefore skip stale work instead of building a frame backlog.
    pub(crate) fn take_tick(&mut self) -> bool {
        let generation = self.shared.generation.load(Ordering::Acquire);
        if generation == self.consumed_generation {
            return false;
        }
        self.consumed_generation = generation;
        true
    }

    fn update(&self, running: bool, fps: u32) {
        let mut control = self
            .shared
            .control
            .lock()
            .expect("media clock mutex poisoned");
        let fps = fps.max(1);
        if control.running == running && control.fps == fps {
            return;
        }
        control.running = running;
        control.fps = fps;
        control.revision = control.revision.wrapping_add(1);
        self.shared.wake.notify_one();
    }
}

impl Drop for MediaClock {
    fn drop(&mut self) {
        {
            let mut control = self
                .shared
                .control
                .lock()
                .expect("media clock mutex poisoned");
            control.stopping = true;
            control.running = false;
            control.revision = control.revision.wrapping_add(1);
            self.shared.wake.notify_one();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run_clock(shared: Arc<SharedClock>, ctx: egui::Context) {
    let mut pacer = DeadlinePacer::new(1);
    let mut pacing_revision = u64::MAX;
    let mut control = shared.control.lock().expect("media clock mutex poisoned");
    loop {
        while !control.running && !control.stopping {
            control = shared
                .wake
                .wait(control)
                .expect("media clock mutex poisoned");
        }
        if control.stopping {
            return;
        }

        let revision = control.revision;
        let fps = control.fps;
        if pacing_revision != revision {
            pacer = DeadlinePacer::new(fps);
            let _ = pacer.plan(monotonic_time_nanos(), fps);
            pacing_revision = revision;
        }
        let plan = pacer.plan(monotonic_time_nanos(), fps);
        let (next, timeout) = shared
            .wake
            .wait_timeout_while(control, Duration::from_nanos(plan.sleep_nanos), |control| {
                control.running && !control.stopping && control.revision == revision
            })
            .expect("media clock mutex poisoned");
        control = next;
        if control.stopping {
            return;
        }
        if timeout.timed_out() && control.running && control.revision == revision {
            shared.generation.fetch_add(1, Ordering::Release);
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn running_clock_produces_latest_only_ticks_and_stop_quiets_it() {
        let mut clock = MediaClock::new(egui::Context::default(), 120);
        clock.start(120);
        let deadline = Instant::now() + Duration::from_secs(1);
        while !clock.take_tick() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        assert!(
            Instant::now() < deadline,
            "media clock did not produce a tick"
        );
        assert!(!clock.take_tick(), "one consumer read must drain old ticks");

        clock.stop();
        thread::sleep(Duration::from_millis(20));
        assert!(!clock.take_tick(), "stopped clock must remain quiet");
    }
}
