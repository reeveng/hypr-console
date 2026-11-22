//! Keep binary search's choice and use its successful position with `get`.

use super::fixes::clean;
use super::recovery::{binding_name, borrow_base, dereference, slice_like};
use clippy_utils::is_lang_item_or_ctor;
use clippy_utils::res::MaybeResPath;
use rustc_hir::{
    Arm, BindingMode, BorrowKind, ClosureKind, Expr, ExprKind, FnRetTy, HirId, LangItem,
    MatchSource, Mutability, Node, PatKind,
    def::{DefKind, Res},
};
use rustc_lint::LateContext;
use rustc_middle::ty::{
    self,
    adjustment::{Adjust, AutoBorrow, AutoBorrowMutability, DerefAdjustKind},
};
use rustc_span::{Span, Symbol, sym};

pub(super) fn inherent_receiver<'tcx>(cx: &LateContext<'tcx>, expression: &Expr<'_>) -> Option<ty::Ty<'tcx>> {
    let method = cx
        .typeck_results()
        .type_dependent_def_id(expression.hir_id)?;
    if !matches!(cx.tcx.crate_name(method.krate).as_str(), "core" | "alloc") {
        return None;
    }
    let implementation = cx.tcx.inherent_impl_of_assoc(method)?;
    Some(
        cx.tcx
            .type_of(implementation)
            .instantiate_identity()
            .skip_norm_wip(),
    )
}

fn method_of(cx: &LateContext<'_>, expression: &Expr<'_>, container: Symbol) -> bool {
    matches!(inherent_receiver(cx, expression).map(|kind| *kind.kind()), Some(ty::Adt(adt, _))
        if cx.tcx.is_diagnostic_item(container, adt.did()))
}

pub(super) fn same_place(cx: &LateContext<'_>, searched: &Expr<'_>, indexed: &Expr<'_>) -> bool {
    let types = cx.typeck_results();
    if searched.span.from_expansion()
        || indexed.span.from_expansion()
        || !searched.span.eq_ctxt(indexed.span)
        || types.expr_ty(searched) != types.expr_ty(indexed)
    {
        return false;
    }
    match (searched.kind, indexed.kind) {
        (ExprKind::Path(ref left), ExprKind::Path(ref right)) => {
            let found = cx.qpath_res(left, searched.hir_id);
            found == cx.qpath_res(right, indexed.hir_id)
                && match found {
                    Res::Local(id) => matches!(cx.tcx.hir_node(id), Node::Pat(pat)
                        if matches!(pat.kind, PatKind::Binding(BindingMode::NONE, ..))),
                    Res::Def(
                        DefKind::Static {
                            mutability: Mutability::Not,
                            ..
                        },
                        _,
                    ) => true,
                    Res::Def(
                        DefKind::Const {
                            is_type_const: false,
                        },
                        _,
                    ) => {
                        matches!(types.expr_ty(indexed).peel_refs().kind(), ty::Array(item, _)
                            if cx.type_is_copy_modulo_regions(*item))
                    }
                    _ => false,
                }
        }
        (ExprKind::Field(left, left_field), ExprKind::Field(right, right_field)) => {
            left_field.name == right_field.name
                && [left, right].iter().all(|field| {
                    types.expr_adjustments(field).iter().all(|adjustment| {
                        matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin))
                    })
                })
                && same_place(cx, left, right)
        }
        _ => false,
    }
}

fn searches_same_collection(cx: &LateContext<'_>, search: &Expr<'_>, base: &Expr<'_>) -> bool {
    let ExprKind::MethodCall(segment, searched, _, _) = search.kind else {
        return false;
    };
    matches!(
        segment.ident.as_str(),
        "binary_search" | "binary_search_by" | "binary_search_by_key"
    ) && inherent_receiver(cx, search).is_some_and(ty::Ty::is_slice)
        && same_place(cx, searched, base)
        && clean(cx, searched.span)
            .zip(clean(cx, base.span))
            .is_some_and(|(left, right)| left == right)
}

// These projections only evaluate the indexed place and, for a tuple
// result, the successful index itself. A preceding operation could change
// the collection and would also move across the new checked lookup.
fn projection(
    cx: &LateContext<'_>,
    access: &Expr<'_>,
    output: &Expr<'_>,
    index: HirId,
    element: &str,
) -> Option<String> {
    let mut inner = access;
    let mut replaced = access.span;
    let mut value = dereference(cx, access, element.to_owned());
    while inner.hir_id != output.hir_id {
        if cx
            .typeck_results()
            .expr_adjustments(inner)
            .iter()
            .any(|adjustment| {
                matches!(
                    adjustment.kind,
                    Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut { .. }))
                )
            })
        {
            return None;
        }
        let Node::Expr(outer) = cx.tcx.parent_hir_node(inner.hir_id) else {
            return None;
        };
        let direct = inner.hir_id == access.hir_id;
        match outer.kind {
            ExprKind::Field(base, _) if base.hir_id == inner.hir_id => {
                if direct {
                    value = element.to_owned();
                }
            }
            ExprKind::MethodCall(_, receiver, [], _) if receiver.hir_id == inner.hir_id => {}
            ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, base)
                if base.hir_id == inner.hir_id =>
            {
                if direct {
                    replaced = outer.span;
                    value = element.to_owned();
                }
            }
            ExprKind::Tup(parts)
                if parts.iter().all(|part| {
                    part.hir_id == inner.hir_id || part.res_local_id() == Some(index)
                }) => {}
            _ => return None,
        }
        inner = outer;
    }
    if !output.span.contains(replaced) || !output.span.eq_ctxt(replaced) {
        return None;
    }
    let before = clean(cx, output.span.with_hi(replaced.lo()))?;
    let after = clean(cx, output.span.with_lo(replaced.hi()))?;
    Some(format!("{before}{value}{after}"))
}

