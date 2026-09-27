#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{FnDecl, FnRetTy, PrimTy, QPath, Ty, TyKind, def::Res};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT007: `fn … -> bool` is forbidden. A boolean status return hides
    /// what went wrong. Use an `enum` (or a `Result`) so the failure mode is
    /// visible at the call site.
    pub EXPLICIT007_NO_BOOL_RETURN,
    Deny,
    "boolean return values are forbidden; use an `enum` or `Result`"
}

fn hir_ty_is_bool(ty: &Ty<'_>) -> bool {
    matches!(
        ty.kind,
        TyKind::Path(QPath::Resolved(
            _,
            rustc_hir::Path { res: Res::PrimTy(PrimTy::Bool), .. }
        ))
    )
}

// A method that implements a trait did not choose its own signature. The rule
// is about a choice, and in an impl of someone else's trait there is none:
// `PartialEq::eq` answers with a `bool` because the trait says it does, and a
// type that wants to be compared has no other way to say so. The place the
// choice was made is the trait, which is where the rule is worth asking.
fn implements_a_trait(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    matches!(
        cx.tcx.def_kind(cx.tcx.parent(def_id.to_def_id())),
        rustc_hir::def::DefKind::Impl { of_trait: true }
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit007NoBoolReturn {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx rustc_hir::Body<'tcx>,
        span: rustc_span::Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        // Skip closures.
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        if implements_a_trait(cx, def_id) {
            return;
        }
        // Skip #[test] functions — they idiomatically return ().
        if clippy_utils::is_test_function(cx.tcx, def_id) {
            return;
        }
        if let FnRetTy::Return(ty) = &decl.output {
            if hir_ty_is_bool(ty) {
                span_lint_and_help(
                    cx,
                    EXPLICIT007_NO_BOOL_RETURN,
                    ty.span,
                    "boolean return value is forbidden",
                    None,
                    "use an `enum` (or `Result`) so the failure mode is visible to the caller",
                );
            }
        }
        let _ = (body, span);
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}