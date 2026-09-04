#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{FnDecl, PrimTy, QPath, Ty, TyKind, def::Res};
use rustc_lint::{LateContext, LateLintPass, LintContext};

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

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A method that implements a trait did not choose its own signature. The rule
// is about a choice, and in an impl of somebody else's trait there is none:
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
        if is_test_build(cx) {
            return;
        }
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        if implements_a_trait(cx, def_id) {
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