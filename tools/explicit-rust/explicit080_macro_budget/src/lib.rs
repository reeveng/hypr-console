//! EXPLICIT080: a macro is a language with one speaker.
//!
//! A `macro_rules!` is a little language invented inside the file: new
//! syntax, new rules, and a reader who cannot understand any line that uses
//! it until they have also read its definition. That is a cost the tree pays
//! sometimes and pays on purpose -- a shape the language lacks (matching the
//! `#[words]` the tree now derives instead), a repetition too wide to write
//! out -- and this rule does not ask that it never be paid. It asks that
//! every macro be *budgeted*: written on purpose, with the reason a reader
//! needs, in the same way EXPLICIT018 asks every `allow` for its reason --
//! because a macro whose reason is nowhere is a word nobody can say why
//! anyone speaks.
//!
//! The budget itself is the number the reason names: what this macro saves
//! over the code it replaces, and what a reader has to hold to use it. A
//! macro that saves more than it costs is a good one, and the crate that
//! defines it says so in the allow; a macro that costs more than it saves is
//! a function, and functions are easier to read. A proc macro is the same
//! question at a higher price -- code that *runs* to write code -- and in
//! the flight crates EXPLICIT078 has already answered it.
//!
//! It arrives `Warn`, over a tree whose macros are few and old enough to
//! have earned their place; each of them now owes its sentence, and the next
//! one owes one before it is written.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::{Item, ItemKind};
use rustc_lint::{EarlyContext, EarlyLintPass};

dylint_linting::declare_early_lint! {
    /// EXPLICIT080: a `macro_rules!` is a little language invented inside the
    /// file. Write it on purpose, with an allow whose reason says what it
    /// saves over the code it replaces.
    pub EXPLICIT080_MACRO_BUDGET,
    Warn,
    "a macro_rules with no reason at it; say what it saves over the code it replaces"
}

impl EarlyLintPass for Explicit080MacroBudget {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if item.span.from_expansion() {
            return;
        }

        if !matches!(item.kind, ItemKind::MacroDef(..)) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT080_MACRO_BUDGET,
            item.span,
            "this is a little language invented inside the file, and nobody said what it is for",
            None,
            "a macro is a cost every reader of it pays: new syntax whose definition is not on the \
             line that uses it. Where it really saves more than it costs, allow this rule here \
             and let the reason say what it saves; where it does not, it wants to be a function",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
