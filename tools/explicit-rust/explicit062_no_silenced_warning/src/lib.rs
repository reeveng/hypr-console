//! EXPLICIT062: a warning is a fault that has not been answered.
//!
//! The Power of Ten rule this tree keeps to is that the build is clean or the
//! build is explained: `deny(warnings)` in every profile, and `unused_results`,
//! `unreachable_pub`, `dead_code` and `missing_docs` answered rather than
//! filed away. A warning silenced is a fault the compiler already found and
//! somebody declined to hear, and it accumulates -- the next one lands beside
//! it in the same attribute, because the attribute is already there, and the
//! build stays green while the code gets worse.
//!
//! What this rule denies is the silencing, not the warnings themselves: an
//! `allow(warnings)` -- or the narrower `allow(unused)`, `allow(dead_code)`,
//! `allow(unreachable_pub)`, `allow(unused_results)`, `allow(missing_docs)`
//! -- is a lint turned off for a whole region at a stroke, which is a door
//! every later fault walks through. The answer a warning wants is the answer
//! every other rule here asks for: the fix, or an `expect` in its place.
//!
//! A reason does not answer it. EXPLICIT018 already holds every `allow` to
//! one, so a rule that let a reason through would be a rule nothing in the
//! tree could reach -- and the reason is not what is missing. Code that is
//! dead with a sentence beside it is still dead the day the sentence stops
//! being true, and the `allow` goes on silencing it after the code it argued
//! for has been called, or deleted, or replaced by something nobody meant to
//! keep. `expect` is not caught: it says which warning it expects, and the
//! compiler takes it away again when the expectation stops holding, so it
//! cannot accumulate. An `allow` of these has no such property; it is a door
//! with nobody watching it.
//!
//! It arrives `Warn`, over a tree whose region-wide allows all carry their
//! reason and want the same word changed: the reason stays, and `expect`
//! keeps it honest.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::Attribute;
use rustc_lint::{EarlyContext, EarlyLintPass};
use rustc_span::sym;

dylint_linting::declare_early_lint! {
    /// EXPLICIT062: `allow(warnings)` and the region-wide allows of the
    /// compile-clean set (`unused`, `dead_code`, `unreachable_pub`,
    /// `unused_results`, `missing_docs`) silence faults the compiler already
    /// found. Fix the warning, or `expect` it, so the silence lapses when the
    /// warning does.
    pub EXPLICIT062_NO_SILENCED_WARNING,
    Warn,
    "a warning silenced by region rather than answered; fix it, or `expect` it so the silence lapses"
}

impl EarlyLintPass for Explicit062NoSilencedWarning {
    fn check_attribute(&mut self, cx: &EarlyContext<'_>, attr: &Attribute) {
        if attr.span.from_expansion() {
            return;
        }

        if !attr.has_name(sym::allow) {
            return;
        }

        let Some(entries) = attr.meta_item_list() else {
            return;
        };

        // The lint names are said by string because `sym::` holds only what
        // the compiler itself reaches for; these six are the compile-clean
        // set, and a string is what `has_name` asks for either way.
        let silenced = [
            "warnings",
            "unused",
            "dead_code",
            "unreachable_pub",
            "unused_results",
            "missing_docs",
        ];

        let caught = entries.iter().any(|entry| {
            entry.meta_item().is_some_and(|item| {
                silenced.iter().any(|name| item.has_name(rustc_span::Symbol::intern(name)))
            })
        });

        if !caught {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT062_NO_SILENCED_WARNING,
            attr.span,
            "this `allow` turns a warning off for a whole region rather than answering it",
            None,
            "fix the warning, or write `expect` where the `allow` is, with the same reason: the \
             compiler takes an `expect` away again when the warning stops, and an `allow` stays \
             a door every later fault walks through",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
