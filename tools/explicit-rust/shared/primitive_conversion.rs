//! A source edit shared by the numeric conversion and cast diagnostics.

use clippy_utils::{is_in_const_context, ty::implements_trait};
use rustc_hir::Expr;
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{Span, sym};

pub fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    operand: &Expr<'tcx>,
    cast_target: Option<Span>,
) -> Option<String> {
    if is_in_const_context(cx) || expr.span.from_expansion() || operand.span.from_expansion() {
        return None;
    }
    let types = cx.typeck_results();
    let from = types.expr_ty(operand);
    let to = types.expr_ty(expr);
    let primitive = |t: ty::Ty<'_>| {
        matches!(
            t.kind(),
            ty::Int(_) | ty::Uint(_) | ty::Float(_) | ty::Bool | ty::Char
        )
    };
    if !primitive(from) || !primitive(to) || from == to || from != types.expr_ty_adjusted(operand) {
        return None;
    }
    let from_trait = cx.tcx.get_diagnostic_item(sym::From)?;
    if !implements_trait(cx, to, from_trait, &[from.into()]) {
        return None;
    }

    let map = cx.sess().source_map();
    let whole = map.span_to_snippet(expr.span).ok()?;
    if whole.contains("//") || whole.contains("/*") {
        return None;
    }
    let value = map.span_to_snippet(operand.span).ok()?;
    let mut destination = format!("::core::primitive::{to}");
    if let Some(span) = cast_target {
        if span.from_expansion() {
            return None;
        }
        let written = map.span_to_snippet(span).ok()?;
        if written != "_" {
            destination = written;
        }
    }
    // Pin both types: the operand can be an unsuffixed literal or a generic call.
    Some(format!(
        "<{destination} as ::core::convert::From<::core::primitive::{from}>>::from({value})"
    ))
}
