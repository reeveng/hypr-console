//! EXPLICIT025: a value may not carry a secret meaning.
//!
//! `-1` for invalid, `0` for nothing, `MAX` for unlimited. A sentinel is a
//! variant no one declared: the type says `i32` and the code says one of these
//! numbers is not a number at all, and because the variant was never declared
//! nothing counts the arms of it and nothing fails when a second one is needed.
//! The day *unlimited* has to be told apart from *unset*, there is no compiler
//! anywhere in the change -- only every comparison in the tree, read by hand.
//!
//! `Option` says absence and an enum says unlimited, and the compiler checks
//! both. That is the whole of the fix, and it is the same argument EXPLICIT016
//! makes about a wildcard arm: a case that exists but was never named is a
//! decision made by omission.
//!
//! EXPLICIT015 has already made this rarer than it would otherwise be.
//! Arithmetic that must name its policy reaches for `saturating_add` rather
//! than for a hand-written clamp against `MAX`, so the sentinels that survive
//! are the ones someone meant.
//!
//! What is narrow enough to ask for: a `MAX` or `MIN` associated constant, or
//! a negative integer literal, standing as the operand of a comparison or as a
//! constant a function answers with. Those are the two places a sentinel is
//! read and the one place it is written, and a number that never meets a
//! comparison is a number rather than a meaning.
//!
//! Saturation is where the false positives are. `saturating_sub` returning
//! `MIN` and a caller asking whether it did is a real thing to write, and so is
//! a bound checked against the end of a type. Both are the site carrying the
//! allow, with EXPLICIT018 asking the reason to say which -- and a reason that
//! cannot be written is the rule finding what it was looking for.
//!
//! A negative float is left alone. A negative integer beside a comparison is
//! nearly always a meaning; a negative float beside one is nearly always a
//! coordinate, and a rule that cried about geometry would be turned off.

//!
//! It arrived `Deny`, which no rule here has managed since 018. The tree was
//! already keeping it, and for the reason the argument gives: EXPLICIT015 sent
//! every clamp to `saturating_*` before anyone thought to write this down.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::source::snippet;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{BinOpKind, Body, Expr, ExprKind, FnDecl, UnOp};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

extern crate rustc_span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT025: a sentinel value is a variant no one declared. `-1` for
    /// invalid and `usize::MAX` for unlimited are cases the type does not
    /// have, so nothing counts them and nothing fails when a second one is
    /// needed. `Option` says absence and an enum says unlimited, and the
    /// compiler checks both.
    pub EXPLICIT025_NO_SENTINEL_VALUE,
    Deny,
    "a value standing for a meaning the type does not declare; say it with `Option` or an enum"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The end of a type, named as a constant: `usize::MAX`, `i32::MIN`, and the
// same two reached through an alias. Read off the resolution rather than the
// spelling, so a `use` of either does not slip past.
fn is_end_of_type(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let ExprKind::Path(ref qpath) = expr.kind else {
        return None;
    };

    let Res::Def(DefKind::AssocConst { .. }, id) = cx.qpath_res(qpath, expr.hir_id) else {
        return None;
    };

    let named = cx.tcx.item_name(id);

    // The resolved path of `usize::MAX` prints as the impl it was declared in,
    // which is not what anyone wrote. What was written is on the screen.
    match named.as_str() {
        "MAX" | "MIN" => Some(format!("`{}`", snippet(cx, expr.span, "the end of the type"))),
        _ => None,
    }
}

// `-1` is one expression in the source and two nodes in the tree: a negation
// over a literal. EXPLICIT015 leaves it alone because it is how a number is
// written; this rule is about what the number is being asked to mean.
fn is_negative_whole_number(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let ExprKind::Unary(UnOp::Neg, operand) = expr.kind else {
        return None;
    };

    let ExprKind::Lit(_) = operand.kind else {
        return None;
    };

    match cx.typeck_results().expr_ty(expr).peel_refs().is_integral() {
        true => Some("a negative number".to_string()),
        false => None,
    }
}

fn stands_for_something(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    is_end_of_type(cx, expr).or_else(|| is_negative_whole_number(cx, expr))
}

fn is_comparison(op: BinOpKind) -> bool {
    matches!(
        op,
        BinOpKind::Eq | BinOpKind::Ne | BinOpKind::Lt | BinOpKind::Le | BinOpKind::Gt | BinOpKind::Ge
    )
}

fn say(cx: &LateContext<'_>, at: Span, named: &str, where_it_stands: &str) {
    span_lint_and_help(
        cx,
        EXPLICIT025_NO_SENTINEL_VALUE,
        at,
        format!("{named} {where_it_stands}, which makes it a case the type does not declare"),
        None,
        "say the case with a type that has it: `Option` for a value that may not be there, an enum \
         for `Unlimited` or `Unset`. Where the end of the type is the answer rather than a stand-in \
         -- a saturating result met on purpose, a bound checked against what the type can hold -- \
         allow this rule here and let the reason say which",
    );
}

// The tail of a body, peeled to the expression the function answers with. A
// block's value is its tail; anything else is the expression itself.
fn answered_with<'tcx>(body: &'tcx Body<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    match body.value.kind {
        ExprKind::Block(block, _) => block.expr,
        _ => Some(body.value),
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit025NoSentinelValue {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        match expr.kind {
            ExprKind::Binary(op, lhs, rhs) => {
                match is_comparison(op.node) {
                    true => {
                        for side in [lhs, rhs] {
                            match stands_for_something(cx, side) {
                                Some(named) => say(cx, side.span, &named, "is compared against"),
                                None => {}
                            }
                        }
                    }
                    false => {}
                }
            }
            ExprKind::Ret(Some(answer)) => match stands_for_something(cx, answer) {
                Some(named) => say(cx, answer.span, &named, "is handed back as an answer"),
                None => {}
            },
            _ => {}
        }
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        if is_test_build(cx) {
            return;
        }

        if matches!(kind, FnKind::Closure) {
            return;
        }

        let Some(answer) = answered_with(body) else {
            return;
        };

        if answer.span.from_expansion() {
            return;
        }

        match stands_for_something(cx, answer) {
            Some(named) => say(cx, answer.span, &named, "is handed back as an answer"),
            None => {}
        }

        let _ = (decl, span, def_id);
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
