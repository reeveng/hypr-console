//! EXPLICIT071: floats are compared by asking, not by asserting.
//!
//! A float is an `Option` pretending to be a number: the answer it cannot
//! give -- "not a value" -- is spelled `NaN`, and `NaN == NaN` is `false`,
//! which is the quietest way a comparison has ever failed. `==` and `!=` on
//! floats answer as though every value were a value, and a single `NaN` on
//! either side turns a comparison somebody meant into an answer nobody
//! intended -- in a sort, a binary search, a `HashMap` key, a boundary.
//!
//! The spelling this rule asks for is the one `std` wrote for exactly this:
//! `f64::total_cmp`, which orders the NaNs and is a total function, so the
//! comparison says what it means on every value the type can hold. A
//! comparison that is really asking "is this the same bit pattern" is
//! `to_bits() == to_bits()`, which is an assertion about representation
//! rather than about numbers and reads as one.
//!
//! What the rule asks of a boundary -- a value read from a file, a device, a
//! protocol -- is the other half: a `NaN` at a boundary is not a value, and
//! meeting it (`is_nan`) at the edge is the decision; letting it through is
//! the fault EXPLICIT064 is already worried about, arriving in a float.
//!
//! It arrives `Warn`, because drawing code in this tree does compare floats
//! where both sides are known good, and each of those wants either
//! `total_cmp` or the sentence saying why `==` is safe here.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT071: `==` and `!=` on floats answer as though every value were
    /// a value, and a `NaN` on either side quietly decides otherwise. Ask
    /// with `total_cmp`, or assert the bit pattern with `to_bits`.
    pub EXPLICIT071_NO_FLOAT_PITFALLS,
    Warn,
    "floats compared with `==` or `!=`; use `total_cmp`, or compare the bits on purpose"
}

impl<'tcx> LateLintPass<'tcx> for Explicit071NoFloatPitfalls {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Binary(op, lhs, rhs) = expr.kind else {
            return;
        };

        if !matches!(op.node, BinOpKind::Eq | BinOpKind::Ne) {
            return;
        }

        let left = cx.typeck_results().expr_ty(lhs);
        let right = cx.typeck_results().expr_ty(rhs);

        if !left.peel_refs().is_floating_point() && !right.peel_refs().is_floating_point() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT071_NO_FLOAT_PITFALLS,
            expr.span,
            "floats compared with `==`, which a `NaN` on either side quietly answers `false` for",
            None,
            "ask with `total_cmp`, which is total over every value the type holds, or -- where \
             the comparison is about the bit pattern -- compare `to_bits()` on purpose. Where \
             both sides are known good by construction, allow this rule here and let the reason \
             say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
