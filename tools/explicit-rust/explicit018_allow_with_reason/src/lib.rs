#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::Attribute;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::sym;

dylint_linting::declare_early_lint! {
    /// EXPLICIT018: an `allow` carries its reason. A rule is allowed at a call
    /// site only when the harm it names is absent there, and the allow says
    /// which -- that has been this suite's policy in prose since the first
    /// allow; this rule moves it from the README into the attribute, where the
    /// compiler keeps it next to the site it excuses.
    ///
    /// `expect` is held to the same sentence. `warn` and `deny` are not: they
    /// hide nothing, so they have nothing to explain.
    pub EXPLICIT018_ALLOW_WITH_REASON,
    Warn,
    "an `allow` with no `reason` is a rule waived in silence"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &EarlyContext<'_>) -> bool {
    cx.sess().opts.test
}

impl EarlyLintPass for Explicit018AllowWithReason {
    fn check_attribute(&mut self, cx: &EarlyContext<'_>, attr: &Attribute) {
        if is_test_build(cx) {
            return;
        }

        // A derive writes allows of its own; those are the derive author's.
        if attr.span.from_expansion() {
            return;
        }

        if !attr.has_name(sym::allow) && !attr.has_name(sym::expect) {
            return;
        }

        let Some(entries) = attr.meta_item_list() else {
            return;
        };

        let has_reason = entries
            .iter()
            .any(|entry| entry.meta_item().is_some_and(|item| item.has_name(sym::reason)));

        if has_reason {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT018_ALLOW_WITH_REASON,
            attr.span,
            "this `allow` waives a rule without saying why",
            None,
            "write `allow(<lint>, reason = \"…\")` naming which harm is absent at this site and what makes it so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
