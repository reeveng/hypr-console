//! EXPLICIT070: a flipped bit lands nowhere.
//!
//! The code that flies answers machines that answer back, and what comes back
//! is bits: a byte off a wire, a page of memory the firmware walked on, a
//! register the device set. When those bits are read into an enum, the value
//! a single flipped bit produces is either a discriminant that names a real
//! state -- and the machine carries on in a state nothing chose -- or it is
//! a discriminant that matches nothing, and the read is caught. Which of the
//! two happens is decided by nothing but the numbers somebody wrote on the
//! variants, which is why this rule asks them for their distance.
//!
//! Consecutive discriminants -- `0, 1, 2, 3` -- are one flipped bit apart all
//! the way down, and they are exactly what nobody wrote anyway, because Rust
//! writes them when a person writes nothing. What the rule asks for is
//! discriminants chosen so that any single-bit error lands between states:
//! a Hamming distance of two or more between every pair. `0b0000, 0b0011,
//! 0b0101, 0b1001` is the four-state shape; the kernel's protocols choose
//! their own words, and the rule holds them to their spacing.
//!
//! An enum with no explicit discriminants is left alone: its values are
//! `0, 1, 2, 3` by the compiler's hand and the wire the enum describes has
//! not been agreed on yet, which is a decision the rule cannot make for it.
//! Once one discriminant is written, the spacing is a decision somebody
//! started making, and the rule asks the rest of it.
//!
//! It arrives `Warn`, which on this tree today is quiet -- and that is the
//! honest starting distance: no enum in it has agreed a wire encoding yet,
//! and the first one that does will be asked its spacing as it is written.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::VariantDiscr;

dylint_linting::declare_late_lint! {
    /// EXPLICIT070: explicit discriminants on an enum are spaced two bits or
    /// more apart, so a single flipped bit cannot name another real state.
    pub EXPLICIT070_SPACED_DISCRIMINANTS,
    Warn,
    "two discriminants one bit apart; space them so a flipped bit lands between states"
}

// One bit, counted, because the fault this rule is about is exactly one.
fn bits_between(one: u128, another: u128) -> u32 {
    (one ^ another).count_ones()
}

impl<'tcx> LateLintPass<'tcx> for Explicit070SpacedDiscriminants {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if item.span.from_expansion() {
            return;
        }

        let ItemKind::Enum(name, _, _) = item.kind else {
            return;
        };

        let adt = cx.tcx.adt_def(item.owner_id.def_id.to_def_id());

        // Only an enum somebody has started encoding. No explicit
        // discriminant means the wire has not been agreed on yet.
        let written = adt.variants().iter().any(|variant| {
            matches!(variant.discr, VariantDiscr::Explicit(_))
        });

        if !written {
            return;
        }

        let values: Vec<_> = adt
            .discriminants(cx.tcx)
            .map(|(idx, discr)| (adt.variant(idx).name, discr.val))
            .collect();

        for (at, (one, one_value)) in values.iter().enumerate() {
            for (another, another_value) in values.iter().skip(at + 1) {
                if bits_between(*one_value, *another_value) >= 2 {
                    continue;
                }

                span_lint_and_help(
                    cx,
                    EXPLICIT070_SPACED_DISCRIMINANTS,
                    item.span,
                    format!(
                        "`{name}`'s discriminants `{one}` and `{another}` are one bit apart, so a \
                         flipped bit turns one into the other and nothing catches it",
                    ),
                    None,
                    "space every pair two bits or more apart, so a single-bit error lands between \
                     states and the read fails rather than continuing in a state nothing chose. \
                     Where the encoding is not this enum's to choose -- a protocol somebody else \
                     wrote -- allow this rule here and let the reason name the protocol",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
