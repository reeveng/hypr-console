//! Fold the wrapping compound-assignment expansion into the numeric boundary.

use super::{CONVERSION_CRATE, boundary_conversion, fixes::clean};
use clippy_utils::{sym, visitors::is_local_used};
use rustc_hir::{
    BinOpKind, BindingMode, BorrowKind, Expr, ExprKind, HirId, LangItem, LetStmt, Mutability, Node,
    PatKind, StmtKind, UnOp, def::Res,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

pub(super) fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    local: &'tcx LetStmt<'tcx>,
) -> Option<(Span, String)> {
    if !cx.sess().edition().at_least_rust_2024()
        || clippy_utils::is_in_const_context(cx)
        || local.els.is_some()
    {
        return None;
    }
    let PatKind::Binding(BindingMode::NONE, target, _, None) = local.pat.kind else {
        return None;
    };
    let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, place) = local.init?.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let width = types.expr_ty(place);
    let suffix = match width.kind() {
        ty::Uint(ty::UintTy::Usize) => "index",
        ty::Int(ty::IntTy::Isize) => "offset",
        _ => return None,
    };

    let Node::Stmt(target_statement) = cx.tcx.parent_hir_node(local.hir_id) else {
        return None;
    };
    let Node::Block(block) = cx.tcx.parent_hir_node(target_statement.hir_id) else {
        return None;
    };
    let Node::Expr(expression) = cx.tcx.parent_hir_node(block.hir_id) else {
        return None;
    };
    let Node::Stmt(statement) = cx.tcx.parent_hir_node(expression.hir_id) else {
        return None;
    };
    let [amount_statement, borrow, update] = block.stmts else {
        return None;
    };
    if !matches!(expression.kind, ExprKind::Block(_, None))
        || !matches!(statement.kind, StmtKind::Semi(_))
        || block.expr.is_some()
        || borrow.hir_id != target_statement.hir_id
        || [
            expression.hir_id,
            statement.hir_id,
            amount_statement.hir_id,
            borrow.hir_id,
            update.hir_id,
        ]
        .into_iter()
        .any(|id| !cx.tcx.hir_attrs(id).is_empty())
    {
        return None;
    }
    let StmtKind::Let(amount) = amount_statement.kind else {
        return None;
    };
    if amount.ty.is_some() || amount.els.is_some() {
        return None;
    }
    let (binding, unwraps_result) = match amount.pat.kind {
        PatKind::TupleStruct(ref path, [binding], _)
            if matches!(cx.qpath_res(path, amount.pat.hir_id), Res::Def(_, id)
                if clippy_utils::is_lang_item_or_ctor(cx, id, LangItem::ResultOk)) =>
        {
            (binding, true)
        }
        _ => (amount.pat, false),
    };
    let PatKind::Binding(BindingMode::NONE, amount_id, _, None) = binding.kind else {
        return None;
    };
    if types.node_type(amount_id) != width || is_local_used(cx, place, amount_id) {
        return None;
    }
    let update_span = update.span;
    let StmtKind::Semi(update) = update.kind else {
        return None;
    };
    let ExprKind::Assign(lhs, rhs, _) = update.kind else {
        return None;
    };
    if !is_deref_of(cx, lhs, target) {
        return None;
    }
    let operation = wrapping_operation(cx, rhs, target, amount_id, width)?;
    let helper = format!("wrapping_{operation}_{suffix}");
    if !boundary_conversion::available(cx, &helper, 0) {
        return None;
    }

    clean(cx, expression.span)?;
    let input = amount.init?;
    let inline_amount =
        !unwraps_result && matches!(input.kind, ExprKind::Path(_) | ExprKind::Lit(_));
    let (replacement_span, amount_source, terminator) = if inline_amount {
        (expression.span, clean(cx, input.span)?, "")
    } else {
        // Keep the initializer's statement scope so its temporaries are
        // dropped before the mutable place is evaluated.
        (
            borrow.span.with_hi(update_span.hi()),
            clean(cx, binding.span)?,
            ";",
        )
    };
    let place_source = clean(cx, place.span)?;
    let place_source = match place.kind {
        ExprKind::Path(_) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Unary(..) => {
            place_source
                .strip_prefix('(')
                .and_then(|inner| inner.strip_suffix(')'))
                .unwrap_or(&place_source)
                .to_owned()
        }
        _ => format!("({place_source})"),
    };
    Some((
        replacement_span,
        format!(
            "let ::core::result::Result::Ok(()) = ::{CONVERSION_CRATE}::{helper}({amount_source}, &mut {place_source}){terminator}",
        ),
    ))
}

fn is_local(cx: &LateContext<'_>, expression: &Expr<'_>, id: HirId) -> bool {
    matches!(expression.kind, ExprKind::Path(ref path)
        if cx.qpath_res(path, expression.hir_id) == Res::Local(id))
}

fn is_deref_of(cx: &LateContext<'_>, expression: &Expr<'_>, id: HirId) -> bool {
    matches!(expression.kind, ExprKind::Unary(UnOp::Deref, operand) if is_local(cx, operand, id))
}

fn wrapping_operation<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &Expr<'tcx>,
    target: HirId,
    amount: HirId,
    width: ty::Ty<'tcx>,
) -> Option<&'static str> {
    let types = cx.typeck_results();
    let (method, left, right) = match expression.kind {
        ExprKind::Call(callee, [left, right]) => {
            let ty::FnDef(id, _) = *types.expr_ty(callee).kind() else {
                return None;
            };
            (id, left, right)
        }
        ExprKind::MethodCall(_, left, [right], _) => {
            (types.type_dependent_def_id(expression.hir_id)?, left, right)
        }
        ExprKind::Field(binary, field) if field.name.as_str() == "0" => {
            let ExprKind::Binary(op, left, right) = binary.kind else {
                return None;
            };
            let left = wrapping_operand(cx, left)?;
            let right = wrapping_operand(cx, right)?;
            if !is_deref_of(cx, left, target) || !is_local(cx, right, amount) {
                return None;
            }
            return match op.node {
                BinOpKind::Add => Some("add"),
                BinOpKind::Sub => Some("sub"),
                BinOpKind::Mul => Some("mul"),
                _ => None,
            };
        }
        _ => return None,
    };
    let implementation = cx.tcx.inherent_impl_of_assoc(method)?;
    if cx.tcx.crate_name(method.krate).as_str() != "core"
        || cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .skip_norm_wip()
            != width
        || !is_deref_of(cx, left, target)
        || !is_local(cx, right, amount)
    {
        return None;
    }
    match cx.tcx.item_name(method).as_str() {
        "wrapping_add" => Some("add"),
        "wrapping_sub" => Some("sub"),
        "wrapping_mul" => Some("mul"),
        _ => None,
    }
}

fn wrapping_operand<'tcx>(
    cx: &LateContext<'tcx>,
    wrapped: &Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let ExprKind::Call(callee, [value]) = wrapped.kind else {
        return None;
    };
    let ExprKind::Path(ref path) = callee.kind else {
        return None;
    };
    let Res::Def(rustc_hir::def::DefKind::Ctor(..), constructor) =
        cx.qpath_res(path, callee.hir_id)
    else {
        return None;
    };
    cx.tcx
        .is_diagnostic_item(sym::Wrapping, cx.tcx.parent(constructor))
        .then_some(value)
}
