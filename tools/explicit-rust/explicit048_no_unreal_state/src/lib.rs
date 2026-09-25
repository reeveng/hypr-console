//! EXPLICIT048: a type spells no state it does not have.
//!
//! EXPLICIT007 and 008 ask about a `bool` at a boundary and never about a
//! field. A struct holding a `bool` and two `Option`s passes every other rule
//! in this suite while spelling sixteen combinations of which four mean
//! anything, and the other twelve are reachable by a caller who sets one field
//! and forgets the next. What that struct is, is an enum written out flat: the
//! states are real, they were simply never named, so nothing counts them and
//! nothing fails when a fifth one arrives.
//!
//! The same complaint in a smaller shape is a nesting. `Option<Option<T>>` and
//! `Option<Result<T, E>>` are three states spelled as two questions, and the
//! reader has to work out which of the two absences meant what. One rule covers
//! both, because both are a type whose spellings outnumber its states.
//!
//! `Result<Option<T>, E>` looks like the third of those and is not one, here.
//! EXPLICIT002 puts every function in this tree through a `Result`, so that
//! shape is the ordinary way to say *the call happened and the thing is not
//! there*: the outer answer is about the call and the inner one is about the
//! value, and they are two questions rather than one asked twice. The rule was
//! written with it in and found it in hundreds of signatures, which is the
//! whole argument for the warned tier -- a count is a thing to read before a
//! rule is believed.
//!
//! This is EXPLICIT016's argument moved from the match to the type. 016 denies
//! the wildcard arm so that a decision names every case; the case the wildcard
//! was hiding is often a combination the type should never have let exist.
//! Read out of a Haskell habit: make illegal states unrepresentable.
//!
//! The false positive is the whole difficulty, and it decides how narrow this
//! is. Two `Option` fields are frequently two optional things and are left
//! alone. What is asked about is a `bool` standing beside an absence in the
//! same struct -- a flag and what the flag is about -- or beside another flag,
//! which is the same shape with the payload left out. The message says how many
//! fields it read that way and how many spellings they make, so `just explicit`
//! prints the distance rather than a verdict.
//!
//! The fix at a site is usually more than a rename. Where the flag was set by a
//! check, the variant holds what the check found -- `Yes(Socket)` rather than a
//! yes -- so whatever does the work takes the thing and cannot be called
//! without it, and the gap between the check and the doing closes with it. That
//! shape is what Elixir gets from a second function head; Rust has no second
//! head, and this is where the same guarantee lives instead.
//!
//! Denied, and the count was the thing to argue with first, exactly as this
//! said. Of what it found, some were the flat enum it is named for and are
//! enums now -- a player that is playing, paused or stopped; a button that is
//! loose, held, shared or already gone; a key drawn pressed, under or plain --
//! and every one of those lost a combination no one meant. The rest were
//! fields that really are independent, and each carries the allow with a
//! sentence saying which different question each field answers. That split is
//! the rule working either way, and it is why the message says how many fields
//! it read rather than pronouncing on them.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::Res;
use rustc_hir::{AmbigArg, GenericArg, Item, ItemKind, PrimTy, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT048: a type that can be written into a state it does not have.
    /// A `bool` beside an `Option` in one struct is an enum written out flat,
    /// and `Option<Option<T>>` is three states spelled as two questions. Name
    /// the states in an enum, where the compiler can count them.
    pub EXPLICIT048_NO_UNREAL_STATE,
    Deny,
    "a type spelling more states than it has; name them in an enum"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn is_flag<A>(ty: &Ty<'_, A>) -> bool {
    let TyKind::Path(QPath::Resolved(_, path)) = ty.kind else {
        return false;
    };

    matches!(path.res, Res::PrimTy(PrimTy::Bool))
}

// The last segment of the written path, so `Option<..>`, `core::option::Option<..>`
// and a `use`d spelling of either all answer the same way.
fn named<'a, A>(ty: &'a Ty<'_, A>) -> Option<&'a str> {
    let TyKind::Path(QPath::Resolved(_, path)) = ty.kind else {
        return None;
    };

    path.segments.last().map(|segment| segment.ident.as_str())
}

fn first_type_argument<'hir, A>(ty: &Ty<'hir, A>) -> Option<&'hir Ty<'hir, AmbigArg>> {
    let TyKind::Path(QPath::Resolved(_, path)) = ty.kind else {
        return None;
    };

    let args = path.segments.last()?.args?;

    args.args.iter().find_map(|arg| match arg {
        GenericArg::Type(inner) => Some(*inner),
        _ => None,
    })
}

