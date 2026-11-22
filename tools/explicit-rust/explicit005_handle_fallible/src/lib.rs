#![feature(rustc_private)]
#![warn(unused_extern_crates)]

#[path = "../../shared/infallible_discard.rs"]
mod infallible_discard;

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::{Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT005: a fallible value must be handled or propagated. A `Result`
    /// used as a statement is a failure the program was told about and walked
    /// past. Match it, `?` it, or -- if the failure really does not matter --
    /// say so with `let _ = …`, which EXPLICIT009 is about.
    pub EXPLICIT005_HANDLE_FALLIBLE,
    Deny,
    "a `Result` used as a statement is a failure no one handled"
}

// `Result`, whatever it is called at the point of use. Asking the type rather
// than the spelling is what makes an alias answer the same as `std`'s own.
fn is_result(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        rustc_middle::ty::Adt(adt, _) => cx
            .tcx
            .is_diagnostic_item(rustc_span::sym::Result, adt.did()),
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit005HandleFallible {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        let StmtKind::Semi(expr) = stmt.kind else {
            return;
        };
        let ty = cx.typeck_results().expr_ty(expr);
        if !is_result(cx, ty) {
            return;
        }
        span_lint_and_then(
            cx,
            EXPLICIT005_HANDLE_FALLIBLE,
            stmt.span,
            "a `Result` is being dropped where it stands",
            |diagnostic| {
                if !stmt.span.from_expansion()
                    && !expr.span.from_expansion()
                    && stmt.span.lo() == expr.span.lo()
                    && infallible_discard::is_call(expr)
                    && let Some(pattern) = infallible_discard::discard_pattern(cx, ty)
                {
                    diagnostic.span_suggestion(
                        expr.span.shrink_to_lo(),
                        "acknowledge the only possible result",
                        format!("let {pattern} = "),
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("handle it with `match`, propagate it with `?`, or write `let _ = …` to say the failure does not matter");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
