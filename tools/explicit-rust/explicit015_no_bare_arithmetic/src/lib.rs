#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_in_const_context;
use rustc_hir::{BinOpKind, Expr, ExprKind, UnOp};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT015: bare arithmetic on integers is forbidden. `+`, `-`, `*`
    /// and the shifts panic in a debug build and wrap in a release build --
    /// behaviour that differs by profile is the definition of implicit -- and
    /// `/` and `%` panic on zero in both. `checked_*`, `saturating_*` and
    /// `wrapping_*` each name a policy at the site, and what they return is a
    /// value the other rules make sure is met.
    ///
    /// Const contexts are left alone: arithmetic the compiler evaluates fails
    /// the build, which is a failure with a name, at the right time.
    pub EXPLICIT015_NO_BARE_ARITHMETIC,
    Warn,
    "bare integer arithmetic is forbidden; name the policy with `checked_*`, `saturating_*`, or `wrapping_*`"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The operators whose result is not always the number written. Comparison and
// the bitwise trio are total over their types and are left alone.
fn can_misbehave(op: BinOpKind) -> bool {
    matches!(
        op,
        BinOpKind::Add
            | BinOpKind::Sub
            | BinOpKind::Mul
            | BinOpKind::Div
            | BinOpKind::Rem
            | BinOpKind::Shl
            | BinOpKind::Shr
    )
}

fn is_integral(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    cx.typeck_results().expr_ty(expr).peel_refs().is_integral()
}

impl<'tcx> LateLintPass<'tcx> for Explicit015NoBareArithmetic {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        if is_in_const_context(cx) {
            return;
        }

        let offending = match expr.kind {
            ExprKind::Binary(op, lhs, _) => can_misbehave(op.node) && is_integral(cx, lhs),
            ExprKind::AssignOp(op, lhs, _) => {
                can_misbehave(op.node.into()) && is_integral(cx, lhs)
            }
            ExprKind::Unary(UnOp::Neg, operand) => is_integral(cx, operand),
            _ => false,
        };

        if !offending {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT015_NO_BARE_ARITHMETIC,
            expr.span,
            "bare integer arithmetic is an implicit panic in one profile and a silent wrap in the other",
            None,
            "name the policy: `checked_*`, `saturating_*`, or `wrapping_*`, and meet what comes back",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
