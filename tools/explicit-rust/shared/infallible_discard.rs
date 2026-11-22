//! Discard an infallible result by acknowledging its sole possible variant.
use clippy_utils::sym;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};

// Calls produce temporaries. A place used as a statement can be moved and
// dropped, and a wildcard let can accept an uninitialized place.
pub fn is_call(expr: &Expr<'_>) -> bool {
    matches!(expr.kind, ExprKind::Call(..) | ExprKind::MethodCall(..))
}

pub fn discard_pattern(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<&'static str> {
    let ty::Adt(result, args) = ty.kind() else {
        return None;
    };
    if !cx.sess().edition().at_least_rust_2018()
        || !cx.tcx.is_diagnostic_item(sym::Result, result.did())
    {
        return None;
    }
    match args.type_at(1).kind() {
        ty::Never => {}
        ty::Adt(error, _)
            if error.is_enum()
                && error.variants().is_empty()
                && !error.is_variant_list_non_exhaustive() => {}
        _ => return None,
    }
    Some(if args.type_at(0).is_unit() {
        "::core::result::Result::Ok(())"
    } else {
        "::core::result::Result::Ok(_)"
    })
}