// A flag beside an absence is the flat enum this rule is named for. Two flags
// are the same shape with the payload left out, and are asked about for the
// same reason. Two absences are left alone: two optional things is what they
// usually are, and a rule that said otherwise would be turned off.
fn what_it_stands_beside(flags: u32, absences: u32) -> Option<&'static str> {
    match (flags, absences) {
        (0, _) => None,
        (_, 0) => match flags {
            1 => None,
            _ => Some("one flag beside another"),
        },
        _ => Some("a flag beside an absence"),
    }
}

fn spellings(fields: u32) -> u32 {
    1u32.checked_shl(fields).unwrap_or(u32::MAX)
}

// `Option<Option<T>>` and `Option<Result<T, E>>` are the two that are really
// one question asked twice. `Result<Option<T>, E>` is not, in this tree: with
// EXPLICIT002 putting every function through a `Result`, the outer answer is
// whether the call happened and the inner one is whether the thing is there,
// which are two questions with different answers. A `Result` inside a `Result`
// is not one of these either -- the second error is a fault of its own, and
// EXPLICIT038 already asks it to be named.
fn is_a_nested_question(outside: &str, inside: &str) -> bool {
    matches!((outside, inside), ("Option", "Option") | ("Option", "Result"))
}

impl<'tcx> LateLintPass<'tcx> for Explicit048NoUnrealState {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if item.span.from_expansion() {
            return;
        }

        let ItemKind::Struct(name, _, data) = item.kind else {
            return;
        };

        let mut flags = 0u32;
        let mut absences = 0u32;

        for field in data.fields() {
            if is_flag(field.ty) {
                flags = flags.saturating_add(1);
                continue;
            }

            if named(field.ty) == Some("Option") {
                absences = absences.saturating_add(1);
            }
        }

        let Some(beside) = what_it_stands_beside(flags, absences) else {
            return;
        };

        let counted = flags.saturating_add(absences);
        let spelled = spellings(counted);

        span_lint_and_help(
            cx,
            EXPLICIT048_NO_UNREAL_STATE,
            name.span,
            format!(
                "`{name}` holds {beside}, so {counted} of its fields spell {spelled} states of \
                 which only a few mean anything"
            ),
            None,
            "a flag beside what it is about, or beside another flag, is an enum written out flat. \
             Name one variant per state that can really happen, and the compiler counts them, a \
             caller cannot set one field and forget the next, and a state added later has to be \
             answered everywhere. Where the flag was set by a check, the variant holds what the \
             check found, so whatever does the work takes that and cannot be called without it. \
             Where the fields really are independent, allow this rule here and let the reason say \
             so",
        );
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        if is_test_build(cx) {
            return;
        }

        let ty: &Ty<'tcx> = ty.as_unambig_ty();

        if ty.span.from_expansion() {
            return;
        }

        let Some(outside) = named(ty) else {
            return;
        };

        if outside != "Option" {
            return;
        }

        let Some(inner) = first_type_argument(ty) else {
            return;
        };

        let Some(inside) = named(inner) else {
            return;
        };

        if !is_a_nested_question(outside, inside) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT048_NO_UNREAL_STATE,
            ty.span,
            format!("`{outside}<{inside}<..>>` is three states spelled as two questions"),
            None,
            "name the three: an enum with a variant per state, so the reader is not left working \
             out which of the two answers the absence came from, and so the compiler counts them",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
