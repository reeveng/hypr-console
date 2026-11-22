#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

mod fixes;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::{FnDecl, FnRetTy, LangItem, TraitFn, TraitItem, TraitItemKind, def_id::DefId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty, TyKind};
use std::collections::HashMap;

dylint_linting::impl_late_lint! {
    /// EXPLICIT007: a boolean function result must name the outcomes in an
    /// enum. `Option` and `Result` describe absence and failure. A boolean
    /// payload still needs a name for each of its outcomes.
    pub EXPLICIT007_NO_BOOL_RETURN,
    Deny,
    "boolean function results are forbidden; name the outcomes in an enum",
    Explicit007NoBoolReturn {
        outcomes: dylint_linting::config_or_default("explicit007_enum_outcomes"),
        uses: None,
    }
}

pub struct Explicit007NoBoolReturn {
    outcomes: HashMap<String, [String; 3]>,
    uses: Option<fixes::Uses>,
}

fn boolean_answer(cx: &LateContext<'_>, answer: Ty<'_>) -> bool {
    match answer.peel_refs().kind() {
        TyKind::Bool => true,
        TyKind::Adt(held, arguments) if matches!(
            cx.tcx.get_diagnostic_name(held.did()), Some(rustc_span::sym::Option | rustc_span::sym::Result)
        ) => arguments.types().any(|value| boolean_answer(cx, value)),
        _ => false,
    }
}

fn answers_a_bool(cx: &LateContext<'_>, def_id: DefId) -> bool {
    let answer = cx.tcx.fn_sig(def_id).skip_binder().output().skip_binder();
    if boolean_answer(cx, answer) {
        return true;
    }
    let TyKind::Alias(alias) = answer.kind() else {
        return false;
    };
    let def_id = match alias.kind {
        ty::Opaque { def_id } => def_id,
        ty::Projection { def_id } if cx.tcx.is_impl_trait_in_trait(def_id) => def_id,
        _ => return false,
    };
    cx.tcx.explicit_item_self_bounds(def_id).skip_binder().iter().any(|(clause, _)| {
        let ty::ClauseKind::Projection(projection) = clause.kind().skip_binder() else {
            return false;
        };
        cx.tcx.is_lang_item(projection.projection_term.def_id(), LangItem::FutureOutput)
            && matches!(projection.term.kind(), ty::TermKind::Ty(value) if boolean_answer(cx, value))
    })
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

fn say(cx: &LateContext<'_>, decl: &FnDecl<'_>, edits: Option<fixes::Edits>) {
    let FnRetTy::Return(ty) = decl.output else {
        return;
    };
    span_lint_and_then(
        cx,
        EXPLICIT007_NO_BOOL_RETURN,
        ty.span,
        "boolean return value is forbidden",
        |diagnostic| match edits {
            Some(edits) => {
                diagnostic.multipart_suggestion(
                    "return the configured enum and update every direct call",
                    edits,
                    rustc_errors::Applicability::MachineApplicable,
                );
            }
            None => {
                diagnostic.help("name both outcomes in an enum, including inside an `Option` or `Result`");
            }
        },
    );
}

impl<'tcx> LateLintPass<'tcx> for Explicit007NoBoolReturn {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx rustc_hir::Body<'tcx>,
        _span: rustc_span::Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        // Skip closures.
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        if implements_a_trait(cx, def_id) {
            return;
        }
        if !answers_a_bool(cx, def_id.to_def_id()) {
            return;
        }
        let edits = if self.outcomes.is_empty() {
            None
        } else {
            self.outcomes.get(&fixes::function_path(cx, def_id))
                .and_then(|outcomes| fixes::plan(cx, def_id, decl, body, outcomes, &mut self.uses))
        };
        say(cx, decl, edits);
    }

    // A trait method with no body is never a `check_fn`, and it is the one
    // place the choice was made for every impl of it. A method with a default
    // body was asked above.
    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        let TraitItemKind::Fn(sig, TraitFn::Required(_)) = item.kind else {
            return;
        };
        if answers_a_bool(cx, item.owner_id.to_def_id()) {
            say(cx, sig.decl, None);
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn enum_outcomes() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_enum_outcomes")
        .dylint_toml(r#"
[explicit007_enum_outcomes]
"configured::bytes::classify" = ["ByteClass", "Control", "Data"]
"blocked::exported" = ["ByteClass", "Control", "Data"]
"blocked::foreign" = ["ByteClass", "Control", "Data"]
"blocked::constant" = ["ByteClass", "Control", "Data"]
"blocked::first_class" = ["ByteClass", "Control", "Data"]
"blocked::alternate" = ["ByteClass", "Control", "Data"]
"blocked::returns_early" = ["ByteClass", "Control", "Data"]
"blocked::conditional_return" = ["ByteClass", "Control", "Data"]
"blocked::missing_variant" = ["ByteClass", "Control", "Other"]
"blocked::gated_variant::classify" = ["ByteClass", "Control", "Data"]
"blocked::macro_call::classify" = ["ByteClass", "Control", "Data"]
"blocked::unresolved_macro::classify" = ["ByteClass", "Control", "Data"]
"blocked::external_child::classify" = ["ByteClass", "Control", "Data"]
"blocked::trait_owner::Check::classify" = ["ByteClass", "Control", "Data"]
"blocked::drop_enum::classify" = ["ByteClass", "Control", "Data"]
"#)
        .run();
}
