#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_span;

mod decisions;

use clippy_utils::diagnostics::{span_lint_and_help, span_lint_and_then};
use rustc_errors::Applicability;
use rustc_hir::{FnDecl, PrimTy, QPath, Ty, TyKind, def::Res};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use std::collections::BTreeMap;

dylint_linting::impl_late_lint! {
    /// EXPLICIT008: bool parameters are forbidden. `fn write(2, true)` is
    /// unreadable at the call site. A typed `enum Mode { Append, Truncate }`
    /// makes the choice legible.
    pub EXPLICIT008_NO_BOOL_PARAM,
    Deny,
    "boolean parameters are forbidden; use an `enum`",
    Explicit008NoBoolParam {
        decisions: decisions::Decisions::configured(),
    }
}

pub struct Explicit008NoBoolParam {
    decisions: decisions::Decisions,
}

fn hir_ty_is_bool(ty: &Ty<'_>) -> bool {
    matches!(
        ty.kind,
        TyKind::Path(QPath::Resolved(
            _,
            rustc_hir::Path {
                res: Res::PrimTy(PrimTy::Bool),
                ..
            }
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
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.decisions.plan(cx);
    }

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
                if let Some(edits) = self.decisions.take(arg.span) {
                    // Cargo applies each file independently, then recompiles the crate
                    // with the complete set of reviewed changes.
                    let map = cx.sess().source_map();
                    let declaration = map.lookup_source_file(arg.span.lo()).start_pos;
                    let mut by_file = BTreeMap::<_, Vec<_>>::new();
                    for edit in edits {
                        let file = map.lookup_source_file(edit.0.lo());
                        by_file.entry(file.start_pos).or_default().push(edit);
                    }
                    for (file, edits) in by_file {
                        let (span, message) = if file == declaration {
                            (arg.span, "boolean parameter is forbidden")
                        } else {
                            (edits[0].0, "use the named enum for this reviewed decision")
                        };
                        span_lint_and_then(
                            cx,
                            EXPLICIT008_NO_BOOL_PARAM,
                            span,
                            message,
                            |diag| {
                                diag.multipart_suggestion(
                                    "propagate the reviewed enum through this function family",
                                    edits,
                                    Applicability::MachineApplicable,
                                );
                            },
                        );
                    }
                    continue;
                }
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

#[test]
fn reviewed_families() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_reviewed")
        .dylint_toml(include_str!("../ui_reviewed/reviewed.toml"))
        .run();
}
