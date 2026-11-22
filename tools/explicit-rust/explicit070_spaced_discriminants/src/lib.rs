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

extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_errors::Applicability;
use rustc_hir::{EnumDef, ExprKind, Item, ItemKind, VariantData};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::VariantDiscr;
use rustc_span::Span;

dylint_linting::impl_late_lint! {
    /// EXPLICIT070: explicit discriminants on an enum are spaced two bits or
    /// more apart, so a single flipped bit cannot name another real state.
    pub EXPLICIT070_SPACED_DISCRIMINANTS,
    Warn,
    "two discriminants one bit apart; space them so a flipped bit lands between states",
    Explicit070SpacedDiscriminants {
        confirmed: dylint_linting::config_or_default("explicit070_parity"),
    }
}

pub struct Explicit070SpacedDiscriminants {
    confirmed: Vec<String>,
}

// One bit, counted, because the fault this rule is about is exactly one.
fn bits_between(one: u128, another: u128) -> u32 {
    (one ^ another).count_ones()
}

impl Explicit070SpacedDiscriminants {
    fn parity_edits(
        &self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        definition: &EnumDef<'_>,
        values: &[u128],
    ) -> Option<Vec<(Span, String)>> {
        let id = item.owner_id.def_id.to_def_id();
        let module_path = cx.tcx.def_path(id).to_string_no_crate_verbose();
        let qualified = format!("{}{module_path}", cx.tcx.crate_name(id.krate));
        if !self.confirmed.contains(&qualified) {
            return None;
        }

        // The caller must review numeric uses and provide an unsigned repr
        // with room for the extra bit. Keep the existing representation here.
        let repr = cx.tcx.adt_def(id).repr();
        let Some(rustc_abi::IntegerType::Fixed(integer, false)) = repr.int else {
            return None;
        };
        let payload_bits = u128::BITS.checked_sub(values.iter().max()?.leading_zeros())?;
        if repr.c() || u64::from(payload_bits) >= integer.size().bits() {
            return None;
        }
        let parity_bit = 1u128.checked_shl(payload_bits)?;
        let map = cx.sess().source_map();
        if map.span_to_snippet(item.span).ok()?.contains('#') {
            return None;
        }

        let mut edits = Vec::new();
        for (variant, &value) in definition.variants.iter().zip(values) {
            if variant.span.from_expansion() || !matches!(variant.data, VariantData::Unit(..)) {
                return None;
            }
            let encoded = value | (u128::from(value.count_ones() & 1) * parity_bit);
            if let Some(written) = variant.disr_expr {
                let expression = cx.tcx.hir_body(written.body).value;
                if expression.span.from_expansion() || !matches!(expression.kind, ExprKind::Lit(_))
                {
                    return None;
                }
                if encoded != value {
                    edits.push((expression.span, format!("{encoded:#x}")));
                }
            } else {
                edits.push((
                    variant.ident.span.shrink_to_hi(),
                    format!(" = {encoded:#x}"),
                ));
            }
        }
        Some(edits)
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit070SpacedDiscriminants {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if item.span.from_expansion() {
            return;
        }

        let ItemKind::Enum(name, _, definition) = item.kind else {
            return;
        };

        let adt = cx.tcx.adt_def(item.owner_id.def_id.to_def_id());

        // Only an enum somebody has started encoding. No explicit
        // discriminant means the wire has not been agreed on yet.
        let written = adt
            .variants()
            .iter()
            .any(|variant| matches!(variant.discr, VariantDiscr::Explicit(_)));

        if !written {
            return;
        }

        let values: Vec<_> = adt
            .discriminants(cx.tcx)
            .map(|(idx, discr)| (adt.variant(idx).name, discr.val))
            .collect();
        let mut edits = if self.confirmed.is_empty() {
            None
        } else {
            let numbers: Vec<_> = values.iter().map(|(_, value)| *value).collect();
            self.parity_edits(cx, item, &definition, &numbers)
        };

        for (at, (one, one_value)) in values.iter().enumerate() {
            for (another, another_value) in values.iter().skip(at + 1) {
                if bits_between(*one_value, *another_value) >= 2 {
                    continue;
                }

                span_lint_and_then(
                    cx,
                    EXPLICIT070_SPACED_DISCRIMINANTS,
                    item.span,
                    format!(
                        "`{name}`'s discriminants `{one}` and `{another}` are one bit apart, so a \
                         flipped bit turns one into the other and nothing catches it",
                    ),
                    |diagnostic| {
                        if let Some(edits) = edits.take() {
                            diagnostic.multipart_suggestion(
                                "use the reviewed internal encoding with an even parity bit",
                                edits,
                                Applicability::MachineApplicable,
                            );
                        } else {
                            diagnostic.help(
                                "space every pair two bits or more apart, so a single-bit error lands between \
                                 states and the read fails rather than continuing in a state nothing chose. \
                                 Where the encoding is not this enum's to choose -- a protocol somebody else \
                                 wrote -- allow this rule here and let the reason name the protocol",
                            );
                        }
                    },
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}

#[test]
fn parity_fixes() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui")
        .dylint_toml(r#"explicit070_parity = ["parity::Payload", "parity_review::FullByte"]"#)
        .run();
}
