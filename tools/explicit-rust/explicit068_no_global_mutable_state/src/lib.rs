//! EXPLICIT068: a process shares nothing by accident.
//!
//! EXPLICIT044 said a function decides from what it was handed; this rule is
//! the same sentence about a whole process. A `static mut` is a value nobody
//! handed anybody: every function that reaches it is a function that takes an
//! input nothing in its signature names, cannot be asked twice with the same
//! answer, and -- on more than one thread -- is a data race the compiler was
//! told not to notice. A `OnceLock` or a `LazyLock` behind a `static` is the
//! same fault one step slower, a value the first caller initializes and every
//! later one depends on; a `thread_local!` is a `static` per thread, which
//! fixes the race and keeps the invisible input. `lazy_static!` was all three
//! at once, and this tree never wrote it. A `static` of a `Mutex` or an
//! atomic is the same value with the race taken out and the input left in:
//! it is not `mut`, and it changes anyway, because what it holds is a cell.
//! The rule reads that off the type rather than off its name -- a `static`
//! whose type is not `Freeze` has a cell in it somewhere, and every cell in a
//! `static` is state the whole process shares.
//!
//! What the rule asks for is the shape the rest of the tree already runs on:
//! state is handed in. A program's values live in its `main` and are passed
//! down, or behind an actor (`console-actor`) that owns them and answers
//! requests about them, so every function that decides from a value says
//! which value in its signature and can be pressed twice with the same
//! answer. Where a process really has one global thing -- the program's own
//! name, a lock the allocator owns -- the site carries the allow and its
//! reason says why nothing else could hold it, which is the sentence
//! `console-core-external-programs` and `console-core-places` already say for
//! the values each of them owns once.
//!
//! It arrives `Warn`: the tree keeps a handful of `static` values, and each
//! one it prints is either a constant in disguise -- a `const`, which this
//! rule does not touch, because it is a value not a state -- or a value
//! somebody has to own.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Item, ItemKind, Mutability, QPath, TyKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT068: no `static mut`, no mutable singleton behind a `static`,
    /// no `static` lock or atomic, no `thread_local!`. State is handed in, or an actor owns it; a global
    /// is an input nothing named.
    pub EXPLICIT068_NO_GLOBAL_MUTABLE_STATE,
    Warn,
    "a value the whole process shares, which nobody handed anybody; hand state in instead"
}

// A `static` whose type is the mutable-singleton family. `thread_local!`
// arrives here as a `static` of a `LocalKey`, and `lazy_static!` as a
// `LazyLock`, so the one place the rule reads is the item the macro left
// behind.
fn is_a_mutable_singleton(ty: &rustc_hir::Ty<'_>) -> bool {
    match ty.kind {
        TyKind::Path(QPath::Resolved(_, path)) => path.segments.last().is_some_and(|last| {
            matches!(
                last.ident.as_str(),
                "OnceLock" | "LazyLock" | "LocalKey" | "LazyStorage"
            )
        }),
        _ => false,
    }
}

// A type that is not `Freeze` has an `UnsafeCell` inside it, which is what
// every lock, atomic and once-cell is built on: the one way a value that is
// not `mut` changes.
fn holds_a_cell(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
    !cx.tcx
        .type_of(item.owner_id)
        .instantiate_identity()
        .skip_norm_wip()
        .is_freeze(cx.tcx, cx.typing_env())
}

impl<'tcx> LateLintPass<'tcx> for Explicit068NoGlobalMutableState {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Static(how, name, held, _) = item.kind else {
            return;
        };

        if item.span.from_expansion() {
            // The exception is the one the head names: `thread_local!`
            // writes a `static` that is nobody's but its macro's, and the
            // rule wants it caught -- as itself, below, not by pretending it
            // was a `static` a hand wrote.
            let caught_by_expansion = is_a_mutable_singleton(held);

            if !caught_by_expansion {
                return;
            }

            span_lint_and_help(
                cx,
                EXPLICIT068_NO_GLOBAL_MUTABLE_STATE,
                item.span.source_callsite(),
                "this `thread_local!` is state the whole thread shares, which nobody handed anybody",
                None,
                "hand the value in, or let an actor own it and answer requests about it. Where \
                 the crate really is the one place the value can live, allow this rule here and \
                 let the reason say why nothing else could hold it",
            );

            return;
        }

        match how {
            Mutability::Mut => {
                span_lint_and_help(
                    cx,
                    EXPLICIT068_NO_GLOBAL_MUTABLE_STATE,
                    item.span,
                    format!("`{name}` is `static mut`: an input nothing named and a race"),
                    None,
                    "hand the value in, or let an actor own it and answer requests about it. \
                     Where the crate really is the one place the value can live, allow this rule \
                     here and let the reason say why nothing else could hold it",
                );
            }

            Mutability::Not if is_a_mutable_singleton(held) => {
                span_lint_and_help(
                    cx,
                    EXPLICIT068_NO_GLOBAL_MUTABLE_STATE,
                    item.span,
                    format!(
                        "`{name}` is a mutable singleton behind a `static`, which is a value \
                         the first caller decides for every later one"
                    ),
                    None,
                    "hand the value in, or let an actor own it and answer requests about it. \
                     Where the crate really is the one place the value can live, allow this rule \
                     here and let the reason say why nothing else could hold it",
                );
            }

            Mutability::Not if holds_a_cell(cx, item) => {
                span_lint_and_help(
                    cx,
                    EXPLICIT068_NO_GLOBAL_MUTABLE_STATE,
                    item.span,
                    format!(
                        "`{name}` is a `static` that changes -- a lock, an atomic, a cell -- which \
                         is state every caller shares and nobody handed anybody"
                    ),
                    None,
                    "hand the value in, or let an actor own it and answer requests about it. \
                     Where the crate really is the one place the value can live, allow this rule \
                     here and let the reason say why nothing else could hold it",
                );
            }

            Mutability::Not => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
