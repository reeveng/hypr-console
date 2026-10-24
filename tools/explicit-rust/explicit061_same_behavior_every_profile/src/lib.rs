//! EXPLICIT061: what runs is one program, in every profile.
//!
//! A `debug_assert` and a `#[cfg(debug_assertions)]` write two programs into
//! one source file: a check that exists on a developer's machine and not on
//! the device that flies, and a difference nobody can see in either place.
//! The fault is not that the check is slow -- it is that the *behaviour*
//! differs, so the machine that was tested is not the machine that runs, and
//! a fault that appears only where the check was compiled out is a fault the
//! developer's own build cannot reproduce. This tree already refuses the
//! same split inside a single expression: EXPLICIT015's arithmetic names one
//! policy that holds in every profile, because behaviour that differs by
//! profile is the definition of implicit.
//!
//! The spelling this rule asks for is the assert that is always there. Where
//! a check really is too dear to keep, it is a decision with a name at the
//! call site: `checked_*` answering with a value costs the same everywhere,
//! and a check too expensive to run is a check whose cost somebody should be
//! made to see and to allow, not one the build quietly drops.
//!
//! The rule runs over `Cargo.toml`'s half of the argument too -- the same
//! panic strategy and the same overflow checks in every profile -- but a lint
//! cannot read the manifest, so that half lives in the profiles themselves and
//! in review; what the lint can hold is the source.
//!
//! It is a pre-expansion lint because expansion is where the split happens:
//! a `debug_assert` in a build that says `debug_assertions` becomes a real
//! check and in one that does not becomes nothing, so the only moment the
//! macro is the same in every profile is before expansion spends it.
//!
//! It arrives `Warn`, which is also the tier the argument starts in: nothing
//! in the tree spells either form today.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::{Attribute, Stmt, StmtKind};
use rustc_lint::{EarlyContext, EarlyLintPass};
use rustc_span::sym;

dylint_linting::declare_pre_expansion_lint! {
    /// EXPLICIT061: `debug_assert*` and `cfg(debug_assertions)` write two
    /// programs into one file. A check worth writing is worth running in every
    /// profile.
    pub EXPLICIT061_SAME_BEHAVIOR_EVERY_PROFILE,
    Warn,
    "behaviour that exists in one profile and not another; assert, or allow with the reason"
}

const HELP: &str = "a check compiled out on the device is a difference between the program that \
                    was tested and the program that flies. Assert always -- the `checked_*` forms \
                    cost the same everywhere and answer with a value -- or, where a check really \
                    is too dear, allow this rule here and let the reason say what is being paid \
                    for and by whom";

fn is_a_debug_assert(path: &rustc_ast::ast::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| matches!(segment.ident.as_str(), "debug_assert" | "debug_assert_eq" | "debug_assert_ne"))
}

impl EarlyLintPass for Explicit061SameBehaviorEveryProfile {
    // The assert is still a macro here -- this is the AST, before the
    // expansion -- and a `debug_assert!` arrives as a statement of its own,
    // so the check is on the statement the macro *is* rather than on what
    // expansion leaves behind.
    fn check_stmt(&mut self, cx: &EarlyContext<'_>, stmt: &Stmt) {
        let StmtKind::MacCall(mac) = &stmt.kind else {
            return;
        };

        if !is_a_debug_assert(&mac.mac.path) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT061_SAME_BEHAVIOR_EVERY_PROFILE,
            stmt.span,
            "this check exists only in a build that says `debug_assertions`",
            None,
            HELP,
        );
    }

    fn check_attribute(&mut self, cx: &EarlyContext<'_>, attr: &Attribute) {
        if attr.span.from_expansion() {
            return;
        }

        if !attr.has_name(sym::cfg) {
            return;
        }

        let Some(entries) = attr.meta_item_list() else {
            return;
        };

        let on_a_debug_only = entries.iter().any(|entry| {
            entry
                .meta_item()
                .is_some_and(|item| item.has_name(sym::debug_assertions))
        });

        if !on_a_debug_only {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT061_SAME_BEHAVIOR_EVERY_PROFILE,
            attr.span,
            "this code exists only in a build that says `debug_assertions`",
            None,
            HELP,
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
