//! Copy between two slices whose checked ranges establish the same length.

use super::endian_copy::{checked_question, core_inherent_type, snippet, wrapping_operands};
use clippy_utils::higher::Range;
use clippy_utils::res::MaybeResPath;
use clippy_utils::source::indent_of;
use clippy_utils::{fn_def_id, is_in_const_context, sym};
use rustc_ast::{LitKind, RangeLimits};
use rustc_hir::{
  BinOpKind, BindingMode, BorrowKind, Expr, ExprKind, MatchSource, Mutability, Node, PatKind,
  UnOp,
  def::{CtorKind, CtorOf, DefKind, Res},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

struct CheckedSlice<'tcx> {
  lookup: &'tcx Expr<'tcx>,
  storage: &'tcx Expr<'tcx>,
  callee: &'tcx Expr<'tcx>,
  error: &'tcx Expr<'tcx>,
  start: Option<&'tcx Expr<'tcx>>,
  width: &'tcx Expr<'tcx>,
}

fn integer_literal(expr: &Expr<'_>) -> Option<u128> {
  let ExprKind::Lit(lit) = expr.kind else {
    return None;
  };
  let LitKind::Int(value, _) = lit.node else {
    return None;
  };
  Some(value.get())
}

fn same_index<'tcx>(
  cx: &LateContext<'tcx>,
  left: &'tcx Expr<'tcx>,
  right: &'tcx Expr<'tcx>,
  depth: u8,
) -> bool {
  let types = cx.typeck_results();
  if left.span.from_expansion()
    || right.span.from_expansion()
    || !types.expr_ty(left).is_usize()
    || !types.expr_ty(right).is_usize()
  {
    return false;
  }
  if let Some(local) = left.res_local_id() {
    return right.res_local_id() == Some(local);
  }
  if let Some(value) = integer_literal(left) {
    return integer_literal(right) == Some(value);
  }
  let Some(next) = depth.checked_sub(1) else {
    return false;
  };
  let Some((left_base, left_step)) = wrapping_operands(cx, left, BinOpKind::Add) else {
    return false;
  };
  let Some((right_base, right_step)) = wrapping_operands(cx, right, BinOpKind::Add) else {
    return false;
  };
  same_index(cx, left_base, right_base, next) && same_index(cx, left_step, right_step, next)
}

fn checked_slice<'tcx>(
  cx: &LateContext<'tcx>,
  base: &'tcx Expr<'tcx>,
  mutability: Mutability,
) -> Option<CheckedSlice<'tcx>> {
  let question = checked_question(base)?;
  let ExprKind::Match(branch, _, MatchSource::TryDesugar(_)) = question.kind else {
    return None;
  };
  let ExprKind::Call(_, [result]) = branch.kind else {
    return None;
  };
  let ExprKind::MethodCall(_, lookup, [error], _) = result.kind else {
    return None;
  };
  let method = fn_def_id(cx, result)?;
  if cx.tcx.item_name(method).as_str() != "ok_or"
    || !cx.tcx.is_diagnostic_item(
      sym::Option,
      core_inherent_type(cx, method)?.ty_adt_def()?.did(),
    )
  {
    return None;
  }
  if !matches!(error.kind, ExprKind::Tup([]))
    && !matches!(error.kind, ExprKind::Path(ref path)
      if matches!(cx.qpath_res(path, error.hir_id),
        Res::Def(DefKind::Ctor(CtorOf::Variant, CtorKind::Const), _)))
  {
    return None;
  }
  let ExprKind::Call(callee, [storage, bounds]) = lookup.kind else {
    return None;
  };
  let method = fn_def_id(cx, lookup)?;
  let name = if mutability == Mutability::Mut {
    "get_mut"
  } else {
    "get"
  };
  if cx.tcx.item_name(method).as_str() != name || !core_inherent_type(cx, method)?.is_slice() {
    return None;
  }
  let types = cx.typeck_results();
  let ExprKind::AddrOf(BorrowKind::Ref, actual, place) = storage.kind else {
    return None;
  };
  let local = match place.kind {
    ExprKind::Unary(UnOp::Deref, local)
      if types.type_dependent_def_id(place.hir_id).is_none() =>
    {
      local
    }
    _ => place,
  };
  if actual != mutability || types.type_dependent_def_id(base.hir_id).is_some() {
    return None;
  }
  local.res_local_id()?;

  let Range {
    start,
    end: Some(end),
    limits: RangeLimits::HalfOpen,
    ..
  } = Range::hir(cx, bounds)?
  else {
    return None;
  };
  let width = if let Some(start) = start {
    if let Some((base, width)) = wrapping_operands(cx, end, BinOpKind::Add)
      && same_index(cx, start, base, 2)
    {
      width
    } else if let Some((base, width)) = wrapping_operands(cx, start, BinOpKind::Sub)
      && same_index(cx, end, base, 2)
    {
      width
    } else {
      return None;
    }
  } else {
    end
  };
  if width.span.from_expansion() {
    return None;
  }
  Some(CheckedSlice {
    lookup,
    storage,
    callee,
    error,
    start,
    width,
  })
}

