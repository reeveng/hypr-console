#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, LoopSource};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT053: `while` is forbidden. A `while` is an `if` at the top of a
    /// loop, and its false answer is the loop ending, which is written nowhere:
    /// the reader learns where the walk stops by working out when the
    /// condition fails. EXPLICIT019 took the `if` and EXPLICIT023 the `else`
    /// that names nothing; this is the same outcome without a name, moved to
    /// the head of a loop.
    ///
    /// What the loop walks is usually a list or an iterator, and then it is a
    /// `for`, or an iterator word -- `take_while`, `successors`, `from_fn` --
    /// that says what is walked and where it stops. A `loop` is not the
    /// answer either: EXPLICIT054 takes it, for the same reason. `while let`
    /// is the same question about a pattern, and the same answer: a `for`
    /// over what the pattern was taking apart.
    pub EXPLICIT053_NO_WHILE,
    Deny,
    "`while` is forbidden; a loop names where it stops"
}

impl<'tcx> LateLintPass<'tcx> for Explicit053NoWhile {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Loop(_, _, LoopSource::While, _) = expr.kind else {
            return;
        };

        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT053_NO_WHILE,
            expr.span,
            "`while` ends the loop on a false answer that has no name here",
            None,
            "walk it with a `for` over the list or the source of events, or an iterator word that says where it stops",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
