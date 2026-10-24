//! EXPLICIT067: a quantity is a type, or it is a bare number.
//!
//! EXPLICIT024 asked that no two parameters share a bare representation, so a
//! pixel count and a number of milliseconds cannot be swapped by a caller who
//! was not looking. This rule is the same argument one step earlier: a
//! physical quantity -- a length, a duration, a weight, a probability -- that
//! is a bare `f32` or `f64` in a signature has no *unit* at all, and a
//! quantity with no unit is a number nobody can check, including the person
//! writing the call. The desktop already owns the answer: a place and a size
//! are types in `console-core-geometry`, not pairs of floats, and a duration
//! is `std::time::Duration` rather than a `f64` somebody has to divide by
//! 1000 to ask.
//!
//! What the rule asks of a signature is that a float in it be a *thing* -- a
//! one-word newtype the way `Ink` and `Ground` name a colour -- so the call
//! site says what the number is and the function cannot be asked the question
//! in the wrong unit. The float does not disappear; it moves inside the type,
//! where the operations that make sense for it are the only ones written.
//! Type-aliasing is not caught (an alias is a nickname, not a type -- the
//! same reading 024 made) because HIR resolves only what resolves.
//!
//! A method is a signature like any other, in an `impl` or declared on a
//! trait. A method that implements a trait is not asked: its signature is
//! the trait's, decided where the trait is declared -- which is asked, when
//! the trait is this tree's -- and not one the `impl` can change.
//!
//! It arrives `Warn`, because the tree still carries bare floats where
//! drawing passes them between functions, and each of those wants either the
//! type or the allow saying it is a ratio.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::trait_ref_of_method;
use rustc_hir::def::Res;
use rustc_hir::{
    FnDecl, ImplItem, ImplItemKind, Item, ItemKind, QPath, TraitItem, TraitItemKind, Ty, TyKind,
};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT067: a bare `f32` or `f64` in a signature or a field is a
    /// quantity with no unit. Give it a one-word type, so the call says what
    /// the number is.
    pub EXPLICIT067_NO_UNIT_LESS_QUANTITY,
    Warn,
    "a float with no unit in a signature; give the quantity a one-word type of its own"
}

const HELP: &str = "give the quantity a one-word type of its own, the way `console-core-geometry` \
                     gives a place and a size each one name, so a caller says what the number is \
                     and the wrong unit does not compile. Where the float really is a ratio or \
                     an answer with no unit at all, allow this rule here and let the reason say so";

fn is_a_bare_float(ty: &Ty<'_>) -> bool {
    match ty.kind {
        TyKind::Path(QPath::Resolved(_, path)) => {
            matches!(path.res, Res::PrimTy(rustc_hir::PrimTy::Float(_)))
        }
        _ => false,
    }
}

fn check_signature(cx: &LateContext<'_>, decl: &FnDecl<'_>) {
    for taken in decl.inputs {
        if is_a_bare_float(taken) {
            span_lint_and_help(
                cx,
                EXPLICIT067_NO_UNIT_LESS_QUANTITY,
                taken.span,
                "a parameter that is a bare float, which is a quantity with no unit",
                None,
                HELP,
            );
        }
    }

    if let rustc_hir::FnRetTy::Return(given) = decl.output
        && is_a_bare_float(given)
    {
        span_lint_and_help(
            cx,
            EXPLICIT067_NO_UNIT_LESS_QUANTITY,
            given.span,
            "an answer that is a bare float, which is a quantity with no unit",
            None,
            HELP,
        );
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit067NoUnitLessQuantity {
    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let ImplItemKind::Fn(sig, _) = item.kind else {
            return;
        };

        if trait_ref_of_method(cx, item.owner_id).is_some() {
            return;
        }

        check_signature(cx, sig.decl);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        let TraitItemKind::Fn(sig, _) = item.kind else {
            return;
        };

        check_signature(cx, sig.decl);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        match item.kind {
            ItemKind::Fn { sig, .. } => check_signature(cx, sig.decl),

            ItemKind::Struct(_, _, data) => {
                for field in data.fields() {
                    if is_a_bare_float(field.ty) {
                        span_lint_and_help(
                            cx,
                            EXPLICIT067_NO_UNIT_LESS_QUANTITY,
                            field.ty.span,
                            "a field that is a bare float, which is a quantity with no unit",
                            None,
                            HELP,
                        );
                    }
                }
            }

            _ => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
