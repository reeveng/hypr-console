//! EXPLICIT057: a wait has an end, and the end is written at the wait.
//!
//! EXPLICIT021 took the waits that end on the clock -- a `thread::sleep`, a
//! timer callback -- and `console-waiting` is where the other kind lives: ask
//! a question until the answer changes, with a patience, and say which of the
//! two ways it ended. What neither reached is the wait that has no end at all:
//! a `recv` on a channel whose other end may have died silently, a `lock`
//! held by a thread that never comes back, a `join` on a task that hangs, a
//! `Condvar::wait` with nobody left to notify, a blocking read on a device
//! that has stopped answering. Each of them stops the thread for as long as
//! the thing it is waiting on takes, which on a machine that has gone wrong
//! is forever -- and a program stopped forever is a program somebody has to
//! hold a power button about.
//!
//! The spelling this rule asks for is the one that says the end: a deadline.
//! `recv_timeout` answers with the message or with the patience running out,
//! `try_lock` answers with the hold or the absence, `wait_timeout` the same,
//! and a blocking read is bounded by whoever opens what it reads, or carries
//! the allow whose reason says why that source is trusted to answer. The
//! retries over an unbounded wait want the same two things EXPLICIT022 asks
//! of every retry: a maximum number of attempts and a cap on the backoff, so
//! the end is arithmetic and not hope.
//!
//! This tree has almost no thread-blocking waits left -- the panels drive
//! their machines through actors and the actors through channels somebody is
//! always on the other end of -- which is why this rule can be written ahead
//! of the code rather than out of a fault. It arrives `Warn`, printing the
//! distance.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT057: `recv`, `lock`, `join`, `Condvar::wait` and the blocking
    /// reads wait for as long as the thing they are waiting on takes, which
    /// on a machine that has gone wrong is forever. Give each one a deadline,
    /// and every retry a maximum count and a backoff cap.
    pub EXPLICIT057_NO_UNBOUNDED_WAIT,
    Warn,
    "a wait with no deadline; say the end at the wait, the way `recv_timeout` and `try_lock` do"
}

// The resolved path, so a `use` does not slip past, and so a wait written in
// this tree's own words is not mistaken for the std calls that block. The
// last segment names the call; the path around it names whose call it is,
// and it is asked a segment at a time, from the crate down -- a `read` on a
// `Session` is not `std::io`'s because the letters `io` are in its name.
fn waits_without_end(path: &str) -> Option<&'static str> {
    let segments: Vec<&str> = path.split("::").collect();
    let last = segments.last().copied().unwrap_or_default();
    let under = |module: &[&str]| segments.starts_with(module);

    match last {
        "recv" if under(&["std", "sync", "mpsc"]) || under(&["crossbeam_channel"]) => Some(
            "`recv` waits for as long as the other end takes, and a dead sender is forever",
        ),
        "lock" if under(&["std", "sync"]) || under(&["parking_lot"]) => Some(
            "`lock` waits for as long as the holder takes, and a holder that never returns is forever",
        ),
        "join" if under(&["std", "thread"]) => Some(
            "`join` waits for as long as the task takes, and a task that hangs is forever",
        ),
        "wait" if under(&["std", "sync", "Condvar"]) => Some(
            "`Condvar::wait` waits for a notify that a machine gone wrong may never send",
        ),
        "read" | "read_exact" | "read_to_end" | "read_to_string" if under(&["std", "io"]) => Some(
            "a blocking read waits for a device that may have stopped answering",
        ),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit057NoUnboundedWait {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        let ExprKind::MethodCall(..) = expr.kind else {
            return;
        };

        let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
            return;
        };

        let Some(harm) = waits_without_end(cx.tcx.def_path_str(id).as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT057_NO_UNBOUNDED_WAIT,
            expr.span,
            harm,
            None,
            "give the wait a deadline -- `recv_timeout`, `try_lock`, `wait_timeout` -- and a retry \
             both a maximum count and a cap on the backoff, so the end is arithmetic rather than \
             hope. Where the source is trusted to answer, allow this rule here and let the reason \
             say who is on the other end",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
