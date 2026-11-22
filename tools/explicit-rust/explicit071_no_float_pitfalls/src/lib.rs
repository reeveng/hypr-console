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

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::consts::{ConstEvalCtxt, Constant};
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::sugg::Sugg;
use rustc_errors::Applicability;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{BinOpKind, Expr, ExprKind, Node, UnOp};
#[path = "../../shared/macro_input.rs"]
mod macro_input;
use macro_input::outside_macro_input;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{FloatTy, TyKind};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT071: `==` and `!=` on floats answer as though every value were
    /// a value, and a `NaN` on either side quietly decides otherwise. Ask
    /// with `total_cmp`, or assert the bit pattern with `to_bits`.
    pub EXPLICIT071_NO_FLOAT_PITFALLS,
    Warn,
    "floats compared with `==` or `!=`; use `total_cmp`, or compare the bits on purpose"
}

fn literal(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<f64> {
    let operand = match expr.kind {
        ExprKind::Unary(UnOp::Neg, operand) => operand,
        _ => expr,
    };
    if expr.span.from_expansion() || !matches!(operand.kind, ExprKind::Lit(_)) {
        return None;
    }
    match ConstEvalCtxt::new(cx).eval(expr)? {
        Constant::F32(value) => Some(f64::from(value)),
        Constant::F64(value) => Some(value),
        _ => None,
    }
}

fn literal_comparison<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'_>,
    lhs: &'tcx Expr<'tcx>,
    rhs: &'tcx Expr<'tcx>,
    op: BinOpKind,
) -> Option<Vec<(Span, String)>> {
    let types = cx.typeck_results();
    let ty = types.expr_ty(lhs);
    let name = match ty.kind() {
        TyKind::Float(FloatTy::F32) => "::core::primitive::f32",
        TyKind::Float(FloatTy::F64) => "::core::primitive::f64",
        _ => return None,
    };
    if ty != types.expr_ty(rhs)
        || !cx.sess().edition().at_least_rust_2018()
        || lhs.span.from_expansion()
        || rhs.span.from_expansion()
        || !outside_macro_input(cx, expr.span)
    {
        return None;
    }
    let map = cx.sess().source_map();
    let (number, other) = literal(cx, rhs)
        .map(|n| (n, lhs))
        .or_else(|| literal(cx, lhs).map(|n| (n, rhs)))?;
    if number == 0.0 {
        // classify recognizes both signs of zero. Every NaN is a nonzero category.
        let source = map.span_to_snippet(expr.span).ok()?;
        if source.contains("//") || source.contains("/*") {
            return None;
        }
        let value = map.span_to_snippet(other.span).ok()?;
        let classified =
            format!("::core::matches!({name}::classify({value}), ::core::num::FpCategory::Zero)");
        let replacement = if op == BinOpKind::Eq {
            classified
        } else if matches!(cx.tcx.parent_hir_node(expr.hir_id), Node::Expr(parent)
            if matches!(parent.kind, ExprKind::MethodCall(..) | ExprKind::Cast(..) | ExprKind::Field(..)))
        {
            format!("(!{classified})")
        } else {
            format!("!{classified}")
        };
        return Some(vec![(expr.span, replacement)]);
    }
    // Every nonzero, non-NaN number has exactly one representation.
    if number.is_nan() {
        return None;
    }
    [lhs, rhs]
        .into_iter()
        .map(|operand| {
            let text = map.span_to_snippet(operand.span).ok()?;
            Some((operand.span, format!("{name}::to_bits({text})")))
        })
        .collect()
}

// A field access may run user code through Deref. Borrowing a packed field
// would also impose an alignment requirement the original comparison lacks.
fn scalar_read<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    let types = cx.typeck_results();
    match expr.kind {
        ExprKind::Lit(_) => true,
        ExprKind::Path(ref path) => match cx.qpath_res(path, expr.hir_id) {
            Res::Local(_) | Res::Def(DefKind::Const { .. } | DefKind::AssocConst { .. }, _) => {
                true
            }
            Res::Def(DefKind::Static { mutability, .. }, _) => !mutability.is_mut(),
            _ => false,
        },
        ExprKind::Field(base, _) => {
            let ty = types.expr_ty(base).peel_refs();
            (matches!(ty.kind(), TyKind::Tuple(_))
                || ty.ty_adt_def().is_some_and(|adt| !adt.repr().packed()))
                && ty == types.expr_ty_adjusted(base).peel_refs()
                && scalar_read(cx, base)
        }
        ExprKind::Unary(UnOp::Deref, base) => {
            types.expr_ty(base).is_ref() && scalar_read(cx, base)
        }
        ExprKind::MethodCall(method, receiver, [], _)
            if matches!(method.ident.as_str(), "abs" | "trunc")
                && types.expr_ty(receiver).is_floating_point()
                && let Some(id) = types.type_dependent_def_id(expr.hir_id)
                && matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "std")
                && cx.tcx.trait_of_assoc(id).is_none() =>
        {
            scalar_read(cx, receiver)
        }
        _ => false,
    }
}

fn ordered_comparison<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    lhs: &'tcx Expr<'tcx>,
    rhs: &'tcx Expr<'tcx>,
    op: BinOpKind,
) -> Option<String> {
    if clippy_utils::is_in_const_context(cx)
        || !cx.sess().edition().at_least_rust_2018()
        || [lhs, rhs]
            .into_iter()
            .any(|operand| operand.span.from_expansion() || !scalar_read(cx, operand))
        || !outside_macro_input(cx, expr.span)
    {
        return None;
    }
    let map = cx.sess().source_map();
    let comparison = map.span_to_snippet(expr.span).ok()?;
    if comparison.contains("//") || comparison.contains("/*") {
        return None;
    }
    let types = cx.typeck_results();
    let float = match types.expr_ty(lhs).peel_refs().kind() {
        TyKind::Float(FloatTy::F32) => "f32",
        TyKind::Float(FloatTy::F64) => "f64",
        _ => return None,
    };
    let reference = |operand: &Expr<'_>| {
        if types.expr_ty(operand).is_ref() {
            map.span_to_snippet(operand.span).ok()
        } else {
            Some(Sugg::hir_opt(cx, operand)?.addr().to_string())
        }
    };
    let left = reference(lhs)?;
    let right = reference(rhs)?;
    Some(format!(
        "<::core::primitive::{float} as ::core::cmp::PartialOrd>::partial_cmp({left}, {right}) {} ::core::option::Option::Some(::core::cmp::Ordering::Equal)",
        op.as_str()
    ))
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

        if !left.peel_refs().is_floating_point() || left.peel_refs() != right.peel_refs() {
            return;
        }

        span_lint_and_then(
            cx,
            EXPLICIT071_NO_FLOAT_PITFALLS,
            expr.span,
            "float equality and inequality have an unordered case when either operand is NaN",
            |diagnostic| {
                if let Some(edits) = literal_comparison(cx, expr, lhs, rhs, op.node) {
                    diagnostic.multipart_suggestion(
                        "compare this literal explicitly, preserving signed zero and NaN behavior",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else if let Some(replacement) = ordered_comparison(cx, expr, lhs, rhs, op.node)
                {
                    diagnostic.span_suggestion(
                        expr.span,
                        "compare the partial ordering explicitly, keeping the unordered NaN case",
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("ask with `total_cmp`, which is total over every value the type holds, or -- where \
             the comparison is about the bit pattern -- compare `to_bits()` on purpose. Where \
             both sides are known good by construction, allow this rule here and let the reason \
             say so");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}

#[test]
fn literal_comparisons() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
