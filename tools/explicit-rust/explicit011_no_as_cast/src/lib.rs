#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT011: `as` casts are forbidden. They silently truncate, widen,
    /// or reinterpret. Use `From` / `TryFrom` for value types, or
    /// `pointer::cast` / `&raw const` / `&raw mut` for pointers.
    pub EXPLICIT011_NO_AS_CAST,
    Deny,
    "`as` casts are forbidden; use `From`, `TryFrom`, or `pointer::cast`"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit011NoAsCast {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        if !matches!(expr.kind, ExprKind::Cast(..)) {
            return;
        }
        span_lint_and_help(
            cx,
            EXPLICIT011_NO_AS_CAST,
            expr.span,
            "`as` cast is forbidden",
            None,
            "use `From` / `TryFrom` for value types; `pointer::cast`, `&raw const`, `&raw mut` for pointers",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}