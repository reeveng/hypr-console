use clippy_utils::{is_in_const_context, msrvs::Msrv};
use rustc_hir::{
    Expr, ExprKind, QPath, Ty, TyKind,
    def::{DefKind, Res},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};

use crate::boundary_conversion;

pub fn integer_cast<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &Expr<'tcx>,
    operand: &Expr<'tcx>,
    target: &Ty<'tcx>,
) -> Option<Vec<(Span, String)>> {
    if !cx.sess().edition().at_least_rust_2018()
        || expression.span.from_expansion()
        || target.span.from_expansion()
    {
        return None;
    }
    let mut operand_span = operand.span.source_callsite();
    if !expression.span.contains(operand_span) || operand_span.hi() > target.span.lo() {
        return None;
    }
    let mut msrv = Msrv::default();
    msrv.read_cargo(cx.sess());
    if !msrv.meets(
        cx,
        rustc_hir::RustcVersion {
            major: 1,
            minor: 43,
            patch: 0,
        },
    ) {
        return None;
    }

    let types = cx.typeck_results();
    let before = types.expr_ty(operand);
    let after = types.expr_ty(expression);
    if !(before.is_bool() || before.is_integral())
        || !after.is_integral()
        || before != types.expr_ty_adjusted(operand)
    {
        return None;
    }

    let map = cx.sess().source_map();
    let source = map.span_to_snippet(expression.span).ok()?;
    if source.contains("//") || source.contains("/*") {
        return None;
    }
    let (destination, primitive) = match target.kind {
        TyKind::Infer(()) => (format!("::core::primitive::{after}"), true),
        TyKind::Path(QPath::Resolved(_, path)) => match path.res {
            Res::PrimTy(_) => (format!("::core::primitive::{after}"), true),
            Res::Def(DefKind::TyAlias, _) => (map.span_to_snippet(target.span).ok()?, false),
            _ => return None,
        },
        _ => return None,
    };

    let (prefix, suffix) = if before.is_bool() {
        (
            format!("::core::convert::identity::<{destination}>(match "),
            " { true => 1, false => 0 })".into(),
        )
    } else if before == after {
        (
            format!("::core::convert::identity::<{destination}>("),
            ")".into(),
        )
    } else if is_in_const_context(cx) {
        // A binary expression needs grouping as a cast operand. The macro
        // parses an expression argument, where that outer pair is redundant.
        if matches!(operand.kind, ExprKind::Binary(..))
            && !operand.span.from_expansion()
            && map
                .span_to_snippet(operand_span)
                .is_ok_and(|text| text.starts_with('(') && text.ends_with(')'))
        {
            operand_span = operand_span
                .with_lo(operand_span.lo() + BytePos(1))
                .with_hi(operand_span.hi() - BytePos(1));
        }
        const_wrapping(cx, before, &destination)?
    } else {
        wrapping(cx, expression, after, &destination, primitive)?
    };

    // Keep the operand's source in place so rustfix can also update a nested
    // cast, or an operation another lint identified within the operand.
    Some(vec![
        (expression.span.with_hi(operand_span.lo()), prefix),
        (expression.span.with_lo(operand_span.hi()), suffix),
    ])
}

fn const_wrapping(
    cx: &LateContext<'_>,
    before: ty::Ty<'_>,
    destination: &str,
) -> Option<(String, String)> {
    let (signedness, helper) = match before.kind() {
        ty::Int(_) => ("signed", "resize_signed_le_bytes"),
        ty::Uint(_) => ("unsigned", "resize_unsigned_le_bytes"),
        _ => return None,
    };
    let conversion = boundary_conversion::CRATE;
    if !boundary_conversion::available(cx, helper, 2)
        || !cx.tcx.crates(()).iter().any(|id| {
            cx.tcx.crate_name(*id).as_str() == conversion
                && cx.tcx.module_children(id.as_def_id()).iter().any(|child| {
                    child.vis.is_public()
                        && child.ident.as_str() == "const_wrapped_integer"
                        && matches!(child.res, Res::Def(DefKind::Macro(_), _))
                })
        })
    {
        return None;
    }

    Some((
        format!(
            "::{conversion}::const_wrapped_integer!({signedness} ::core::primitive::{before} => {destination}, "
        ),
        ")".into(),
    ))
}

fn wrapping(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    after: ty::Ty<'_>,
    destination: &str,
    primitive: bool,
) -> Option<(String, String)> {
    let (name, parameters, generics) = match (after.kind(), primitive) {
        (ty::Uint(ty::UintTy::Usize), true) => ("wrapped_index", 1, String::new()),
        (ty::Int(ty::IntTy::Isize), true) => ("wrapped_offset", 1, String::new()),
        _ => ("wrapped", 2, format!("::<_, {destination}>")),
    };
    if !boundary_conversion::available(cx, name, parameters) {
        return None;
    }

    let (opening, closing) = boundary_conversion::extraction(cx, expression);
    let conversion = boundary_conversion::CRATE;
    Some((
        format!("{opening}::{conversion}::{name}{generics}("),
        format!("){closing}"),
    ))
}
