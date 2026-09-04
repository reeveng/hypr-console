#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, FnRetTy};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT001: a function that can fail must say so in its return type.
    ///
    /// A lint cannot read "fallible" off a signature, so this reads it off the
    /// body instead: a function that swallows somebody else's error -- with
    /// `unwrap_or`, `unwrap_or_else`, `unwrap_or_default`, `ok`, `is_ok`,
    /// `is_err` -- is a function that has met a failure and decided not to
    /// mention it. If it returns `Result` that is a choice it is entitled to
    /// make. If it does not, the failure has nowhere to go and the caller
    /// cannot know there was one.
    pub EXPLICIT001_FALLIBLE_RESULT,
    Deny,
    "a function that swallows a failure must return `Result<T, E>`"
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

// The ways a `Result` is turned into a plain value without a word said. The
// panicking ones (`unwrap`, `expect`) belong to EXPLICIT004 and are left to it.
const SWALLOWS: &[&str] = &[
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "ok",
    "is_ok",
    "is_err",
];

impl<'tcx> LateLintPass<'tcx> for Explicit001FallibleResult {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        if !SWALLOWS.contains(&path.ident.name.as_str()) {
            return;
        }
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_result(cx, recv_ty) {
            return;
        }

        // The function this sits in, and what it promises to return.
        let owner = cx.tcx.hir_get_parent_item(expr.hir_id);
        let Some(decl) = cx.tcx.hir_fn_decl_by_hir_id(cx.tcx.local_def_id_to_hir_id(owner.def_id))
        else {
            return;
        };
        let says_so = match decl.output {
            FnRetTy::Return(ty) => {
                let hir_id = ty.hir_id;
                let _ = hir_id;
                // Read the promise off the written type: `Result<..>` however
                // it is spelled, including an alias whose name ends in Result.
                matches!(
                    ty.kind,
                    rustc_hir::TyKind::Path(rustc_hir::QPath::Resolved(_, p))
                        if p.segments.last().is_some_and(|s| s.ident.name.as_str().ends_with("Result"))
                )
            }
            FnRetTy::DefaultReturn(_) => false,
        };
        if says_so {
            return;
        }
        span_lint_and_help(
            cx,
            EXPLICIT001_FALLIBLE_RESULT,
            expr.span,
            format!(
                "`{}` swallows a failure in a function that does not return `Result`",
                path.ident.name
            ),
            None,
            "return `Result<T, E>` and propagate with `?`, so the caller is told there was a failure",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
