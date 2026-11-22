//! EXPLICIT024: no two parameters the compiler would take in either order.
//!
//! Every rule before this one is about what a signature says out loud. This is
//! the one about what a signature cannot say at all, because the type it would
//! have said it with was never written. A pixel count and a number of
//! milliseconds are both `u32`; a workspace name and a window title are both
//! `String`. The compiler takes either one where the other was meant, so the
//! call site that has them the wrong way round compiles, runs, and is wrong
//! somewhere else entirely -- in a frame that is drawn at the wrong size, or a
//! wait that is over before it began.
//!
//! The sentence underneath is larger than anything a lint can ask: distinct
//! quantities want distinct types. The narrow version is the honest one and it
//! is decidable off a signature -- no two parameters the compiler would accept
//! in either order. That is the moment a coincidence of representation stops
//! being a smell and becomes a fault waiting for someone to write the call.
//!
//! Where it stops is bare representations: the integers, the floats, `char`,
//! `str` and `String`. A repeated named type is a pair someone has already
//! decided about -- `fn between(from: Pixels, to: Pixels)` is two of one
//! quantity rather than two quantities, and the answer there is a range if it
//! is anything, not a third type. `bool` stays with EXPLICIT008, which owns it.
//!
//! An alias resolves and the rule fires anyway, which is why the middle
//! signature is read rather than the written one: `type Pixels = u32` is a
//! nickname and not a type, and a rule that took a nickname for a type would
//! be satisfied by the very thing it exists to refuse. Borrowing a bare type
//! still leaves it in scope. Compare the parameter types with their borrow
//! depth and mutability intact, and ignore lifetime differences.
//!
//! Written the way EXPLICIT007 and EXPLICIT008 are, and skipping what they
//! skip. A closure has no call site to protect. A method that implements
//! someone else's trait did not choose its own signature, and the place the
//! choice was made is the trait. A receiver is not a parameter anyone passes.
//! The span goes on the parameter that repeats rather than on the pair,
//! because that is the one someone is going to give a name to.
//!
//! It arrived with the tree breaking it in more places than any rule since 019,
//! and it stood in the warned tier while the pairs were named a crate at a
//! time. They are named now, so the level here is `Deny` and does not move
//! back. Geometry was the worst of it, as the argument for the rule said it
//! would be: a place and a size had been written out in about fifteen crates
//! before `console-core-geometry` gave each of them one name and every call
//! site moved onto it.
//!
//! What the rest wanted was one of three answers, and which one is a reading
//! rather than a rule. A struct with named fields where the two values are one
//! thing with two parts -- a summary and a body are a notification, an old name and a
//! new one are a renaming, a heading and a key are where a line is. A one-word
//! type where they are two roles of one kind of thing, which is what `Ink` and
//! `Ground` had already been doing for a color. And where neither fitted, the
//! reading was that one of the two had been the wrong type all along: a
//! notification's timeout was a `&str` holding a number of milliseconds, and
//! the fix was the number rather than a wrapper around the string.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_abi;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

mod fixes;
mod scope;
mod public_interfaces;
#[path = "../../shared/macro_input.rs"]
mod macro_input;
#[path = "../../shared/authored_function.rs"]
mod authored_function;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl, ImplicitSelfKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

dylint_linting::impl_late_lint! {
    /// EXPLICIT024: two parameters of the same bare representation can be
    /// passed in either order, and the compiler will not say which was meant.
    /// `fn draw(width: u32, height: u32)` accepts a height where a width
    /// belongs. Give each quantity a type of its own -- a newtype, not an
    /// alias -- so the wrong order is a compile error rather than a wrong
    /// picture.
    pub EXPLICIT024_NO_SWAPPABLE_PARAMS,
    Deny,
    "two parameters of one bare representation can be passed in either order; give each quantity its own type",
    Explicit024NoSwappableParams {
        fixes: fixes::Fixes::default(),
        typed_coordinates: dylint_linting::config_or_default("explicit024_typed_coordinates"),
    }
}

#[derive(Default)]
pub struct Explicit024NoSwappableParams {
    fixes: fixes::Fixes,
    typed_coordinates: bool,
}

