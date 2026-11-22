#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

#[path = "../../shared/primitive_conversion.rs"]
mod primitive_conversion;
#[path = "../../shared/boundary_conversion.rs"]
mod boundary_conversion;
#[path = "../../shared/float_conversion.rs"]
mod float_conversion;
mod integer_cast;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::msrvs::{self, Msrv};
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind, QPath, Ty, TyKind, def::Res};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT011: `as` casts are forbidden. They silently truncate, widen,
    /// or reinterpret. Use `From` / `TryFrom` for value types, or
    /// `pointer::cast` / `&raw const` / `&raw mut` for pointers.
    pub EXPLICIT011_NO_AS_CAST,
    Deny,
    "`as` casts are forbidden; use `From`, `TryFrom`, or `pointer::cast`"
}

fn sign_cast<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    operand: &Expr<'tcx>,
    target: &Ty<'tcx>,
) -> Option<String> {
    // An alias may denote a different width on another target. Keep its name.
    let TyKind::Path(QPath::Resolved(_, path)) = target.kind else {
        return None;
    };
    if !matches!(path.res, Res::PrimTy(_))
        || expr.span.from_expansion()
        || target.span.from_expansion()
        || operand.span.from_expansion()
    {
        return None;
    }
    let types = cx.typeck_results();
    let from = types.expr_ty(operand);
    let to = types.expr_ty(expr);
    let method = match (from.kind(), to.kind()) {
        (ty::Int(a), ty::Uint(b)) if a.bit_width() == b.bit_width() => "cast_unsigned",
        (ty::Uint(a), ty::Int(b)) if a.bit_width() == b.bit_width() => "cast_signed",
        _ => return None,
    };
    let mut msrv = Msrv::default();
    msrv.read_cargo(cx.sess());
    if !msrv.meets(cx, msrvs::INTEGER_SIGN_CAST) {
        return None;
    }

    let map = cx.sess().source_map();
    let whole = map.span_to_snippet(expr.span).ok()?;
    if whole.contains("//") || whole.contains("/*") {
        return None;
    }
    let value = map.span_to_snippet(operand.span).ok()?;
    Some(format!("::core::primitive::{from}::{method}({value})"))
}

fn reference_coercion(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    operand: &Expr<'_>,
    target: &Ty<'_>,
) -> Option<String> {
    let ExprKind::Path(ref path) = operand.kind else { return None };
    if !matches!(cx.qpath_res(path, operand.hir_id), Res::Local(_))
        || !matches!(target.kind, TyKind::Ref(..))
        || [expr.span, operand.span, target.span].iter().any(|s| s.from_expansion())
    {
        return None;
    }
    let types = cx.typeck_results();
    let ty::Ref(_, source, ty::Mutability::Not) = types.expr_ty(operand).kind() else { return None };
    let ty::Ref(_, dest, ty::Mutability::Not) = types.expr_ty(expr).kind() else { return None };
    // A shared reference local is Copy. Passing it through a typed coercion
    // site preserves both method dispatch and the referent's lifetime.
    if !matches!(dest.kind(), ty::Dynamic(..))
        && !matches!((source.kind(), dest.kind()), (ty::Array(a, _), ty::Slice(b)) if a == b)
    {
        return None;
    }
    let mut msrv = Msrv::default();
    msrv.read_cargo(cx.sess());
    if !msrv.meets(cx, rustc_hir::RustcVersion { major: 1, minor: 33, patch: 0 }) {
        return None;
    }
    let map = cx.sess().source_map();
    let whole = map.span_to_snippet(expr.span).ok()?;
    if whole.contains("//") || whole.contains("/*") { return None; }
    let target = map.span_to_snippet(target.span).ok()?;
    let value = map.span_to_snippet(operand.span).ok()?;
    Some(format!("::core::convert::identity::<{target}>({value})"))
}

fn pointer_cast(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    operand: &Expr<'_>,
    target: &Ty<'_>,
) -> Option<String> {
    let ExprKind::Path(ref path) = operand.kind else { return None };
    let TyKind::Ptr(pointee) = target.kind else { return None };
    if !matches!(cx.qpath_res(path, operand.hir_id), Res::Local(_))
        || [expr.span, operand.span, target.span].iter().any(|span| span.from_expansion())
    {
        return None;
    }

    let types = cx.typeck_results();
    let ty::RawPtr(_, source_mutability) = types.expr_ty(operand).kind() else { return None };
    let ty::RawPtr(destination, target_mutability) = types.expr_ty(expr).kind() else { return None };
    if source_mutability != target_mutability || !destination.is_sized(cx.tcx, cx.typing_env()) {
        return None;
    }

    let mut msrv = Msrv::default();
    msrv.read_cargo(cx.sess());
    if !msrv.meets(cx, msrvs::POINTER_CAST) {
        return None;
    }

    let map = cx.sess().source_map();
    let whole = map.span_to_snippet(expr.span).ok()?;
    if whole.contains("//") || whole.contains("/*") {
        return None;
    }
    let variable = map.span_to_snippet(operand.span).ok()?;
    let pointee = map.span_to_snippet(pointee.ty.span).ok()?;

    Some(format!("{variable}.cast::<{pointee}>()"))
}

impl<'tcx> LateLintPass<'tcx> for Explicit011NoAsCast {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Cast(operand, target) = expr.kind else {
            return;
        };
        span_lint_and_then(
            cx,
            EXPLICIT011_NO_AS_CAST,
            expr.span,
            "`as` cast is forbidden",
            |diag| {
                if let Some(edit) =
                    primitive_conversion::replacement(cx, expr, operand, Some(target.span))
                {
                    diag.span_suggestion(
                        expr.span,
                        "name the lossless conversion",
                        edit,
                        Applicability::MachineApplicable,
                    );
                } else if let Some(edit) = sign_cast(cx, expr, operand, target) {
                    diag.span_suggestion(
                        expr.span,
                        "name the change of signedness",
                        edit,
                        Applicability::MachineApplicable,
                    );
                } else if let Some(edit) = reference_coercion(cx, expr, operand, target) {
                    diag.span_suggestion(
                        expr.span,
                        "coerce the shared reference to the written type",
                        edit,
                        Applicability::MachineApplicable,
                    );
                } else if let Some(edit) = pointer_cast(cx, expr, operand, target) {
                    diag.span_suggestion(
                        expr.span,
                        "name the raw pointer cast",
                        edit,
                        Applicability::MachineApplicable,
                    );
                } else if let Some(edits) = float_conversion::edits(cx, expr, operand, target) {
                    diag.multipart_suggestion(
                        "name the rounding policy and keep the cast's numeric behavior",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else if let Some(edits) = integer_cast::integer_cast(cx, expr, operand, target) {
                    diag.multipart_suggestion(
                        "keep the same low bits with a named integer conversion",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help("use `From` / `TryFrom` for value types; `pointer::cast`, `&raw const`, `&raw mut` for pointers");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
