//! EXPLICIT030: a copy is not made in order to lend it.
//!
//! `f(&held.clone())` allocates a whole second copy of something, hands out a
//! borrow of the copy, and drops it on the next line. `&held` was already a
//! borrow of the thing itself and would have done. The copy is not a mistake
//! anyone makes on purpose: it is what a borrow checker complaint turns into
//! when the fix is a guess, which is why it collects in generated code and in
//! code written in a hurry, and why it survives review -- the line compiles,
//! the line is short, and the only thing wrong with it is invisible.
//!
//! It is asked narrowly, so that what it says is always true: the copy has to
//! be of the same type as the thing copied, once references are peeled off
//! both. That is the case where `&held` is exactly what `&held.clone()` was
//! standing in for and nothing else has to change. `&text.to_string()` where
//! `text` is a `&str` is a different question -- the types differ, a coercion is
//! doing work, and what to write instead depends on what the other side wanted
//! -- so it is left alone rather than half-answered.
//!
//! What survives the rule is every copy that is kept. A clone stored, returned,
//! pushed onto something, or moved into a closure that outlives the line is a
//! copy someone needs, and none of those are written with an `&` in front of
//! them.

//!
//! It arrived `Warn` with almost nothing in front of it, and what it did find is
//! worth reading before believing: a copy lent to one argument of a call whose
//! next argument moves the original is a copy the borrow checker asked for. That
//! is the site's allow and the sentence is easy to write, which is the test this
//! rule sets -- a copy no one can explain is a copy no one chose.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT030: `&thing.clone()` allocates a copy so that a borrow of the
    /// copy can be handed over, and `&thing` was already that borrow. Take the
    /// reference to the thing itself.
    pub EXPLICIT030_NO_COPY_TO_LEND,
    Deny,
    "a copy allocated only so that a borrow of it could be handed over"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn makes_a_copy(named: &str) -> bool {
    matches!(named, "clone" | "to_owned" | "to_vec" | "to_string")
}

impl<'tcx> LateLintPass<'tcx> for Explicit030NoCopyToLend {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::AddrOf(_, _, copied) = expr.kind else {
            return;
        };

        let ExprKind::MethodCall(asked, held, taken, _) = copied.kind else {
            return;
        };

        if !taken.is_empty() {
            return;
        }

        if !makes_a_copy(asked.ident.name.as_str()) {
            return;
        }

        let copy = cx.typeck_results().expr_ty(copied).peel_refs();
        let thing = cx.typeck_results().expr_ty(held).peel_refs();

        // Different types means a coercion is doing work here, and what to
        // write instead depends on what the far side asked for.
        if copy != thing {
            return;
        }

        let named = asked.ident.name;

        span_lint_and_help(
            cx,
            EXPLICIT030_NO_COPY_TO_LEND,
            expr.span,
            format!("`{named}` allocates a copy of a `{thing}` so that a borrow of the copy can be handed over"),
            None,
            "take the reference to the thing itself. A copy that is kept -- stored, answered with, \
             pushed onto something -- is not written with an `&` in front of it, so nothing that is \
             needed is being asked for here",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
