//! Panic containment for callbacks the platform calls into Rust.
//!
//! objc2 0.6.4 defines every generated class method as `extern "C-unwind"`, so
//! a panic in a CoreMediaIO callback does not abort at the boundary: it unwinds
//! into Objective-C frames that own no Rust cleanup and are not required to
//! handle a foreign unwind. objc2 offers no containment for this; its
//! `catch-all` feature only converts Objective-C exceptions raised by outgoing
//! message sends into Rust panics. The policy is contain, log, degrade; the
//! boundary-invariant register under `docs/` records it and the per-callback
//! defaults.

use std::any::Any;
use std::io::{self, Write};
use std::panic::{self, AssertUnwindSafe};
use std::process;

/// Runs `body`, returning `fallback()` if it panics.
///
/// `body` is wrapped in `AssertUnwindSafe` on every caller's behalf. There is
/// no alternative at this boundary: the platform calls the next callback
/// whether or not this one left its state consistent, so the repair is owed by
/// the caller, not provable by the type system. Any callback that mutates
/// shared state restores a definite state inside `fallback`.
///
/// `fallback` must not panic. It runs contained too, but no `T` can be
/// fabricated when it fails, so a second panic aborts instead of resuming the
/// unwind into Objective-C: an abort is a defined outcome, a foreign unwind is
/// not. Keep fallbacks minimal. Most are a constant; the rest call one
/// framework constructor whose failure means the process has already run out of
/// Objective-C objects.
pub fn contain_panic<T>(
    callback: &'static str,
    fallback: impl FnOnce() -> T,
    body: impl FnOnce() -> T,
) -> T {
    match panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(value) => value,
        Err(payload) => {
            report(callback, payload);
            match panic::catch_unwind(AssertUnwindSafe(fallback)) {
                Ok(value) => value,
                Err(payload) => {
                    report_line(&format!(
                        "CameraMan: the {callback} fallback panicked too: {}; aborting instead of unwinding into the platform",
                        panic_message(payload)
                    ));
                    process::abort()
                }
            }
        }
    }
}

/// `contain_panic` for callbacks that return nothing.
pub fn contain_panic_unit(callback: &'static str, body: impl FnOnce()) {
    contain_panic(callback, || (), body)
}

/// Readable form of a panic payload; `&'static str` and `String` are the only
/// payloads `panic!` produces, anything else is opaque by construction.
///
/// Takes the boxed payload `catch_unwind` and `JoinHandle::join` return by
/// value on purpose: `&Box<dyn Any + Send>` unsize-coerces to `&dyn Any` as the
/// box itself, which downcasts to neither payload type, so a by-reference
/// signature silently reports every panic as unknown.
pub fn panic_message(payload: Box<dyn Any + Send>) -> String {
    let payload: &(dyn Any + Send) = &*payload;
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        String::from("unknown panic payload")
    }
}

/// Writes one line to stderr for code that must not panic.
///
/// Deliberately not `eprintln!`, which panics when stderr is gone: on a
/// fallback path that panic would leave the boundary by the exact route this
/// module closes. Callers running inside a contained body can keep using
/// `eprintln!`.
pub fn report_line(message: &str) {
    let _ = writeln!(io::stderr(), "{message}");
}

fn report(callback: &'static str, payload: Box<dyn Any + Send>) {
    report_line(&format!(
        "CameraMan: {callback} panicked: {}; returning the safe default",
        panic_message(payload)
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::env;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};

    #[test]
    fn body_result_passes_through_and_the_fallback_stays_unused() {
        let fallback_ran = Cell::new(false);
        let value = contain_panic(
            "passthrough",
            || {
                fallback_ran.set(true);
                0
            },
            || 7,
        );

        assert_eq!(value, 7);
        assert!(!fallback_ran.get());
    }

    #[test]
    fn panicking_body_answers_with_the_fallback() {
        let value = contain_panic("exploding", || 3, || panic!("callback exploded"));

        assert_eq!(value, 3);
    }

    #[test]
    fn containment_leaves_no_state_behind_for_the_next_call() {
        assert!(!contain_panic(
            "first",
            || false,
            || panic!("first call exploded")
        ));
        assert!(contain_panic("second", || false, || true));
    }

    #[test]
    fn unit_callbacks_run_their_body_and_contain_its_panic() {
        let body_ran = Cell::new(false);
        contain_panic_unit("unit", || {
            body_ran.set(true);
            panic!("unit callback exploded");
        });

        assert!(body_ran.get());
    }

    #[test]
    fn payload_reporting_covers_both_standard_panic_types_and_neither() {
        let literal = panic::catch_unwind(|| panic!("literal payload")).unwrap_err();
        let owned =
            panic::catch_unwind(|| panic!("{}", String::from("owned payload"))).unwrap_err();
        let opaque = panic::catch_unwind(|| panic::panic_any(7_u8)).unwrap_err();

        assert_eq!(panic_message(literal), "literal payload");
        assert_eq!(panic_message(owned), "owned payload");
        assert_eq!(panic_message(opaque), "unknown panic payload");
    }

    /// The abort is only observable from outside the process, so this re-runs
    /// itself as a child with the guard set and inspects how the child died. A
    /// child that exits normally would mean the fallback's unwind escaped
    /// `contain_panic` and would have reached Objective-C.
    #[test]
    fn a_panicking_fallback_aborts_instead_of_resuming_the_unwind() {
        const CHILD_GUARD: &str = "CAMERAMAN_PANIC_BOUNDARY_ABORT_CHILD";
        const TEST_PATH: &str =
            "panic_boundary::tests::a_panicking_fallback_aborts_instead_of_resuming_the_unwind";

        if env::var_os(CHILD_GUARD).is_some() {
            let _: () = contain_panic(
                "double panic",
                || panic!("fallback exploded"),
                || panic!("body exploded"),
            );
            unreachable!("a panicking fallback must abort, not return");
        }

        let status = Command::new(env::current_exe().expect("test binary path"))
            .args(["--exact", TEST_PATH])
            .env(CHILD_GUARD, "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("re-running this test as a child process");

        assert_eq!(
            status.signal(),
            Some(libc::SIGABRT),
            "the child was expected to abort, it reported {status}"
        );
    }
}
