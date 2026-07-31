use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacingPlan {
    pub sleep_nanos: u64,
    pub skipped_deadlines: u64,
    pub clock_discontinuity: bool,
}

/// Absolute-deadline frame pacer. Rendering time is not added to the next
/// period, so slow frames cause an explicit skip instead of permanent drift.
#[derive(Debug, Clone)]
pub struct DeadlinePacer {
    fps: u32,
    interval_nanos: u64,
    next_deadline_nanos: Option<u64>,
    last_now_nanos: Option<u64>,
}

impl DeadlinePacer {
    pub fn new(fps: u32) -> Self {
        let fps = fps.max(1);
        Self {
            fps,
            interval_nanos: interval_for_fps(fps),
            next_deadline_nanos: None,
            last_now_nanos: None,
        }
    }

    pub fn plan(&mut self, now_nanos: u64, fps: u32) -> PacingPlan {
        let fps = fps.max(1);
        let clock_discontinuity = self
            .last_now_nanos
            .is_some_and(|previous| now_nanos < previous);
        self.last_now_nanos = Some(now_nanos);
        if clock_discontinuity {
            self.next_deadline_nanos = Some(now_nanos);
        }
        if fps != self.fps {
            self.fps = fps;
            self.interval_nanos = interval_for_fps(fps);
            self.next_deadline_nanos = Some(now_nanos);
        }

        let mut deadline = self.next_deadline_nanos.unwrap_or(now_nanos);
        let mut skipped_deadlines = 0;
        if now_nanos > deadline {
            skipped_deadlines = now_nanos.saturating_sub(deadline) / self.interval_nanos;
            deadline =
                deadline.saturating_add(skipped_deadlines.saturating_mul(self.interval_nanos));
            if deadline < now_nanos {
                deadline = deadline.saturating_add(self.interval_nanos);
                skipped_deadlines = skipped_deadlines.saturating_add(1);
            }
        }

        self.next_deadline_nanos = Some(deadline.saturating_add(self.interval_nanos));
        PacingPlan {
            sleep_nanos: deadline.saturating_sub(now_nanos),
            skipped_deadlines,
            clock_discontinuity,
        }
    }

    pub const fn interval_nanos(&self) -> u64 {
        self.interval_nanos
    }
}

