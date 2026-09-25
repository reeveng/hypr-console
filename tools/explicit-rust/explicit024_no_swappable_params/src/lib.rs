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
//! be satisfied by the very thing it exists to refuse. References peel before
//! two types are compared -- `&str` beside `&str` is the same pair of hands
//! taking the same two words in either order.
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

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl, ImplicitSelfKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT024: two parameters of the same bare representation can be
    /// passed in either order, and the compiler will not say which was meant.
    /// `fn draw(width: u32, height: u32)` accepts a height where a width
    /// belongs. Give each quantity a type of its own -- a newtype, not an
    /// alias -- so the wrong order is a compile error rather than a wrong
    /// picture.
    pub EXPLICIT024_NO_SWAPPABLE_PARAMS,
    Deny,
    "two parameters of one bare representation can be passed in either order; give each quantity its own type"
}

// Tests are exempt, as they are for every rule here. `opts.test` is true only
// for the harness build of a target, so the ordinary build of the same library
// is linted as production.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
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

impl<'tcx> LateLintPass<'tcx> for Explicit024NoSwappableParams {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        if is_test_build(cx) {
            return;
        }

        if matches!(kind, FnKind::Closure) {
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
            .map(|ty| ty.peel_refs())
            .collect();

        for (later, ty) in written.iter().enumerate() {
            if !is_bare_representation(cx, *ty) {
                continue;
            }

            let earlier = written.iter().take(later).any(|before| before == ty);

            if !earlier {
                continue;
            }

            let at = match decl.inputs.get(later.saturating_add(first)) {
                Some(at) => at.span,
                None => span,
            };

            span_lint_and_help(
                cx,
                EXPLICIT024_NO_SWAPPABLE_PARAMS,
                at,
                format!(
                    "a second parameter of type `{ty}`: the compiler takes these two in either order and says nothing"
                ),
                None,
                "give each quantity a type of its own -- a newtype, not an alias, since an alias resolves \
                 to the same type and is taken in either order as well. Then the wrong order is a compile \
                 error rather than a wrong answer somewhere else",
            );
        }

        let _ = body;
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
