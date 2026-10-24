//! EXPLICIT058: unsafe is a place, not a permission.
//!
//! EXPLICIT012 asks every `unsafe` block to say which invariant it relies on,
//! which keeps an unsafe honest once written. This rule asks the question
//! before that one: whether it should be written here at all. An `unsafe` is
//! a hole in every guarantee the rest of the tree is compiled against, and
//! the ones this tree carries -- holding a child's death signal, mapping the
//! framebuffer the kernel already checked, writing to a page two processes
//! agreed on -- are the narrow ends of questions somebody else asked: the
//! kernel, the C library, the protocol. They are written where they are
//! because nothing else *can* hold them, and that is exactly what the allow
//! this rule asks for has to say.
//!
//! What the rule denies is the unsafe that exists because it was convenient:
//! a pointer taken to skip a borrow checker's answer, a `transmute` (which
//! EXPLICIT059 owns), a cast that a safe call answers as a value. Those
//! belong behind a type that checks once, not at every call site after.
//!
//! It arrives `Warn` over a tree that already carries unsafe blocks with
//! `SAFETY` reasons on them. Each one it prints is either a sentence this
//! rule is waiting to hear -- why nothing else could hold it -- or an unsafe
//! that has a safe spelling nobody has written yet. When the last of the
//! second kind is gone, the rule moves to `Deny` and the first kind carries
//! its allow in peace.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Block, BlockCheckMode, Item, ItemKind, Safety};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT058: an `unsafe` is a hole in the guarantees the rest of the
    /// tree is compiled against. Write it only where nothing else could hold
    /// it, and carry an allow whose reason names who is doing the checking
    /// the block stands in for.
    pub EXPLICIT058_CONTAIN_UNSAFE,
    Warn,
    "`unsafe` outside the places named to hold it; say who checks what the block stands in for"
}

const HELP: &str = "an `unsafe` block is held to this rule wherever it is written. Either the \
                     guarantee is checked somewhere -- the kernel, the type, the protocol -- and \
                     the checking belongs behind a type of its own that checks once, or it is \
                     held nowhere and must not be written. Where the site really is the one that \
                     cannot be helped -- a signal handler, a page two processes agreed on -- \
                     allow this rule here and let the reason say who is doing the checking";

impl<'tcx> LateLintPass<'tcx> for Explicit058ContainUnsafe {
    fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx Block<'tcx>) {
        if !matches!(block.rules, BlockCheckMode::UnsafeBlock(_)) {
            return;
        }

        if block.span.from_expansion() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT058_CONTAIN_UNSAFE,
            block.span,
            "an `unsafe` block is written here",
            None,
            HELP,
        );
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        match item.kind {
            ItemKind::Fn { sig, .. }
                if matches!(
                    sig.header.safety,
                    rustc_hir::HeaderSafety::Normal(Safety::Unsafe)
                ) =>
            {
                span_lint_and_help(
                    cx,
                    EXPLICIT058_CONTAIN_UNSAFE,
                    item.span,
                    "an `unsafe fn` is written here, which hands the invariant to every caller",
                    None,
                    HELP,
                );
            }

            // An `unsafe impl` is an `impl` of a trait whose header claims it.
            ItemKind::Impl(under)
                if under
                    .of_trait
                    .is_some_and(|header| header.safety == Safety::Unsafe) =>
            {
                span_lint_and_help(
                    cx,
                    EXPLICIT058_CONTAIN_UNSAFE,
                    item.span,
                    "an `unsafe` impl is written here",
                    None,
                    HELP,
                );
            }

            _ => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
