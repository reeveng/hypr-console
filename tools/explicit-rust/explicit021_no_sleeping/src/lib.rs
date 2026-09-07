#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT021: a program waits for a thing, not for a number of seconds.
    /// `thread::sleep` and the timer callbacks name a duration where the thing
    /// being waited for has a name of its own -- a window that is drawn, a
    /// child that has exited, a socket with a byte on it, a channel with a
    /// message in it -- and almost all of those can be asked. A wait on the
    /// clock is right when it is too short on the machine it was written on
    /// and too long on every other one, which is a fault that arrives once the
    /// author is gone.
    ///
    /// The rule is not that a duration may never be waited out. It is that
    /// waiting one out is a decision, and a decision is written down: where
    /// the elapsing *is* the thing -- a frame held on the screen, a button held
    /// down long enough for somebody else to see it, a backoff between two
    /// attempts at something that is not ready -- the site carries the allow
    /// and its reason says which.
    ///
    /// A bounded wait on a real event is not caught, and is the shape this
    /// rule is pushing towards: `recv_timeout` sleeps until a message arrives
    /// or the patience runs out, and it is the message it returns. So is
    /// `console_waiting::until`, which asks and asks again and answers with
    /// whether the thing arrived -- where a sleep answers with nothing at all
    /// and leaves the caller assuming.
    pub EXPLICIT021_NO_SLEEPING,
    Deny,
    "waiting on the clock instead of on the thing; poll for what is actually being waited for"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Two families, read off the resolved path rather than off the spelling, so a
// `use` of either does not slip past. `thread::sleep` and its kin block this
// thread on nothing but the clock. The glib timers do the same to a main loop:
// the callback runs because time passed and not because anything happened.
//
// The glib half is asked for `glib::` anywhere in the path rather than at the
// front of it, because most of this tree reaches those functions through
// gtk4's re-export and the definition they resolve to is
// `gtk4::glib::timeout_add_local_once`.
fn sleeps(path: &str) -> Option<&'static str> {
    let blocking = [
        "std::thread::sleep",
        "std::thread::sleep_ms",
        "std::thread::sleep_until",
        "std::thread::park_timeout",
        "std::thread::park_timeout_ms",
    ];

    match blocking.contains(&path) {
        true => return Some("this thread stops until the clock says so, and nothing else can wake it"),
        false => {}
    }

    let timed = path.contains("glib::")
        && path
            .rsplit("::")
            .next()
            .is_some_and(|last| last.starts_with("timeout_add") || last.starts_with("timeout_future"));

    match timed {
        true => Some("this runs because time passed, not because the thing it is waiting for happened"),
        false => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit021NoSleeping {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        if expr.span.from_expansion() {
            return;
        }
        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };
        let ExprKind::Path(QPath::Resolved(_, path)) = called.kind else {
            return;
        };
        let Res::Def(DefKind::Fn, id) = path.res else {
            return;
        };
        let named = cx.tcx.def_path_str(id);
        let Some(harm) = sleeps(named.as_str()) else {
            return;
        };
        span_lint_and_help(
            cx,
            EXPLICIT021_NO_SLEEPING,
            expr.span,
            format!("`{named}` waits on the clock: {harm}"),
            None,
            "a duration here is an assumption about a machine, and it is checked on one machine and \
             relied on by every other. Ask instead, and keep asking: `console_waiting::until` polls for the \
             thing and says whether it arrived or the patience ran out. Where the elapsing is itself what is \
             being asked for, allow this rule here and let the reason say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
