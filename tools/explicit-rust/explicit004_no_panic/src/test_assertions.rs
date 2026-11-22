//! Remove standard unwraps in assertions while keeping their comparison and evaluation order.
use super::test_results;
use clippy_utils::{
  macros::{find_assert_eq_args, root_macro_call_first_node},
  sym,
  ty::implements_trait,
  visitors::for_each_expr_without_closures,
};
use rustc_hir::{
  BindingMode, Expr, ExprKind, HirId, Node, PatKind, QPath, UnOp, def::Res, def_id::LocalDefId,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;
use std::ops::ControlFlow;

type Edits = Vec<(Span, String)>;

fn assertion<'tcx>(
  cx: &LateContext<'tcx>,
  expr: &'tcx Expr<'tcx>,
) -> Option<(Span, &'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
  let call = root_macro_call_first_node(cx, expr)?;
  if !cx.tcx.is_diagnostic_item(sym::assert_eq_macro, call.def_id) || call.span.from_expansion() {
    return None;
  }
  let (left, right, panic) = find_assert_eq_args(cx, expr, call.expn)?;
  // A custom message may have effects only when an equality check fails.
  (panic.is_default_message() && call.span.contains(left.span) && call.span.contains(right.span))
    .then_some((call.span, left, right))
}

fn inert(cx: &LateContext<'_>, expected: &Expr<'_>) -> bool {
  if expected.span.from_expansion() {
    return false;
  }
  match expected.kind {
    ExprKind::Lit(_) => true,
    ExprKind::Unary(UnOp::Neg, inner) => {
      !inner.span.from_expansion() && matches!(inner.kind, ExprKind::Lit(_))
    }
    ExprKind::Path(QPath::Resolved(None, path)) => matches!(path.res, Res::Local(id)
            if matches!(cx.tcx.hir_node(id), Node::Pat(p) if matches!(p.kind, PatKind::Binding(BindingMode::NONE, ..)))),
    _ => false,
  }
}

fn operands<'tcx>(
  cx: &LateContext<'tcx>,
  value: &'tcx Expr<'tcx>,
  expected: &'tcx Expr<'tcx>,
) -> Option<(HirId, Edits)> {
  if !inert(cx, expected) {
    return None;
  }
  let extracted = test_results::standard_unwrap(cx, value)?;
  let types = cx.typeck_results();
  let inner = types.expr_ty(value);
  let wrapped = types.expr_ty(extracted.receiver);
  if inner != types.expr_ty(expected)
    || !(inner.is_primitive()
      || matches!(inner.kind(), ty::Ref(_, s, ty::Mutability::Not) if s.is_str()))
    || !implements_trait(
      cx,
      wrapped,
      cx.tcx.lang_items().eq_trait()?,
      &[wrapped.into()],
    )
  {
    return None;
  }
  // assert_eq! borrows its arguments. A non-Copy place would retain
  // ownership after removing its by-value unwrap.
  if !cx.type_is_copy_modulo_regions(wrapped)
    && !matches!(
      extracted.receiver.kind,
      ExprKind::Call(..) | ExprKind::MethodCall(..)
    )
  {
    return None;
  }
  let variant = if extracted.error.is_some() {
    "::core::result::Result::Ok"
  } else {
    "::core::option::Option::Some"
  };
  let text = cx.sess().source_map().span_to_snippet(expected.span).ok()?;
  Some((
    value.hir_id,
    vec![
      (extracted.suffix, String::new()),
      (expected.span, format!("{variant}({text})")),
    ],
  ))
}

pub(super) fn edits<'tcx>(
  cx: &LateContext<'tcx>,
  expr: &'tcx Expr<'tcx>,
) -> Option<(LocalDefId, HirId, Edits)> {
  let (_, left, right) = assertion(cx, expr)?;
  let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
  if !test_results::ordinary_test(cx, owner) {
    return None;
  }
  let (site, changes) = operands(cx, left, right).or_else(|| operands(cx, right, left))?;
  let ExprKind::Block(block, None) = cx.tcx.hir_body_owned_by(owner).value.kind else {
    return None;
  };
  let source = cx.sess().source_map().span_to_snippet(block.span).ok()?;
  if !test_results::supported_source(cx, block, &source) {
    return None;
  }
  Some((owner, site, changes))
}

fn temporary_with_drop<'tcx>(cx: &LateContext<'tcx>, receiver: &'tcx Expr<'tcx>) -> bool {
  let types = cx.typeck_results();
  for_each_expr_without_closures(receiver, |sub| {
    let local = matches!(sub.kind, ExprKind::Path(QPath::Resolved(_, path))
            if matches!(path.res, Res::Local(_)));
    let place = local || matches!(sub.kind, ExprKind::Field(..));
    if sub.hir_id != receiver.hir_id
      && !place
      && types.expr_ty(sub).needs_drop(cx.tcx, cx.typing_env())
    {
      ControlFlow::Break(())
    } else {
      ControlFlow::Continue(())
    }
  })
  .is_some()
}

// Walk just the chain of receivers. An earlier argument, condition, or
// short-circuit must still run before a later fallible expression.
fn first_scalar<'tcx>(
  cx: &LateContext<'tcx>,
  mut value: &'tcx Expr<'tcx>,
  within: Span,
) -> Option<(HirId, Ty<'tcx>, Span, String)> {
  loop {
    if value.span.from_expansion() || !within.contains(value.span) {
      return None;
    }
    if let Some(extracted) = test_results::standard_unwrap(cx, value) {
      let error = extracted.error?;
      let ty = cx.typeck_results().expr_ty(value);
      if !ty.is_primitive() || temporary_with_drop(cx, extracted.receiver) {
        return None;
      }
      let source = cx
        .sess()
        .source_map()
        .span_to_snippet(extracted.receiver.span)
        .ok()?;
      return Some((value.hir_id, error, value.span, source));
    }
    value = match value.kind {
      ExprKind::MethodCall(_, receiver, ..)
      | ExprKind::Field(receiver, _)
      | ExprKind::Cast(receiver, _) => receiver,
      _ => return None,
    };
  }
}

/// Propagate the scalar read when its error type cannot be compared, or its
/// value is projected before comparison. The caller groups it with test setup.
pub(super) fn setup<'tcx>(
  cx: &LateContext<'tcx>,
  expr: &'tcx Expr<'tcx>,
  block: HirId,
  fresh: impl FnOnce() -> String,
) -> Option<(HirId, Ty<'tcx>, Edits)> {
  let Node::Stmt(statement) = cx.tcx.parent_hir_node(expr.hir_id) else {
    return None;
  };
  if cx.tcx.parent_hir_id(statement.hir_id) != block
    || !cx.tcx.hir_attrs(statement.hir_id).is_empty()
  {
    return None;
  }
  let (invocation, left, right) = assertion(cx, expr)?;
  if operands(cx, left, right).is_some() || operands(cx, right, left).is_some() {
    return None;
  }
  let (id, error, span, source) = first_scalar(cx, left, invocation).or_else(|| {
    if inert(cx, left) {
      first_scalar(cx, right, invocation)
    } else {
      None
    }
  })?;
  let indent = clippy_utils::source::indent_of(cx, invocation)?;
  let name = fresh();
  Some((
    id,
    error,
    vec![
      (
        invocation.shrink_to_lo(),
        format!("let {name} = {source}?;\n{}", " ".repeat(indent)),
      ),
      (span, name),
    ],
  ))
}
