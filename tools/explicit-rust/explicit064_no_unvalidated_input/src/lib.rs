//! EXPLICIT064: outside data enters through a door somebody built.
//!
//! Everything this tree reads that it did not write -- a `.desktop` file, a
//! manifest, a profile, a page of JSON -- arrives as bytes written by some
//! other program on some other day, and the type it is read into is the door.
//! A `Deserialize` that accepts unknown fields is a door that opens wider
//! than the type says: the file on the machine can name fields the type has
//! never heard of and the read still succeeds, so what the program runs on
//! is not what the type describes. A `String` or a `Vec` with no bound is a
//! door that opens as wide as the file: a handful of bytes on a machine
//! somebody compromised becomes a heap nobody budgeted for, on a device
//! nobody is holding.
//!
//! What the rule asks for is the decoder this tree already writes elsewhere:
//! a format read in one place, by the crate that owns it (`console-core-ini`
//! files for a heading and its lines, `console-applications` for what a
//! desktop file *means*), bounded in what it accepts and loud about what it
//! refuses. A serde type that wants to be that door says so: every field
//! known, unknown fields denied, and the lengths that can grow without end
//! bounded at the decoder rather than trusted at the file.
//!
//! It is a pre-expansion lint, which is the one place a rule can read the
//! door: a `derive` and a `#[serde(...)]` are spent by the time the HIR is
//! written -- the derive becomes impls nobody attributes -- and even the
//! early pass that runs after expansion cannot see them. Before expansion is
//! the last moment they are attributes. It arrives `Warn`, over a tree
//! whose serde types are readers for formats this tree also writes, where the
//! unknown field is a compatibility question rather than a hole, and each of
//! those wants either the attribute or the sentence.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::{Item, ItemKind};
use rustc_lint::{EarlyContext, EarlyLintPass};
use rustc_span::Span;

dylint_linting::declare_pre_expansion_lint! {
    /// EXPLICIT064: a `Deserialize` type is the door outside data comes
    /// through. Deny unknown fields, and bound the `String`s and `Vec`s that
    /// grow as wide as the file.
    pub EXPLICIT064_NO_UNVALIDATED_INPUT,
    Warn,
    "outside data read by a type that accepts more than it says; deny unknown fields and bound what grows"
}

// A derive, read off the attribute while it is still an attribute -- before
// expansion spends it into impls. Only `Deserialize` is a door: a type that
// is only ever `Serialize`d is written by this program and read by somebody
// else, and what it accepts is whatever this program put in it.
fn derives_a_decoder(item: &Item) -> bool {
    item.attrs.iter().any(|attr| {
        let Some(entries) = attr.meta_item_list() else {
            return false;
        };

        attr.has_name(rustc_span::Symbol::intern("derive"))
            && entries.iter().any(|entry| {
                entry
                    .meta_item()
                    .is_some_and(|inside| {
                        inside.path.segments.last().is_some_and(|last| {
                            last.ident.as_str() == "Deserialize"
                        })
                    })
            })
    })
}

// The one spelling this rule asks of the door itself.
fn denies_unknown_fields(item: &Item) -> bool {
    item.attrs.iter().any(|attr| {
        let Some(entries) = attr.meta_item_list() else {
            return false;
        };

        attr.has_name(rustc_span::Symbol::intern("serde"))
            && entries.iter().any(|entry| {
                entry
                    .meta_item()
                    .is_some_and(|inside| inside.has_name(rustc_span::Symbol::intern("deny_unknown_fields")))
            })
    })
}

// A type whose length the file decides: `String` and `Vec` read by a bare
// path. An alias is not caught, which is the honest half of the rule -- it
// is the spelling a person writes at the door.
fn grows_without_end(item: &Item) -> Vec<(Span, String)> {
    let ItemKind::Struct(_, _, data) = &item.kind else {
        return Vec::new();
    };

    data.fields()
        .iter()
        .filter_map(|field| {
            let name = match &field.ty.kind {
                rustc_ast::ast::TyKind::Path(_, path) => path
                    .segments
                    .last()
                    .map(|last| last.ident.as_str().to_owned()),
                _ => None,
            }?;

            match name.as_str() {
                "String" | "Vec" => Some((field.ty.span, name)),
                _ => None,
            }
        })
        .collect()
}

impl EarlyLintPass for Explicit064NoUnvalidatedInput {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let ItemKind::Struct(name, _, _) = &item.kind else {
            return;
        };

        let name = name.as_str();

        // A struct nothing decodes into reads no outside data, whatever its
        // fields are.
        if !derives_a_decoder(item) {
            return;
        }

        if !denies_unknown_fields(item) {
            span_lint_and_help(
                cx,
                EXPLICIT064_NO_UNVALIDATED_INPUT,
                item.span,
                format!("`{name}` reads outside data and accepts fields it has never heard of"),
                None,
                "say `#[serde(deny_unknown_fields)]`, so the read fails on what the type does not \
                 describe rather than running on it. Where a field is genuinely not read any more \
                 and files in the wild still name it, allow this rule here and let the reason \
                 name the field and the date it can go",
            );
        }

        for (span, grown) in grows_without_end(item) {
            span_lint_and_help(
                cx,
                EXPLICIT064_NO_UNVALIDATED_INPUT,
                span,
                format!("`{name}` reads outside data into a `{grown}` whose length the file decides"),
                None,
                "bound the length at the decoder -- a fixed-capacity form, or a limit said where \
                 the file is opened -- rather than trusting the file to be the size a person \
                 would have written",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
