#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{FnDecl, PrimTy, QPath, Ty, TyKind, def::Res};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT008: bool parameters are forbidden. `fn write(2, true)` is
    /// unreadable at the call site. A typed `enum Mode { Append, Truncate }`
    /// makes the choice legible.
    pub EXPLICIT008_NO_BOOL_PARAM,
    Deny,
    "boolean parameters are forbidden; use an `enum`"
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

impl<'tcx> LateLintPass<'tcx> for Explicit008NoBoolParam {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx rustc_hir::Body<'tcx>,
        span: rustc_span::Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        if implements_a_trait(cx, def_id) {
            return;
        }
        if clippy_utils::is_test_function(cx.tcx, def_id) {
            return;
        }
        for arg in decl.inputs {
            if hir_ty_is_bool(arg) {
                span_lint_and_help(
                    cx,
                    EXPLICIT008_NO_BOOL_PARAM,
                    arg.span,
                    "boolean parameter is forbidden",
                    None,
                    "use an `enum` so the call site reads `Mode::Append`, not `true`",
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