#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{FnDecl, FnRetTy, QPath, TyKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT002: an infallible function still returns `Result<T, Never>`, so
    /// that every call site reads the same and a function that learns how to
    /// fail does not change the shape of its callers.
    ///
    /// Registered `Allow`, alone in this suite. The other eleven rules describe
    /// a workspace that is nearly there; this one describes a workspace that
    /// does not exist yet -- there is no `Never` type here, and turning it on
    /// denies every function in the tree. It is written so the rule is real and
    /// countable rather than a line in a README:
    ///
    ///     cargo dylint --all -- --all-targets -- -W explicit002_infallible_result
    ///
    /// Turning it on for good is a decision about the whole codebase, and it
    /// wants a `Never` type first.
    pub EXPLICIT002_INFALLIBLE_RESULT,
    Allow,
    "an infallible function should still return `Result<T, Never>`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn returns_result(decl: &FnDecl<'_>) -> bool {
    let FnRetTy::Return(ty) = decl.output else {
        return false;
    };
    matches!(
        ty.kind,
        TyKind::Path(QPath::Resolved(_, p))
            if p.segments.last().is_some_and(|s| s.ident.name.as_str().ends_with("Result"))
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit002InfallibleResult {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        _body: &'tcx rustc_hir::Body<'tcx>,
        _span: rustc_span::Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        if is_test_build(cx) {
            return;
        }
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
        if cx
            .tcx
            .hir_attrs(hir_id)
            .iter()
            .any(|a| a.has_name(rustc_span::sym::test))
        {
            return;
        }
        if returns_result(decl) {
            return;
        }
        // `fn main` and `-> !` answer to nobody.
        if matches!(decl.output, FnRetTy::Return(ty) if matches!(ty.kind, TyKind::Never)) {
            return;
        }
        let span = match decl.output {
            FnRetTy::Return(ty) => ty.span,
            FnRetTy::DefaultReturn(sp) => sp,
        };
        span_lint_and_help(
            cx,
            EXPLICIT002_INFALLIBLE_RESULT,
            span,
            "this function cannot say it succeeded",
            None,
            "return `Result<T, Never>` so that every call site reads the same whether or not it can fail",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
