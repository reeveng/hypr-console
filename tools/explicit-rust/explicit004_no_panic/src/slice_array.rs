//! Keep exact-length borrowed-array conversions on the slice lookup's existing error path.
use clippy_utils::{fn_def_id, is_in_const_context, sym};
use rustc_hir::{
    BindingMode, BorrowKind, Expr, ExprKind, MatchSource, Node, PatKind, UnOp, def::Res,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;

fn reborrowed<'t>(cx: &LateContext<'t>, expr: &'t Expr<'t>) -> &'t Expr<'t> {
    if let ExprKind::AddrOf(BorrowKind::Ref, _, place) = expr.kind
        && let ExprKind::Unary(UnOp::Deref, inner) = place.kind
        && cx.typeck_results().expr_ty(inner) == cx.typeck_results().expr_ty(expr)
        && cx.typeck_results().expr_adjustments(inner).is_empty()
        && cx
            .typeck_results()
            .type_dependent_def_id(place.hir_id)
            .is_none()
    {
        inner
    } else {
        expr
    }
}

fn bounds_lookup<'t>(
    cx: &LateContext<'t>,
    question: &'t Expr<'t>,
) -> Option<(&'t Expr<'t>, &'t Expr<'t>)> {
    let ExprKind::Match(branch, _, MatchSource::TryDesugar(_)) = question.kind else {
        return None;
    };
    let ExprKind::Call(_, [result]) = branch.kind else {
        return None;
    };
    let ExprKind::MethodCall(method, lookup, [error], _) = result.kind else {
        return None;
    };
    let ty::Adt(option, _) = cx.typeck_results().expr_ty(lookup).kind() else {
        return None;
    };
    if method.ident.name.as_str() != "ok_or"
        || !cx.tcx.is_diagnostic_item(sym::Option, option.did())
    {
        return None;
    }
    let id = fn_def_id(cx, lookup)?;
    let imp = cx.tcx.inherent_impl_of_assoc(id)?;
    if !matches!(cx.tcx.item_name(id).as_str(), "get" | "get_mut")
        || !cx
            .tcx
            .type_of(imp)
            .instantiate_identity()
            .skip_norm_wip()
            .is_slice()
    {
        return None;
    }
    Some((lookup, error))
}

fn variant(cx: &LateContext<'_>, error: &Expr<'_>, use_at: &Expr<'_>) -> Option<String> {
    if matches!(error.kind, ExprKind::Tup([])) {
        return Some("()".to_owned());
    }
    let ExprKind::Path(ref path) = error.kind else {
        return None;
    };
    let constructor = cx.qpath_res(path, error.hir_id).opt_def_id()?;
    let ty::Adt(adt, args) = cx.typeck_results().expr_ty(error).kind() else {
        return None;
    };
    if !adt.is_enum() || !adt.did().is_local() || !args.is_empty() {
        return None;
    }
    let name = adt
        .variants()
        .iter()
        .find(|v| {
            v.fields.is_empty()
                && (v.def_id == constructor || v.ctor_def_id() == Some(constructor))
        })?
        .name;
    let path =
        super::local_type_path::local_path(cx, adt.did(), cx.tcx.parent_module(use_at.hir_id))?;
    Some(format!(
        "{path}::{}",
        super::local_type_path::identifier(cx, name)
    ))
}

pub(super) fn replacement<'t>(
    cx: &LateContext<'t>,
    call: &'t Expr<'t>,
    conversion: &'t Expr<'t>,
) -> Option<String> {
    if call.span.from_expansion() || conversion.span.from_expansion() || is_in_const_context(cx) {
        return None;
    }
    let types = cx.typeck_results();
    let ExprKind::Call(callee, [argument]) = conversion.kind else {
        return None;
    };
    let constructor = fn_def_id(cx, conversion)?;
    if !cx.tcx.is_diagnostic_item(
        rustc_span::sym::TryFrom,
        cx.tcx.trait_of_assoc(constructor)?,
    ) {
        return None;
    }
    let ty::Adt(result, args) = types.expr_ty(conversion).kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
        || !types.expr_adjustments(conversion).is_empty()
        || types.expr_ty(call) != args.type_at(0)
    {
        return None;
    }
    let ty::Ref(_, arr, out_mut) = *types.expr_ty(call).kind() else {
        return None;
    };
    let ty::Array(out_elem, _) = *arr.kind() else {
        return None;
    };
    let ty::Ref(_, slice, in_mut) = *types.expr_ty(argument).kind() else {
        return None;
    };
    if !matches!(*slice.kind(), ty::Slice(elem) if elem == out_elem) || in_mut != out_mut {
        return None;
    }
    let input = reborrowed(cx, argument);
    let map = cx.sess().source_map();
    let source = map.span_to_snippet(call.span).ok()?;
    if source.contains("//") || source.contains("/*") || source.contains('#') {
        return None;
    }
    let (lookup, error, local) = if let ExprKind::Path(ref path) = input.kind {
        let Res::Local(id) = cx.qpath_res(path, input.hir_id) else {
            return None;
        };
        let Node::LetStmt(binding) = cx.tcx.parent_hir_node(id) else {
            return None;
        };
        if !matches!(
            binding.pat.kind,
            PatKind::Binding(BindingMode::NONE, _, _, None)
        ) || binding.els.is_some()
        {
            return None;
        }
        let init = binding.init?;
        let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
        if owner != cx.tcx.hir_enclosing_body_owner(init.hir_id) {
            return None;
        }
        // Adding a question mark in a try block or a different closure could select
        // a different residual target than the one chosen by the original lookup.
        let has_break_target = cx
            .tcx
            .hir_parent_iter(call.hir_id)
            .take_while(|(id, _)| id.owner == call.hir_id.owner)
            .any(|(_, node)| match node {
                Node::Expr(expr) => {
                    matches!(expr.kind, ExprKind::Block(block, _) if block.targeted_by_break)
                }
                _ => false,
            });
        if has_break_target {
            return None;
        }
        let (lookup, error) = bounds_lookup(cx, init)?;
        let output = types.expr_ty(cx.tcx.hir_body_owned_by(owner).value);
        let ty::Adt(ret, ret_args) = output.kind() else {
            return None;
        };
        if !cx.tcx.is_diagnostic_item(sym::Result, ret.did())
            || ret_args.type_at(1) != types.expr_ty(error)
        {
            return None;
        }
        (lookup, error, Some(input))
    } else {
        let (lookup, error) = bounds_lookup(cx, input)?;
        (lookup, error, None)
    };
    let err = variant(cx, error, call)?;
    let replacement = match local {
        Some(_) => format!(
            "{}.map_err(|_| {err})?",
            map.span_to_snippet(conversion.span).ok()?
        ),
        None => {
            if !call.span.contains(lookup.span)
                || !call.span.contains(error.span)
                || !call.span.contains(callee.span)
            {
                return None;
            }
            let lookup = map.span_to_snippet(lookup.span).ok()?;
            let convert = map.span_to_snippet(callee.span).ok()?;
            let module = cx.tcx.parent_module(call.hir_id).to_local_def_id();
            let binding = "_explicit_array_source";
            let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
            let body = map
                .span_to_snippet(cx.tcx.hir_body_owned_by(owner).value.span)
                .ok()?;
            if body.contains(binding)
                || cx
                    .tcx
                    .module_children_local(module)
                    .iter()
                    .any(|item| item.ident.as_str() == binding)
            {
                return None;
            }
            format!(
                "{lookup}.ok_or({err}).and_then(|{binding}| {convert}({binding}).map_err(|_| {err}))?"
            )
        }
    };
    Some(replacement)
}
