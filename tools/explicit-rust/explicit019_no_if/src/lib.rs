#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT019: `if` is forbidden. An `if` without an `else` decides the
    /// false path by omission; an `else if` chain is a `match` that lost its
    /// scrutinee; an `if let` names one case and waves at the rest. A `match`
    /// is the one form that puts every outcome on the screen with a name on
    /// it: `match cond { true => …, false => … }` for a question,
    /// `match value { … }` for the cases an `if let` hides -- where
    /// EXPLICIT016 then asks that the variants be named too.
    ///
    /// `let … else` is not an `if` and is left alone: both of its outcomes are
    /// already written, and one of them is required to leave.
    pub EXPLICIT019_NO_IF,
    Warn,
    "`if` is forbidden; write a `match` that names both outcomes"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit019NoIf {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if !matches!(expr.kind, ExprKind::If(..)) {
            return;
        }

        // A `while` desugars to an `if` nobody wrote, and a macro's `if` is
        // the macro author's; what is linted is what somebody typed here.
        if expr.span.from_expansion() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT019_NO_IF,
            expr.span,
            "`if` leaves an outcome implicit: the path not taken has no name here",
            None,
            "write the decision as a `match` -- `match cond { true => …, false => … }`, or `match value { … }` for what an `if let` asks",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