fn checked_lookup(
    cx: &LateContext<'_>,
    access: &Expr<'_>,
    output: &Expr<'_>,
    base: &Expr<'_>,
    offset: &Expr<'_>,
    index: HirId,
) -> Option<String> {
    clean(cx, output.span)?;
    if !cx.typeck_results().expr_adjustments(output).is_empty() {
        return None;
    }
    let borrowed = borrow_base(cx, base, false)?;
    let position = clean(cx, offset.span)?;
    let lookup = format!("<[_]>::get({borrowed}, {position})");
    if access.hir_id == output.hir_id {
        return cx
            .type_is_copy_modulo_regions(cx.typeck_results().expr_ty(access))
            .then(|| format!("{lookup}.copied()"));
    }
    let element = binding_name(cx, access, "searched_entry")?;
    let value = projection(cx, access, output, index, &element)?;
    Some(format!("{lookup}.map(|{element}| {value})"))
}

fn closure_edits(
    cx: &LateContext<'_>,
    access: &Expr<'_>,
    base: &Expr<'_>,
    offset: &Expr<'_>,
    index: HirId,
    closure_expr: &Expr<'_>,
) -> Option<Vec<(Span, String)>> {
    let ExprKind::Closure(closure) = closure_expr.kind else {
        return None;
    };
    if closure.kind != ClosureKind::Closure
        || !matches!(closure.fn_decl.output, FnRetTy::DefaultReturn(_))
        || !matches!(closure.capture_clause, rustc_ast::CaptureBy::Ref)
    {
        return None;
    }
    let body = cx.tcx.hir_body(closure.body);
    let [parameter] = body.params else {
        return None;
    };
    if !matches!(parameter.pat.kind, PatKind::Binding(BindingMode::NONE, id, _, None) if id == index)
    {
        return None;
    }
    let Node::Expr(call) = cx.tcx.parent_hir_node(closure_expr.hir_id) else {
        return None;
    };
    let ExprKind::MethodCall(map, option, [callback], _) = call.kind else {
        return None;
    };
    if callback.hir_id != closure_expr.hir_id
        || clean(cx, map.ident.span)? != "map"
        || !method_of(cx, call, sym::Option)
    {
        return None;
    }
    let ExprKind::MethodCall(ok, search, [], _) = option.kind else {
        return None;
    };
    if ok.ident.as_str() != "ok"
        || !method_of(cx, option, sym::Result)
        || !searches_same_collection(cx, search, base)
    {
        return None;
    }
    let replacement = checked_lookup(cx, access, body.value, base, offset, index)?;
    Some(vec![
        (map.ident.span, "and_then".to_owned()),
        (body.value.span, replacement),
    ])
}

fn arm_edits(
    cx: &LateContext<'_>,
    access: &Expr<'_>,
    base: &Expr<'_>,
    offset: &Expr<'_>,
    index: HirId,
    arm: &Arm<'_>,
) -> Option<Vec<(Span, String)>> {
    let PatKind::TupleStruct(ref constructor, [parameter], _) = arm.pat.kind else {
        return None;
    };
    let result = cx.qpath_res(constructor, arm.pat.hir_id).opt_def_id()?;
    if arm.guard.is_some()
        || !is_lang_item_or_ctor(cx, result, LangItem::ResultOk)
        || !matches!(parameter.kind, PatKind::Binding(BindingMode::NONE, id, _, None) if id == index)
    {
        return None;
    }
    let Node::Expr(matched) = cx.tcx.parent_hir_node(arm.hir_id) else {
        return None;
    };
    let ExprKind::Match(search, _, MatchSource::Normal) = matched.kind else {
        return None;
    };
    if !searches_same_collection(cx, search, base) {
        return None;
    }
    let ExprKind::Call(present, [output]) = arm.body.kind else {
        return None;
    };
    let ExprKind::Path(ref variant) = present.kind else {
        return None;
    };
    let option = cx.qpath_res(variant, present.hir_id).opt_def_id()?;
    if !is_lang_item_or_ctor(cx, option, LangItem::OptionSome) {
        return None;
    }
    clean(cx, arm.body.span)?;
    let lookup = checked_lookup(cx, access, output, base, offset, index)?;
    Some(vec![(arm.body.span, lookup)])
}

pub(super) fn edits(cx: &LateContext<'_>, access: &Expr<'_>) -> Option<Vec<(Span, String)>> {
    let ExprKind::Index(base, offset, _) = access.kind else {
        return None;
    };
    let types = cx.typeck_results();
    if !slice_like(cx, types.expr_ty(base).peel_refs())
        || !types.expr_ty(offset).is_usize()
        || types
            .type_dependent_def_id(access.hir_id)
            .is_some_and(|id| !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "alloc"))
    {
        return None;
    }
    let index = offset.res_local_id()?;
    clean(cx, access.span)?;
    for (_, node) in cx.tcx.hir_parent_iter(access.hir_id) {
        match node {
            Node::Expr(closure) if matches!(closure.kind, ExprKind::Closure(_)) => {
                return closure_edits(cx, access, base, offset, index, closure);
            }
            Node::Arm(arm) => return arm_edits(cx, access, base, offset, index, arm),
            Node::Block(_) => return None,
            _ => {}
        }
    }
    None
}
