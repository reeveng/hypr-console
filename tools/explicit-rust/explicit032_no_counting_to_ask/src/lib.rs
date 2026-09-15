//! EXPLICIT032: nothing is counted to find out whether there is anything.
//!
//! `held.filter(wanted).count() > 0` walks every element of the list to learn
//! one bit, and it had the answer at the first element that matched. `.any(…)`
//! stops there. On a list of six this is nothing; on an iterator that reads a
//! directory, decodes a file or asks another process it is the whole cost of
//! the line, paid to find out something that was already known.
//!
//! `Iterator::count` is the one to watch rather than `len`. A `len` is a field
//! on a list and answering it costs nothing, and where somebody has written
//! `held.len() == 0` stock clippy already asks for `is_empty` -- that rule is on
//! in this workspace and this one does not repeat it. A `count` is a walk: it
//! consumes the iterator to the end, by definition, and the definition is the
//! part nobody reads.
//!
//! What is asked for is a comparison of a `count` against nothing or one, which
//! is a question about emptiness wearing a number. `> 0` and `!= 0` are
//! `.next().is_some()`, or `.any(…)` where a `filter` stands in front of the
//! count; `== 0` is `.next().is_none()`, or `.all(…)` with the test turned
//! around; `< 1` and `>= 1` are the same two written the other way about.
//!
//! A count compared against a real number is left alone. Asking whether there
//! are more than three of something does mean counting them, and counting is
//! what the call is for.

//!
//! It arrived `Warn` with a short distance in front of it. Both of the places it
//! found are a `split_whitespace().count()` asked whether there was more than
//! one word, which is `.nth(1).is_some()` and stops at the second.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::LitKind;
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT032: comparing `.count()` against nothing or one walks the
    /// whole iterator to answer a yes or a no. `.any(…)` and
    /// `.next().is_some()` stop at the first element that settles it.
    pub EXPLICIT032_NO_COUNTING_TO_ASK,
    Deny,
    "an iterator walked to its end to answer whether it holds anything"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A count of an iterator, and whether a `filter` stands in front of it -- which
// decides whether the answer is `any` or `next`.
fn counts_a_walk<'tcx>(expr: &'tcx Expr<'tcx>) -> Option<&'static str> {
    let ExprKind::MethodCall(asked, walked, taken, _) = expr.kind else {
        return None;
    };

    if !taken.is_empty() {
        return None;
    }

    match asked.ident.name.as_str() {
        "count" => {}
        _ => return None,
    }

    let filtered = matches!(
        walked.kind,
        ExprKind::MethodCall(sifted, ..) if matches!(sifted.ident.name.as_str(), "filter" | "filter_map")
    );

    match filtered {
        true => Some("`.any(…)` asks the same thing and stops at the first element that answers it"),
        false => Some("`.next().is_some()` and `.next().is_none()` answer this at the first element"),
    }
}

// Nothing, or one: the two numbers a count is compared against when the
// question was never about a number.
fn is_nothing_or_one(expr: &Expr<'_>) -> bool {
    let ExprKind::Lit(written) = expr.kind else {
        return false;
    };

    matches!(written.node, LitKind::Int(said, _) if said.get() <= 1)
}

fn is_comparison(op: BinOpKind) -> bool {
    matches!(
        op,
        BinOpKind::Eq | BinOpKind::Ne | BinOpKind::Lt | BinOpKind::Le | BinOpKind::Gt | BinOpKind::Ge
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit032NoCountingToAsk {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Binary(op, lhs, rhs) = expr.kind else {
            return;
        };

        if !is_comparison(op.node) {
            return;
        }

        let counted = match (counts_a_walk(lhs), is_nothing_or_one(rhs)) {
            (Some(instead), true) => Some((lhs, instead)),
            _ => match (counts_a_walk(rhs), is_nothing_or_one(lhs)) {
                (Some(instead), true) => Some((rhs, instead)),
                _ => None,
            },
        };

        let Some((counted, instead)) = counted else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT032_NO_COUNTING_TO_ASK,
            expr.span,
            "this walks the whole iterator to answer whether it holds anything",
            None,
            format!(
                "{instead}. A count is a walk to the end by definition, and the definition is the part \
                 that does not show on the line"
            ),
        );

        let _ = counted;
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
