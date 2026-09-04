#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, MatchSource, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT017: `?` stands alone. `frame(settle(x)?, y)` hides an early
    /// return in the middle of a line; EXPLICIT013 asks that a block that
    /// decides something be visible in the shape of the function, and a buried
    /// `?` is a decision with no shape at all. `let settled = settle(x)?;`
    /// puts the exit where a reader scanning the left margin sees it.
    ///
    /// Allowed positions are the ones a scanning eye already reads as an exit:
    /// the whole of a statement, the right-hand side of a `let`, a `return`,
    /// the tail expression of a block, and the whole body of a `match` arm.
    pub EXPLICIT017_QUESTION_MARK_ALONE,
    Warn,
    "`?` buried in an expression hides an early return; lift it into a `let` of its own"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit017QuestionMarkAlone {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        // `?` lowers to a match the compiler marks as its own.
        let ExprKind::Match(_, _, MatchSource::TryDesugar(_)) = expr.kind else {
            return;
        };

        let stands_alone = match cx.tcx.parent_hir_node(expr.hir_id) {
            Node::Stmt(_) => true,
            Node::LetStmt(_) => true,
            Node::Block(_) => true,
            Node::Arm(_) => true,
            Node::Expr(parent) => matches!(parent.kind, ExprKind::Ret(_)),
            _ => false,
        };

        if stands_alone {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT017_QUESTION_MARK_ALONE,
            expr.span,
            "this `?` is buried in a larger expression: an early return with no shape on the screen",
            None,
            "lift it out -- `let name = …?;` -- so the exit sits on the left margin where it can be seen",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
