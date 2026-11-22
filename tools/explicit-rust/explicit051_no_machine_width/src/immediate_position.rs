//! Consume an infallible position result at the existing block boundary.
use super::{bounded_ranges::permits_edit, fixes::clean};
use clippy_utils::is_lang_item_or_ctor;
use rustc_hir::{
    BindingMode, Expr, ExprKind, HirId, LangItem, LetStmt, Node, PatKind,
    def::{DefKind, Res},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, sym};

fn result_arguments<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> Option<ty::GenericArgsRef<'tcx>> {
    let ty::Adt(adt, args) = ty.kind() else {
        return None;
    };
    cx.tcx
        .is_diagnostic_item(sym::Result, adt.did())
        .then_some(*args)
}

pub(super) fn infallible_result<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(Ty<'tcx>, Ty<'tcx>)> {
    let args = result_arguments(cx, ty)?;
    let error = args.type_at(1);
    if !error.is_never()
        && !matches!(error.kind(), ty::Adt(adt, _) if adt.is_enum()
        && adt.variants().is_empty() && !adt.is_variant_list_non_exhaustive())
    {
        return None;
    }
    Some((args.type_at(0), error))
}

fn returns_same_error<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'_>, error: Ty<'tcx>) -> bool {
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    if !matches!(cx.tcx.def_kind(owner), DefKind::Fn | DefKind::AssocFn)
        || cx
            .tcx
            .hir_parent_iter(expr.hir_id)
            .take_while(|(id, _)| id.owner == expr.hir_id.owner)
            .any(|(_, node)| matches!(node, Node::Block(block) if block.targeted_by_break))
    {
        return false;
    }
    let signature = cx.tcx.fn_sig(owner).instantiate_identity().skip_binder();
    result_arguments(cx, signature.output()).is_some_and(|args| args.type_at(1) == error)
}

fn local_value(cx: &LateContext<'_>, value: &Expr<'_>, binding: HirId) -> bool {
    matches!(value.kind, ExprKind::Path(ref path) if cx.qpath_res(path, value.hir_id) == Res::Local(binding))
}

pub(super) fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    local: &'tcx LetStmt<'tcx>,
) -> Option<Vec<(Span, String)>> {
    // In edition 2024 a block's tail-expression temporaries drop before its
    // enclosing expression continues. Keep the block itself and any preceding
    // statements so a borrow or guard in the initializer has the same scope.
    if !cx.sess().edition().at_least_rust_2024()
        || local.els.is_some()
        || clippy_utils::is_in_const_context(cx)
    {
        return None;
    }
    let PatKind::TupleStruct(path, [field], _) = local.pat.kind else {
        return None;
    };
    let ctor = cx.qpath_res(&path, local.pat.hir_id).opt_def_id()?;
    if !is_lang_item_or_ctor(cx, ctor, LangItem::ResultOk) {
        return None;
    }
    let PatKind::Binding(BindingMode::NONE, binding, ident, None) = field.kind else {
        return None;
    };
    let Node::Stmt(stmt) = cx.tcx.parent_hir_node(local.hir_id) else {
        return None;
    };
    let Node::Block(block) = cx.tcx.parent_hir_node(stmt.hir_id) else {
        return None;
    };
    let Node::Expr(boundary) = cx.tcx.parent_hir_node(block.hir_id) else {
        return None;
    };
    if block.stmts.last()?.hir_id != stmt.hir_id
        || !permits_edit(cx, boundary)
        || !cx.tcx.hir_attrs(stmt.hir_id).is_empty()
        || !cx.tcx.hir_attrs(local.hir_id).is_empty()
    {
        return None;
    }
    let types = cx.typeck_results();
    let width = types.node_type(binding);
    if width != cx.tcx.types.usize && width != cx.tcx.types.isize {
        return None;
    }
    let init = local.init?;
    let init_ty = types.expr_ty(init);
    let (_, error) = infallible_result(cx, init_ty)?;
    let tail = block.expr?;
    if !types.expr_adjustments(init).is_empty()
        || !types.expr_adjustments(tail).is_empty()
        || init.span.from_expansion()
        || tail.span.from_expansion()
        || clean(cx, stmt.span.to(tail.span))?.contains('#')
    {
        return None;
    }
    // Forwarding the original Result also retains the exact success and error types.
    let (prefix, postfix) = if let ExprKind::Call(ok, [value]) = tail.kind
        && let ExprKind::Path(path) = ok.kind
        && cx
            .qpath_res(&path, ok.hir_id)
            .opt_def_id()
            .is_some_and(|id| is_lang_item_or_ctor(cx, id, LangItem::ResultOk))
        && local_value(cx, value, binding)
        && types.expr_ty(tail) == init_ty
    {
        ("", String::new())
    } else if local_value(cx, tail, binding) {
        if returns_same_error(cx, boundary, error) {
            ("", "?".to_owned())
        } else {
            let name = clean(cx, ident.span)?;
            (
                "match ",
                format!(" {{ ::core::result::Result::Ok({name}) => {name} }}"),
            )
        }
    } else {
        return None;
    };
    let grouped = postfix == "?"
        && !matches!(
            init.kind,
            ExprKind::Call(..)
                | ExprKind::MethodCall(..)
                | ExprKind::Path(..)
                | ExprKind::Field(..)
                | ExprKind::Index(..)
        );
    Some(vec![
        (
            stmt.span.with_hi(init.span.lo()),
            if grouped {
                "(".into()
            } else {
                prefix.to_owned()
            },
        ),
        (
            init.span.with_lo(init.span.hi()).to(tail.span),
            format!("{}{postfix}", if grouped { ")" } else { "" }),
        ),
    ])
}
