#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT010: a number must not change width without saying so. Rust has
    /// no implicit numeric coercion, so the widening happens behind a trait:
    /// `x.into()` reads as "the same number" and is a different type with a
    /// different range. EXPLICIT011 has the `as` casts; this has the quiet ones.
    pub EXPLICIT010_NO_NUMERIC_INTO,
    Deny,
    "numeric `into()` / `from()` hides a change of width; name the two types"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn is_numeric(ty: Ty<'_>) -> bool {
    matches!(ty.kind(), rustc_middle::ty::Int(_) | rustc_middle::ty::Uint(_) | rustc_middle::ty::Float(_))
}

impl<'tcx> LateLintPass<'tcx> for Explicit010NoNumericInto {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        if path.ident.name.as_str() != "into" {
            return;
        }
        let from = cx.typeck_results().expr_ty_adjusted(receiver);
        let to = cx.typeck_results().expr_ty(expr);
        // Both ends numeric, and not the same type: a width changed.
        if !is_numeric(from) || !is_numeric(to) || from == to {
            return;
        }
        span_lint_and_help(
            cx,
            EXPLICIT010_NO_NUMERIC_INTO,
            expr.span,
            format!("`into()` widens `{from}` to `{to}` without saying so at the call site"),
            None,
            "write the destination out -- `i64::from(x)` -- so the reader sees which two widths are in play",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
