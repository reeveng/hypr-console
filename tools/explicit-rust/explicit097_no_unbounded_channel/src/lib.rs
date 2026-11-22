//! EXPLICIT097: a queue says how much it holds.
//!
//! `mpsc::channel()` is a queue with no end. A `send` on it never waits and
//! never fails while the receiver lives, so whatever is on the sending side
//! sets the pace and the queue absorbs the difference -- in memory, for as
//! long as the difference lasts. Where the sender is a reader of something
//! outside the process, that is a bus, a socket, a device or a child's
//! output deciding how much this program holds, and a receiver that has
//! stalled is a heap that grows until something else notices. It is
//! EXPLICIT087's fault and 088's with a thread in the middle, and
//! EXPLICIT055's said of a buffer: the bound is not in the line because
//! nobody decided one.
//!
//! `sync_channel(n)` is the same channel with the decision made. A sender
//! that outruns its receiver waits, which is the pressure arriving where it
//! can do something -- the read slows, and whatever is writing meets a full
//! pipe instead of a patient queue -- or it is told with `try_send`, and the
//! site has to say what a full queue means: drop the oldest, drop this one,
//! or stop. That is a sentence every one of these channels has an answer to
//! and none of them has written down.
//!
//! Where the sender cannot outrun the reader -- one message for each thing a
//! person presses, a reply to a call that was made -- the site says so. The
//! unbounded spellings of the channel crates this tree does not use are
//! named beside the standard one, so the day one arrives the gate is there.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT097: no `mpsc::channel()`. A `sync_channel(n)` says what the
    /// queue holds, and the sender meets a full one.
    pub EXPLICIT097_NO_UNBOUNDED_CHANNEL,
    Warn,
    "a channel with no bound holds as much as its sender cares to put in it"
}

fn has_no_end(krate: &str, path: &str) -> bool {
    let last = path.rsplit("::").next().unwrap_or_default();

    match krate {
        "std" => path == "std::sync::mpsc::channel",
        "crossbeam_channel" | "flume" | "async_channel" => last == "unbounded",
        "tokio" | "futures_channel" => matches!(last, "unbounded_channel" | "unbounded"),
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit097NoUnboundedChannel {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };

        let ExprKind::Path(ref qpath) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = cx.qpath_res(qpath, called.hir_id) else {
            return;
        };

        let named = cx.tcx.def_path_str(id);

        if !has_no_end(cx.tcx.crate_name(id.krate).as_str(), named.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT097_NO_UNBOUNDED_CHANNEL,
            expr.span,
            format!("`{named}` makes a queue with no end, so its sender decides how much this program holds"),
            None,
            "make it with `sync_channel(MOST)`, the bound a `const` whose name says what it is the \
             most of, and decide at the `send` what a full queue means. Where the sender cannot \
             outrun the receiver, allow this rule here and let the reason say what paces it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
