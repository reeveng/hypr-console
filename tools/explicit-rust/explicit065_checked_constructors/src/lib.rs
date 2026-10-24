//! EXPLICIT065: a type's invariant is one door, and the door is checked.
//!
//! EXPLICIT048 asked a type to spell no state it does not have; this rule is
//! about the states it *must* have -- the `try_new` that builds a `Ring` only
//! of something, the parse that refuses what is not a profile. A type with an
//! invariant is a type that cannot be built wrong, which is the whole of what
//! it is for; every other route to its value is a route around the check. A
//! `pub` field is one: whoever can write `held.name = …` can write the
//! invariant false. A `Default` is one: it builds the type without asking
//! whether the default even *is* one of its states. A `Deserialize` is one,
//! and it is the route EXPLICIT064 is already worried about -- a type that
//! checks its values by hand can be built by a file that checks nothing.
//!
//! What the rule asks for is the shape `console-core-walking` already has:
//! private fields, one checked constructor, and no way in except through it.
//! The struct literal from outside cannot compile against private fields,
//! which is the check the rule would otherwise have to ask for by hand;
//! `Default` and `Deserialize` are derived on purpose, and the allow this
//! rule accepts for either says why the unchecked route is safe -- usually
//! that the type has no invariant at all, which is a thing worth saying once.
//!
//! It arrives `Warn`, because the tree is full of types whose constructor
//! checks and whose fields are `pub` for the reader's convenience -- each one
//! a decision this rule wants said out loud or a field made private.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{ImplItem, Item, ItemKind, QPath, TyKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use std::collections::HashSet;

dylint_linting::impl_late_lint! {
    /// EXPLICIT065: a type whose constructor checks must have no route around
    /// the check: private fields, and no `Default` or `Deserialize` derived
    /// over them.
    pub EXPLICIT065_CHECKED_CONSTRUCTORS,
    Warn,
    "a checked constructor with a way around the check; make the fields private and derive nothing over them",
    Explicit065CheckedConstructors::new()
}

struct Explicit065CheckedConstructors {
    structs: Vec<(LocalDefId, Span, Vec<Span>)>,
    checked: HashSet<LocalDefId>,
    unchecked: Vec<(LocalDefId, String, Span)>,
}

impl Explicit065CheckedConstructors {
    fn new() -> Self {
        Self {
            structs: Vec::new(),
            checked: HashSet::new(),
            unchecked: Vec::new(),
        }
    }
}

// The type an implementation is for, when the implementation is written in
// this crate and is for a type written here too.
fn self_type_of(item: &Item<'_>) -> Option<LocalDefId> {
    let ItemKind::Impl(under) = item.kind else {
        return None;
    };

    let TyKind::Path(QPath::Resolved(_, path)) = under.self_ty.kind else {
        return None;
    };

    match path.res {
        Res::Def(DefKind::Struct | DefKind::Enum | DefKind::Union, did) => did.as_local(),
        _ => None,
    }
}

// A constructor that refuses: a `try_new`, or a `new` that answers with a
// `Result`. Read off the name and the signature, not the body, because the
// rule is about the route other code takes in.
fn refuses(bound: &ImplItem<'_>) -> bool {
    let rustc_hir::ImplItemKind::Fn(sig, _) = bound.kind else {
        return false;
    };

    match bound.ident.as_str() {
        "try_new" => true,
        "new" => {
            let rustc_hir::FnRetTy::Return(written) = sig.decl.output else {
                return false;
            };

            matches!(written.kind, TyKind::Path(QPath::Resolved(_, path))
                if path.segments.last().is_some_and(|last| last.ident.as_str() == "Result"))
        }
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit065CheckedConstructors {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        match item.kind {
            ItemKind::Struct(_, _, data) => {
                let opened: Vec<_> = data
                    .fields()
                    .iter()
                    .filter(|field| cx.tcx.visibility(field.def_id.to_def_id()).is_public())
                    .map(|field| field.ty.span)
                    .collect();

                self.structs.push((item.owner_id.def_id, item.span, opened));
            }

            ItemKind::Impl(_) => {
                let Some(owner) = self_type_of(item) else {
                    return;
                };

                let ItemKind::Impl(under) = item.kind else {
                    return;
                };

                if under
                    .items
                    .iter()
                    .any(|id| refuses(cx.tcx.hir_impl_item(*id)))
                {
                    self.checked.insert(owner);
                }

                // The unchecked routes arrive as impls: a derive spends its
                // attribute into an `impl Default` before any rule can read
                // the attribute, so the impl is what this rule reads instead
                // -- and a hand-written `Default` is the same route by
                // another hand.
                let Some(header) = under.of_trait else {
                    return;
                };

                let path = header.trait_ref.path;

                let Res::Def(DefKind::Trait, id) = path.res else {
                    return;
                };

                let named = cx.tcx.def_path_str(id);
                let last = named.rsplit("::").next().unwrap_or_default();

                if matches!(last, "Default" | "Deserialize") {
                    self.unchecked.push((owner, last.to_owned(), item.span));
                }
            }

            _ => {}
        }
    }

    // The implementation can come after the struct it implements, so the
    // decision is made once, at the end of the crate.
    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (def_id, span, opened) in &self.structs {
            if !self.checked.contains(def_id) {
                continue;
            }

            let routes: Vec<_> = self
                .unchecked
                .iter()
                .filter(|(of, _, _)| of == def_id)
                .map(|(_, named, at)| (named.as_str(), *at))
                .collect();

            if !routes.is_empty() {
                let named: Vec<_> = routes.iter().map(|(one, _)| *one).collect();

                span_lint_and_help(
                    cx,
                    EXPLICIT065_CHECKED_CONSTRUCTORS,
                    *span,
                    format!(
                        "a type whose constructor refuses, with an `impl {}` beside it -- a route \
                         around the check",
                        named.join(", ")
                    ),
                    None,
                    "the constructor refuses what the impl accepts, and anything that can build the \
                     type without asking is the check undone. Where the type really has no invariant \
                     the impl can break, allow this rule here and let the reason say so",
                );
            }

            for field in opened {
                span_lint_and_help(
                    cx,
                    EXPLICIT065_CHECKED_CONSTRUCTORS,
                    *field,
                    "a field anyone can write, on a type whose constructor refuses",
                    None,
                    "the field wants to be private, so the invariant is the constructor's to keep \
                     and a reader outside has to ask for it. Where the type really has no invariant \
                     -- nothing a writer could set false -- allow this rule here and let the reason \
                     say so",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
