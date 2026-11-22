//! Bind elements in the slice pattern that already decides their presence.
use super::fixes::{clean, function_source, position};
use clippy_utils::visitors::{for_each_expr_without_closures, is_local_used};
use rustc_ast::LitKind;
use rustc_hir::{
    BindingMode, Expr, ExprKind, HirId, MatchSource, Node, PatExprKind, PatKind, QPath, def::Res,
};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;
use std::ops::ControlFlow;

fn local(expr: &Expr<'_>) -> Option<HirId> {
    if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind
        && let Res::Local(id) = path.res
    {
        Some(id)
    } else {
        None
    }
}

// A fallback guard may use the original length, as in `n if n & 1 != 0`.
// Re-read it only in arithmetic on that immutable slice and literal values.
fn length_guard(
    expr: &Expr<'_>,
    binding: HirId,
    original: &str,
    edits: &mut Vec<(Span, String)>,
) -> bool {
    if expr.span.from_expansion() {
        return false;
    }
    if local(expr) == Some(binding) {
        edits.push((expr.span, original.to_owned()));
        return true;
    }
    match expr.kind {
        ExprKind::Lit(_) => true,
        ExprKind::Binary(_, left, right) => {
            length_guard(left, binding, original, edits)
                && length_guard(right, binding, original, edits)
        }
        _ => false,
    }
}

pub(super) fn edits<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(HirId, Vec<(Span, String)>)> {
    let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = expr.kind else {
        return None;
    };
    let ExprKind::MethodCall(method, slice, [], _) = scrutinee.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let binding = local(slice)?;
    let Node::Pat(pat) = cx.tcx.hir_node(binding) else {
        return None;
    };
    if method.ident.as_str() != "len"
        || !matches!(pat.kind, PatKind::Binding(BindingMode::NONE, ..))
        || !matches!(types.expr_ty(slice).kind(), ty::Ref(_, inner, ty::Mutability::Not) if inner.is_slice())
        || !types
            .type_dependent_def_id(scrutinee.hir_id)
            .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
            .is_some_and(|id| {
                cx.tcx
                    .type_of(id)
                    .instantiate_identity()
                    .skip_norm_wip()
                    .is_slice()
            })
    {
        return None;
    }
    let source = clean(cx, expr.span)?;
    if source.contains('#') {
        return None;
    }
    let name = clean(cx, slice.span)?;
    let length = clean(cx, scrutinee.span)?;
    let function = function_source(cx, slice)?;
    let mut edits = vec![(scrutinee.span, name.clone())];
    let mut first = None;
    let mut covered = [false; 9];
    for arm in arms {
        let exact = match arm.pat.kind {
            PatKind::Expr(pat) => {
                let PatExprKind::Lit {
                    lit,
                    negated: false,
                } = pat.kind
                else {
                    return None;
                };
                let LitKind::Int(n, _) = lit.node else {
                    return None;
                };
                let n = usize::try_from(n.get())
                    .ok()
                    .filter(|&n| n < covered.len())?;
                Some(n)
            }
            PatKind::Wild => None,
            PatKind::Binding(BindingMode::NONE, id, _, None) => {
                if is_local_used(cx, arm.body, id)
                    || !length_guard(arm.guard?, id, &length, &mut edits)
                {
                    return None;
                }
                None
            }
            _ => return None,
        };
        clean(cx, arm.pat.span)?;
        let count = exact.unwrap_or_else(|| covered.iter().take_while(|&&matched| matched).count());
        let mut slots = vec!["_".to_owned(); count];
        for_each_expr_without_closures(arm.body, |index| {
            if let ExprKind::Index(base, at, _) = index.kind
                && local(base) == Some(binding)
                && clean(cx, index.span).is_some()
                && let Some(at) = position(at, 8).map(usize::from)
                && let Some(slot) = slots.get_mut(at)
            {
                let element = format!("{name}_{at}");
                if !function.contains(&element) {
                    let read = match cx.tcx.parent_hir_node(index.hir_id) {
                        Node::Expr(parent) if matches!(parent.kind, ExprKind::Field(..)) => element.clone(),
                        Node::Expr(parent) if matches!(parent.kind, ExprKind::MethodCall(_, r, _, _)
                            | ExprKind::Index(r, _, _) | ExprKind::Call(r, _) if r.hir_id == index.hir_id)
                            => format!("(*{element})"),
                        _ => format!("*{element}"),
                    };
                    edits.push((index.span, read));
                    *slot = element;
                    first.get_or_insert(index.hir_id);
                }
            }
            ControlFlow::<(), ()>::Continue(())
        });
        if let Some(n) = exact {
            covered[n] |= arm.guard.is_none();
        } else {
            while slots.last().is_some_and(|name| name == "_") {
                slots.pop();
            }
            if slots.is_empty() {
                edits.push((arm.pat.span, "_".to_owned()));
                continue;
            }
            slots.push("..".to_owned());
        }
        edits.push((arm.pat.span, format!("[{}]", slots.join(", "))));
    }
    Some((first?, edits))
}