fn immutable_local_width(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<rustc_hir::HirId> {
  let id = expr.res_local_id()?;
  let Node::Pat(pat) = cx.tcx.hir_node(id) else {
    return None;
  };
  (matches!(pat.kind, PatKind::Binding(BindingMode::NONE, ..))
    && cx.typeck_results().expr_ty(expr).is_usize())
  .then_some(id)
}

fn checked_chunk(
  cx: &LateContext<'_>,
  outer: Span,
  slice: &CheckedSlice<'_>,
  method: &str,
  width: u128,
  indent: &str,
) -> Option<String> {
  let storage = snippet(cx, outer, slice.storage.span)?;
  let error = snippet(cx, outer, slice.error.span)?;
  let chunk = format!("<[_]>::{method}::<{width}>");
  let lookup = if let Some(start) = slice.start {
    let start = snippet(cx, outer, start.span)?;
    let callee = snippet(cx, outer, slice.callee.span)?;
    format!("{callee}({storage}, {start}..)\n{indent}    .and_then({chunk})")
  } else {
    format!("{chunk}({storage})")
  };
  Some(format!("{lookup}\n{indent}    .ok_or({error})?"))
}

fn checked_range(cx: &LateContext<'_>, outer: Span, slice: &CheckedSlice<'_>) -> Option<String> {
  let lookup = snippet(cx, outer, slice.lookup.span)?;
  let error = snippet(cx, outer, slice.error.span)?;
  Some(format!("{lookup}.ok_or({error})?"))
}

pub(super) fn replacement<'tcx>(
  cx: &LateContext<'tcx>,
  call: &'tcx Expr<'tcx>,
  base: &'tcx Expr<'tcx>,
  from: &'tcx Expr<'tcx>,
) -> Option<String> {
  let outer = call.span;
  let original = snippet(cx, outer, outer)?;
  if original.contains("//")
    || original.contains("/*")
    || original.contains('#')
    || is_in_const_context(cx)
    || !matches!(cx.tcx.parent_hir_node(call.hir_id), Node::Stmt(_))
    || !core_inherent_type(cx, fn_def_id(cx, call)?)?.is_slice()
    || !matches!(cx.typeck_results().expr_ty(base).kind(), ty::Slice(element) if element.is_primitive())
  {
    return None;
  }
  let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, from) = from.kind else {
    return None;
  };
  let dest = checked_slice(cx, base, Mutability::Mut)?;
  let source = checked_slice(cx, from, Mutability::Not)?;

  let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
  let body = cx.tcx.hir_body_owned_by(owner).value.span;
  let body = cx.sess().source_map().span_to_snippet(body).ok()?;
  let module = cx.tcx.parent_module(call.hir_id).to_local_def_id();
  let children = cx.tcx.module_children_local(module);
  let names = [
    "checked_copy_destination",
    "checked_copy_source",
    "destination_element",
    "source_element",
  ];
  if names
    .iter()
    .any(|name| body.contains(name) || children.iter().any(|child| child.ident.as_str() == *name))
  {
    return None;
  }
  let [destination_name, source_name, dest_element, source_element] = names;
  let indent = " ".repeat(indent_of(cx, outer)?);

  // Successful wrapping ranges have their requested width. With a fixed width,
  // a tail cannot hold that chunk after either kind of wraparound.
  let (destination_check, source_check, copy) = if let Some(width) = integer_literal(dest.width) {
    if integer_literal(source.width)? != width {
      return None;
    }
    (
      checked_chunk(cx, outer, &dest, "first_chunk_mut", width, &indent)?,
      checked_chunk(cx, outer, &source, "first_chunk", width, &indent)?,
      format!("*{destination_name} = *{source_name};"),
    )
  } else {
    if immutable_local_width(cx, dest.width)? != immutable_local_width(cx, source.width)? {
      return None;
    }
    (
      checked_range(cx, outer, &dest)?,
      checked_range(cx, outer, &source)?,
      format!(
        "for ({dest_element}, {source_element}) in {destination_name}\n{indent}    \
         .iter_mut()\n{indent}    .zip({source_name}.iter())\n{indent}  {{\n\
         {indent}    *{dest_element} = *{source_element};\n{indent}  }}"
      ),
    )
  };
  Some(format!(
    "{{\n{indent}  let {destination_name} = {destination_check};\n\
     {indent}  let {source_name} = {source_check};\n{indent}  {copy}\n{indent}}}"
  ))
}
