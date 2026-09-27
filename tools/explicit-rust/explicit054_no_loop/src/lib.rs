#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, LoopSource};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT054: `loop` is forbidden. A `loop` runs until something inside
    /// it says `break`, so where it ends is wherever a `break` was written,
    /// and a reader finds out by reading all of it. EXPLICIT053 took the
    /// `while`, whose end is a condition failing; a bare `loop` has no end
    /// written at its head at all.
    ///
    /// What a loop walks is a list, an iterator or a source of events, and
    /// each of those already says where it stops: a `for` over the list, an
    /// iterator word -- `successors`, `from_fn`, `take_while`, `find_map`,
    /// `try_for_each` -- over the steps, and a `for` over a receiver or a
    /// reader's lines for a program that runs for as long as something is
    /// handing it events, which ends when that source does. `console-waiting`
    /// is where waiting for a thing is written, and it is written the same way.
    pub EXPLICIT054_NO_LOOP,
    Deny,
    "`loop` is forbidden; walk a list, an iterator or a source of events instead"
}

impl<'tcx> LateLintPass<'tcx> for Explicit054NoLoop {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Loop(_, _, LoopSource::Loop, _) = expr.kind else {
            return;
        };

        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT054_NO_LOOP,
            expr.span,
            "`loop` ends wherever a `break` was written, and says so nowhere at its head",
            None,
            "walk it with a `for` over the list or the source of events, or an iterator word that says where it stops",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
