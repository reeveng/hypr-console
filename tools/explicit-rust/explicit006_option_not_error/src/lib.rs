#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT006: `Option` is for "may not exist", not for "went wrong".
    /// `.ok()` on a `Result` is the moment the two are confused: it takes an
    /// error that said what happened and returns a `None` that says nothing.
    /// A caller handed that `None` cannot tell an empty answer from a failure.
    pub EXPLICIT006_OPTION_NOT_ERROR,
    Deny,
    "`Result::ok()` throws away why it failed; `Option` is for absence, not for errors"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// `Result`, whatever it is called at the point of use. Asking the type rather
// than the spelling is what makes an alias answer the same as `std`'s own.
fn is_result(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        rustc_middle::ty::Adt(adt, _) => {
            cx.tcx.is_diagnostic_item(rustc_span::sym::Result, adt.did())
        }
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit006OptionNotError {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        let name = path.ident.name.as_str();
        if name != "ok" && name != "err" {
            return;
        }
        // Only when the receiver really is a `Result`. `.ok()` on anything
        // else is somebody's own method and none of this rule's business.
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_result(cx, recv_ty) {
            return;
        }
        let (what, help) = if name == "ok" {
            (
                "`Result::ok()` turns an error into `None`",
                "propagate the error with `?`, or `match` it; `Option` should mean the value may not exist",
            )
        } else {
            (
                "`Result::err()` throws away the value and keeps only the failure",
                "`match` the `Result` so both arms are written out",
            )
        };
        span_lint_and_help(cx, EXPLICIT006_OPTION_NOT_ERROR, expr.span, what, None, help);
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
