//! Match primitive casts to shared conversions with identical rounding and range behavior.

use crate::boundary_conversion::{self, CRATE};
use clippy_utils::is_in_const_context;
use rustc_hir::{BindingMode, Expr, Node, PatKind, QPath, Ty, TyKind, def::Res};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, FloatTy, IntTy, UintTy};
use rustc_span::Span;

// The optional type widens the input to the helper's parameter type and
// pins the original input type even if the cast was constraining inference.
fn conversion(from: ty::Ty<'_>, to: ty::Ty<'_>) -> Option<(String, Option<&'static str>)> {
    let recipe = match (from.kind(), to.kind()) {
        (
            ty::Float(FloatTy::F32 | FloatTy::F64),
            ty::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize)
            | ty::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64 | UintTy::Usize),
        ) => {
            let widen = matches!(from.kind(), ty::Float(FloatTy::F32)).then_some("f64");
            return Some((format!("toward_zero_{to}"), widen));
        }
        (ty::Float(FloatTy::F64), ty::Float(FloatTy::F32)) => ("nearest_f32_from_f64", None),
        (ty::Int(IntTy::I32 | IntTy::I64), ty::Float(FloatTy::F32)) => (
            "nearest_f32_from_i64",
            (matches!(from.kind(), ty::Int(IntTy::I32))).then_some("i64"),
        ),
        (ty::Uint(UintTy::U32 | UintTy::U64), ty::Float(FloatTy::F32)) => (
            "nearest_f32_from_u64",
            (matches!(from.kind(), ty::Uint(UintTy::U32))).then_some("u64"),
        ),
        (ty::Int(IntTy::Isize), ty::Float(FloatTy::F32)) => ("nearest_f32_from_isize", None),
        (ty::Uint(UintTy::Usize), ty::Float(FloatTy::F32)) => ("nearest_f32_from_usize", None),
        _ => return None,
    };
    Some((recipe.0.to_owned(), recipe.1))
}

pub(super) fn edits(
    cx: &LateContext<'_>,
    cast: &Expr<'_>,
    operand: &Expr<'_>,
    target: &Ty<'_>,
) -> Option<Vec<(Span, String)>> {
    let TyKind::Path(QPath::Resolved(_, path)) = target.kind else {
        return None;
    };
    if !matches!(path.res, Res::PrimTy(_))
        || !cx.sess().edition().at_least_rust_2018()
        || is_in_const_context(cx)
        || [cast.span, operand.span, target.span]
            .iter()
            .any(|span| span.from_expansion())
    {
        return None;
    }

    let types = cx.typeck_results();
    let from = types.expr_ty(operand);
    if from != types.expr_ty_adjusted(operand) {
        return None;
    }
    let (helper, widen) = conversion(from, types.expr_ty(cast))?;
    if !boundary_conversion::available(cx, &helper, 0) {
        return None;
    }
    let map = cx.sess().source_map();
    let original = map.span_to_snippet(cast.span).ok()?;
    if original.contains("//") || original.contains("/*") {
        return None;
    }

    let mut edits = Vec::new();
    let mut opening = format!("::{CRATE}::{helper}(");
    let mut closing = ")".to_owned();
    if let Some(wider) = widen {
        opening.push_str(&format!(
            "<::core::primitive::{wider} as ::core::convert::From<::core::primitive::{from}>>::from("
        ));
        closing.push(')');
    }
    if let Node::LetStmt(local) = cx.tcx.parent_hir_node(cast.hir_id)
        && local.ty.is_none()
        && local.els.is_none()
        && matches!(
            local.pat.kind,
            PatKind::Binding(BindingMode::NONE | BindingMode::MUT, _, _, None)
        )
        && !local.pat.span.from_expansion()
    {
        let binding = map.span_to_snippet(local.pat.span).ok()?;
        edits.push((
            local.pat.span,
            format!("::core::result::Result::Ok({binding})"),
        ));
    } else {
        let (before, after) = boundary_conversion::extraction_named(cx, cast, "_converted_number");
        opening.insert_str(0, before);
        closing.push_str(&after);
    }

    edits.push((cast.span.with_hi(operand.span.lo()), opening));
    edits.push((cast.span.with_lo(operand.span.hi()), closing));
    Some(edits)
}