const fn interval_for_fps(fps: u32) -> u64 {
    let fps = if fps == 0 { 1 } else { fps };
    1_000_000_000 / fps as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Detached,
    Ready,
    Starting,
    Running,
    Stopping,
    Faulted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartAction {
    StartWorker,
    AlreadyRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopAction {
    StopWorker,
    AlreadyStopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleError {
    pub state: StreamState,
    pub operation: &'static str,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot {} stream while it is {:?}",
            self.operation, self.state
        )
    }
}

impl std::error::Error for LifecycleError {}

#[derive(Debug)]
pub struct StreamLifecycle {
    state: StreamState,
}

impl Default for StreamLifecycle {
    fn default() -> Self {
        Self {
            state: StreamState::Detached,
        }
    }
}

impl StreamLifecycle {
    pub const fn state(&self) -> StreamState {
        self.state
    }

    pub fn attach(&mut self) -> Result<(), LifecycleError> {
        match self.state {
            StreamState::Detached | StreamState::Ready => {
                self.state = StreamState::Ready;
                Ok(())
            }
            state => Err(LifecycleError {
                state,
                operation: "attach",
            }),
        }
    }

    pub fn begin_start(&mut self) -> Result<StartAction, LifecycleError> {
        match self.state {
            StreamState::Ready => {
                self.state = StreamState::Starting;
                Ok(StartAction::StartWorker)
            }
            StreamState::Starting | StreamState::Running => Ok(StartAction::AlreadyRunning),
            state => Err(LifecycleError {
                state,
                operation: "start",
            }),
        }
    }

    pub fn finish_start(&mut self, succeeded: bool) -> Result<(), LifecycleError> {
        if self.state != StreamState::Starting {
            return Err(LifecycleError {
                state: self.state,
                operation: "finish starting",
            });
        }
        self.state = if succeeded {
            StreamState::Running
        } else {
            StreamState::Faulted
        };
        Ok(())
    }

    pub fn begin_stop(&mut self) -> Result<StopAction, LifecycleError> {
        match self.state {
            StreamState::Running => {
                self.state = StreamState::Stopping;
                Ok(StopAction::StopWorker)
            }
            StreamState::Detached | StreamState::Ready | StreamState::Stopping => {
                Ok(StopAction::AlreadyStopped)
            }
            state => Err(LifecycleError {
                state,
                operation: "stop",
            }),
        }
    }

    pub fn finish_stop(&mut self) -> Result<(), LifecycleError> {
        if self.state != StreamState::Stopping {
            return Err(LifecycleError {
                state: self.state,
                operation: "finish stopping",
            });
        }
        self.state = StreamState::Ready;
        Ok(())
    }

    pub fn recover(&mut self) -> Result<(), LifecycleError> {
        if self.state != StreamState::Faulted {
            return Err(LifecycleError {
                state: self.state,
                operation: "recover",
            });
        }
        self.state = StreamState::Ready;
        Ok(())
    }

    /// Forces a definite state after a contained panic or a worker that ended
    /// on its own, and reports the state it interrupted. Unlike `recover`,
    /// every state is accepted: a panic can stop a transition halfway, and
    /// neither `begin_start` nor `begin_stop` can leave `Starting` or
    /// `Stopping` again. `Detached` is preserved because no stream is attached
    /// yet.
    pub fn force_ready(&mut self) -> StreamState {
        let interrupted = self.state;
        if interrupted != StreamState::Detached {
            self.state = StreamState::Ready;
        }
        interrupted
    }
}

/// Decides what a start callback must do, reaping a worker that ended without
/// a stop first.
///
/// The two halves of "the stream is running" are the lifecycle state and the
/// worker's run flag, and a worker that ended on its own (a contained panic, or
/// an early return when the stream retain or the pixel pool failed) leaves them
/// disagreeing: the lifecycle still says `Running`. The reap has to happen
/// before `begin_start`, which would otherwise answer `AlreadyRunning` for a
/// worker that is gone and wedge the stream until the client stops it. Both
/// steps live here so the order cannot be inverted at the call site. Returns
/// the state the dead worker left behind alongside the action.
pub fn begin_start_reaping_finished_worker(
    lifecycle: &mut StreamLifecycle,
    streaming: &AtomicBool,
    worker_finished: bool,
) -> Result<(StartAction, Option<StreamState>), LifecycleError> {
    let reaped = worker_finished.then(|| {
        streaming.store(false, Ordering::SeqCst);
        lifecycle.force_ready()
    });
    lifecycle.begin_start().map(|action| (action, reaped))
}

/// Restores a definite state after a panic contained inside a start or stop
/// callback: the worker is told to exit (the next start joins it) and the
/// half-finished transition the panic interrupted is reopened, which no
/// ordinary transition can leave. Returns the interrupted state.
pub fn reset_after_contained_panic(
    lifecycle: &mut StreamLifecycle,
    streaming: &AtomicBool,
) -> StreamState {
    streaming.store(false, Ordering::SeqCst);
    lifecycle.force_ready()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_pacer_does_not_accumulate_render_time() {
        let mut pacer = DeadlinePacer::new(10);
        assert_eq!(pacer.plan(1_000, 10).sleep_nanos, 0);
        assert_eq!(pacer.plan(50_001_000, 10).sleep_nanos, 50_000_000);
        assert_eq!(pacer.plan(100_001_000, 10).sleep_nanos, 100_000_000);
    }

    #[test]
    fn deadline_pacer_reports_missed_periods() {
        let mut pacer = DeadlinePacer::new(10);
        pacer.plan(0, 10);
        let plan = pacer.plan(350_000_000, 10);
        assert_eq!(plan.skipped_deadlines, 3);
        assert_eq!(plan.sleep_nanos, 50_000_000);
    }

    #[test]
    fn fps_change_resets_deadline_without_a_jump_backwards() {
        let mut pacer = DeadlinePacer::new(30);
        pacer.plan(10, 30);
        let plan = pacer.plan(20, 60);
        assert_eq!(plan.sleep_nanos, 0);
        assert_eq!(pacer.interval_nanos(), 16_666_666);
    }

    #[test]
    fn backward_clock_discontinuity_resets_the_deadline() {
        let mut pacer = DeadlinePacer::new(30);
        pacer.plan(1_000_000_000, 30);
        let plan = pacer.plan(100, 30);
        assert!(plan.clock_discontinuity);
        assert_eq!(plan.sleep_nanos, 0);
        assert_eq!(plan.skipped_deadlines, 0);
    }

    #[test]
    fn wake_after_long_sleep_skips_but_does_not_spin() {
        let mut pacer = DeadlinePacer::new(30);
        pacer.plan(0, 30);
        let plan = pacer.plan(60_000_000_000, 30);
        assert!(plan.skipped_deadlines >= 1_799);
        assert!(plan.sleep_nanos <= pacer.interval_nanos());
        assert!(!plan.clock_discontinuity);
    }

    #[test]
    fn repeated_client_start_and_stop_are_idempotent() {
        let mut lifecycle = StreamLifecycle::default();
        lifecycle.attach().unwrap();
        assert_eq!(lifecycle.begin_start().unwrap(), StartAction::StartWorker);
        lifecycle.finish_start(true).unwrap();
        assert_eq!(
            lifecycle.begin_start().unwrap(),
            StartAction::AlreadyRunning
        );
        assert_eq!(lifecycle.begin_stop().unwrap(), StopAction::StopWorker);
        lifecycle.finish_stop().unwrap();
        assert_eq!(lifecycle.begin_stop().unwrap(), StopAction::AlreadyStopped);
    }

    #[test]
    fn overlapping_client_callbacks_do_not_spawn_or_stop_twice() {
        let mut lifecycle = StreamLifecycle::default();
        lifecycle.attach().unwrap();

        assert_eq!(lifecycle.begin_start().unwrap(), StartAction::StartWorker);
        assert_eq!(
            lifecycle.begin_start().unwrap(),
            StartAction::AlreadyRunning
        );
        lifecycle.finish_start(true).unwrap();

        assert_eq!(lifecycle.begin_stop().unwrap(), StopAction::StopWorker);
        assert_eq!(lifecycle.begin_stop().unwrap(), StopAction::AlreadyStopped);
        lifecycle.finish_stop().unwrap();
    }

    #[test]
    fn impossible_transition_is_rejected() {
        let mut lifecycle = StreamLifecycle::default();
        let error = lifecycle.begin_start().unwrap_err();
        assert_eq!(error.state, StreamState::Detached);
    }

    /// The state field is private, so every fixture reaches its state through
    /// the same transitions a real stream would take.
    fn lifecycle_in(state: StreamState) -> StreamLifecycle {
        let mut lifecycle = StreamLifecycle::default();
        if state == StreamState::Detached {
            return lifecycle;
        }
        lifecycle.attach().unwrap();
        match state {
            StreamState::Ready => {}
            StreamState::Starting => {
                lifecycle.begin_start().unwrap();
            }
            StreamState::Faulted => {
                lifecycle.begin_start().unwrap();
                lifecycle.finish_start(false).unwrap();
            }
            StreamState::Running => {
                lifecycle.begin_start().unwrap();
                lifecycle.finish_start(true).unwrap();
            }
            StreamState::Stopping => {
                lifecycle.begin_start().unwrap();
                lifecycle.finish_start(true).unwrap();
                lifecycle.begin_stop().unwrap();
            }
            StreamState::Detached => unreachable!("handled before attaching"),
        }
        assert_eq!(lifecycle.state(), state);
        lifecycle
    }

    #[test]
    fn force_ready_reopens_every_half_finished_transition() {
        for state in [
            StreamState::Ready,
            StreamState::Starting,
            StreamState::Running,
            StreamState::Stopping,
            StreamState::Faulted,
        ] {
            let mut lifecycle = lifecycle_in(state);
            assert_eq!(lifecycle.force_ready(), state);
            assert_eq!(lifecycle.state(), StreamState::Ready);
            assert_eq!(lifecycle.begin_start().unwrap(), StartAction::StartWorker);
        }
    }

    #[test]
    fn force_ready_does_not_invent_an_attached_stream() {
        let mut lifecycle = lifecycle_in(StreamState::Detached);
        assert_eq!(lifecycle.force_ready(), StreamState::Detached);
        assert_eq!(lifecycle.state(), StreamState::Detached);
        assert!(lifecycle.begin_start().is_err());
    }

    #[test]
    fn a_start_interrupted_before_finish_start_wedges_the_stream_until_repair() {
        let mut lifecycle = lifecycle_in(StreamState::Starting);
        assert_eq!(
            lifecycle.begin_start().unwrap(),
            StartAction::AlreadyRunning
        );
        assert!(lifecycle.begin_stop().is_err());

        lifecycle.force_ready();
        assert_eq!(lifecycle.begin_start().unwrap(), StartAction::StartWorker);
        lifecycle.finish_start(true).unwrap();
        assert_eq!(lifecycle.begin_stop().unwrap(), StopAction::StopWorker);
        lifecycle.finish_stop().unwrap();
    }

    #[test]
    fn a_worker_that_ended_on_its_own_does_not_make_the_next_start_a_no_op() {
        let mut lifecycle = lifecycle_in(StreamState::Running);
        let streaming = AtomicBool::new(true);

        let (action, reaped) =
            begin_start_reaping_finished_worker(&mut lifecycle, &streaming, true).unwrap();

        assert_eq!(action, StartAction::StartWorker);
        assert_eq!(reaped, Some(StreamState::Running));
        assert!(!streaming.load(Ordering::SeqCst));
        lifecycle.finish_start(true).unwrap();
    }

    #[test]
    fn a_live_worker_is_not_reaped_and_the_start_stays_idempotent() {
        let mut lifecycle = lifecycle_in(StreamState::Running);
        let streaming = AtomicBool::new(true);

        let (action, reaped) =
            begin_start_reaping_finished_worker(&mut lifecycle, &streaming, false).unwrap();

        assert_eq!(action, StartAction::AlreadyRunning);
        assert_eq!(reaped, None);
        assert!(streaming.load(Ordering::SeqCst));
        assert_eq!(lifecycle.state(), StreamState::Running);
    }

    #[test]
    fn reaping_a_worker_that_never_started_leaves_the_detached_error_intact() {
        let mut lifecycle = lifecycle_in(StreamState::Detached);
        let streaming = AtomicBool::new(false);

        let error =
            begin_start_reaping_finished_worker(&mut lifecycle, &streaming, true).unwrap_err();

        assert_eq!(error.state, StreamState::Detached);
    }

    #[test]
    fn a_contained_panic_reset_clears_the_flag_and_reopens_the_transition() {
        for state in [StreamState::Starting, StreamState::Stopping] {
            let mut lifecycle = lifecycle_in(state);
            let streaming = AtomicBool::new(true);

            assert_eq!(
                reset_after_contained_panic(&mut lifecycle, &streaming),
                state
            );
            assert!(!streaming.load(Ordering::SeqCst));
            assert_eq!(lifecycle.begin_start().unwrap(), StartAction::StartWorker);
        }
    }
}