// A method that implements a trait did not choose its own signature. The place
// the choice was made is the trait, which is where the rule is worth asking.
fn implements_a_trait(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    matches!(
        cx.tcx.def_kind(cx.tcx.parent(def_id.to_def_id())),
        rustc_hir::def::DefKind::Impl { of_trait: true }
    )
}

// The representations that carry no meaning of their own. A named type is a
// decision someone already made; `bool` belongs to EXPLICIT008; and anything
// with a lifetime or a generic in it is not a coincidence of representation.
fn is_bare_representation<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    match ty.kind() {
        ty::Int(_) | ty::Uint(_) | ty::Float(_) | ty::Char | ty::Str => true,
        ty::Adt(held, _) => matches!(
            cx.tcx.def_path_str(held.did()).as_str(),
            "std::string::String" | "alloc::string::String"
        ),
        _ => false,
    }
}

fn same_parameter_type<'tcx>(mut left: Ty<'tcx>, mut right: Ty<'tcx>) -> bool {
    loop {
        match (left.kind(), right.kind()) {
            (ty::Ref(_, lhs, lhs_mutability), ty::Ref(_, rhs, rhs_mutability))
                if lhs_mutability == rhs_mutability =>
            {
                left = *lhs;
                right = *rhs;
            }
            _ => return left == right,
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit024NoSwappableParams {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if self.typed_coordinates
            && let Some(replacement) = public_interfaces::drawing_call(cx, expr)
        {
            span_lint_and_then(cx, EXPLICIT024_NO_SWAPPABLE_PARAMS, expr.span,
                "this drawing call passes positional coordinates through the compatibility API",
                |diagnostic| {
                    diagnostic.span_suggestion(expr.span, "pass points to the typed drawing method", replacement, rustc_errors::Applicability::MachineApplicable);
                });
        }

        if self.typed_coordinates
            && let Some(replacement) = public_interfaces::transform_call(cx, expr)
        {
            span_lint_and_then(cx, EXPLICIT024_NO_SWAPPABLE_PARAMS, expr.span,
                "this transform call passes positional values through the compatibility API",
                |diagnostic| {
                    diagnostic.span_suggestion(expr.span, "name the values for both axes", replacement, rustc_errors::Applicability::MachineApplicable);
                });
        }
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) {
            return;
        }

        if implements_a_trait(cx, def_id) {
            return;
        }


        if clippy_utils::is_test_function(cx.tcx, def_id) {
            return;
        }

        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();

        // A receiver is not a parameter anyone passes, so it cannot be passed
        // in the wrong order either.
        let first = match decl.implicit_self() {
            ImplicitSelfKind::None => 0,
            ImplicitSelfKind::Imm
            | ImplicitSelfKind::Mut
            | ImplicitSelfKind::RefImm
            | ImplicitSelfKind::RefMut => 1,
        };

        let written: Vec<Ty<'tcx>> = signature
            .inputs()
            .iter()
            .skip(first)
            .copied()
            .collect();

        let mut grouped = None;
        for (later, ty) in written.iter().enumerate() {
            let representation = ty.peel_refs();
            if !is_bare_representation(cx, representation) {
                continue;
            }

            let earlier = written
                .iter()
                .take(later)
                .any(|before| same_parameter_type(*before, *ty));

            if !earlier {
                continue;
            }

            let at = match decl.inputs.get(later.saturating_add(first)) {
                Some(at) => at.span,
                None => span,
            };

            let suggestion = grouped.get_or_insert_with(|| self.fixes.for_function(cx, def_id, decl, body));
            span_lint_and_then(
                cx,
                EXPLICIT024_NO_SWAPPABLE_PARAMS,
                at,
                format!(
                    "a second parameter of type `{representation}`: the compiler takes these two in either order and says nothing"
                ),
                |diagnostic| {
                    if let Some((_, edits)) = suggestion.take_if(|(pivot, _)| *pivot == later + first) {
                        diagnostic.multipart_suggestion("name the arguments at every call to this private function", edits, rustc_errors::Applicability::MachineApplicable);
                    } else {
                        diagnostic.help("give each quantity a type of its own -- a newtype, not an alias, since an alias resolves \
                 to the same type and is taken in either order as well. Then the wrong order is a compile \
                 error rather than a wrong answer somewhere else");
                    }
                },
            );
        }

    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn coordinate_calls() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_coordinates")
        .dylint_toml("explicit024_typed_coordinates = true")
        .run();
}
