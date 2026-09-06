#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_in_const_context;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT014: indexing and slicing are forbidden. `xs[i]` and `&s[a..b]`
    /// are panics nobody declared -- the harm EXPLICIT004 names, arriving
    /// through syntax instead of a call. `get` and `get_mut` turn the absence
    /// into a value, and EXPLICIT005 then makes sure the value is met.
    ///
    /// Const contexts are left alone, for the reason EXPLICIT015 leaves them
    /// alone: an index the compiler evaluates and finds out of range fails the
    /// build. That is a failure with a name, at the right time, on the machine
    /// that has a screen -- which is the whole of what this rule is asking
    /// for, already granted.
    pub EXPLICIT014_NO_INDEX_SLICE,
    Deny,
    "indexing and slicing are forbidden; ask with `get` and meet the `None`"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit014NoIndexSlice {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if !matches!(expr.kind, ExprKind::Index(..)) {
            return;
        }

        // An index the compiler works out is an index the compiler checks. Out
        // of range there is a build that stops, not a handheld that does.
        if is_in_const_context(cx) {
            return;
        }

        // Code a macro wrote is the macro author's to answer for; what is
        // linted here is what somebody typed into this file.
        if expr.span.from_expansion() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT014_NO_INDEX_SLICE,
            expr.span,
            "indexing is an implicit panic: the missing case has no name here",
            None,
            "ask with `get` / `get_mut` and meet the `None`, so the absent element is a case the code names",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
