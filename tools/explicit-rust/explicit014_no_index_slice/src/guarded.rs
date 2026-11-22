//! Replace a short-circuiting bounds check and its scalar reads together.

use super::{
  binary_search::{inherent_receiver, same_place},
  fixes::clean,
  recovery::{binding_name, borrow_base, slice_like},
};
use clippy_utils::res::MaybeResPath;
use rustc_ast::LitKind;
use rustc_hir::{
  BinOpKind, Expr, ExprKind, Node, UnOp,
  def::{DefKind, Res},
};
use rustc_lint::LateContext;
use rustc_middle::ty::{
  self,
  adjustment::{Adjust, AutoBorrow, AutoBorrowMutability},
};
use rustc_span::{Span, sym};

#[derive(Clone, Copy)]
enum Lookup<'tcx> {
  Element(&'tcx Expr<'tcx>),
  Prefix(u8),
}

#[derive(Clone, Copy)]
struct Read {
  span: Span,
  position: u8,
}

fn literal_position(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<u8> {
  if expression.span.from_expansion() || !cx.typeck_results().expr_ty(expression).is_usize() {
    return None;
  }
  let ExprKind::Lit(literal) = expression.kind else {
    return None;
  };
  let LitKind::Int(value, _) = literal.node else {
    return None;
  };
  u8::try_from(value.get()).ok()
}

impl<'tcx> Lookup<'tcx> {
  fn position(self, cx: &LateContext<'_>, key: &Expr<'_>) -> Option<u8> {
    match self {
      Self::Element(offset) => same_offset(cx, key, offset, 8).then_some(0),
      Self::Prefix(length) => literal_position(cx, key).filter(|position| *position < length),
    }
  }

  fn variable(self, name: &str, position: u8) -> String {
    match self {
      Self::Element(_) => name.to_owned(),
      Self::Prefix(_) => format!("{name}_{position}"),
    }
  }

  fn pattern(
    self,
    cx: &LateContext<'_>,
    borrowed: &str,
    name: &str,
    positions: &[u8],
  ) -> Option<(String, String)> {
    match self {
      Self::Element(offset) => {
        let at = clean(cx, offset.span)?;
        Some((format!("<[_]>::get({borrowed}, {at})"), format!("&{name}")))
      }
      Self::Prefix(length) => {
        let fields = (0..length)
          .map(|position| {
            if positions.contains(&position) {
              self.variable(name, position)
            } else {
              "_".to_owned()
            }
          })
          .collect::<Vec<_>>()
          .join(", ");
        Some((
          format!("<[_]>::first_chunk::<{length}>({borrowed})"),
          format!("&[{fields}]"),
        ))
      }
    }
  }
}

fn same_offset(cx: &LateContext<'_>, left: &Expr<'_>, right: &Expr<'_>, budget: u8) -> bool {
  let Some(next) = budget.checked_sub(1) else {
    return false;
  };
  let types = cx.typeck_results();
  if left.span.from_expansion()
    || right.span.from_expansion()
    || !left.span.eq_ctxt(right.span)
    || !types.expr_ty(left).is_usize()
    || !types.expr_ty(right).is_usize()
  {
    return false;
  }
  match (left.kind, right.kind) {
    (ExprKind::Lit(a), ExprKind::Lit(b)) => {
      matches!((&a.node, &b.node), (LitKind::Int(x, _), LitKind::Int(y, _)) if x == y)
    }
    (ExprKind::Path(..), ExprKind::Path(..)) => left
      .res_local_id()
      .is_some_and(|local| right.res_local_id() == Some(local)),
    (ExprKind::Field(..), ExprKind::Field(..)) => same_place(cx, left, right),
    (ExprKind::Binary(a, x, y), ExprKind::Binary(b, u, v))
      if a.node == b.node && matches!(a.node, BinOpKind::Add | BinOpKind::Sub) =>
    {
      same_offset(cx, x, u, next) && same_offset(cx, y, v, next)
    }
    _ => false,
  }
}

fn collection_method(cx: &LateContext<'_>, call: &Expr<'_>, base: &Expr<'_>, name: &str) -> bool {
  let ExprKind::MethodCall(segment, receiver, [], _) = call.kind else {
    return false;
  };
  segment.ident.as_str() == name
    && inherent_receiver(cx, call).is_some_and(|owner| {
      owner.is_slice()
        || matches!(owner.kind(), ty::Adt(adt, _)
                if cx.tcx.is_diagnostic_item(sym::Vec, adt.did()))
    })
    && same_place(cx, receiver, base)
}

fn bounds_check<'tcx>(
  cx: &LateContext<'tcx>,
  guard: &'tcx Expr<'tcx>,
  base: &'tcx Expr<'tcx>,
  offset: &'tcx Expr<'tcx>,
  connective: BinOpKind,
) -> Option<Lookup<'tcx>> {
  if let ExprKind::Binary(operator, left, right) = guard.kind {
    let (threshold, length, add) = match (connective, operator.node) {
      (BinOpKind::And, BinOpKind::Lt) | (BinOpKind::Or, BinOpKind::Ge) => (left, right, 1),
      (BinOpKind::And, BinOpKind::Gt) | (BinOpKind::Or, BinOpKind::Le) => (right, left, 1),
      (BinOpKind::And, BinOpKind::Le) | (BinOpKind::Or, BinOpKind::Gt) => (left, right, 0),
      (BinOpKind::And, BinOpKind::Ge) | (BinOpKind::Or, BinOpKind::Lt) => (right, left, 0),
      _ => return None,
    };
    if !collection_method(cx, length, base, "len") {
      return None;
    }
    // Requiring the whole prefix preserves a stronger guard even when the
    // predicate only reads an earlier byte.
    if let Some(width) = literal_position(cx, threshold).and_then(|width| width.checked_add(add))
      && (2..=16).contains(&width)
      && literal_position(cx, offset).is_some_and(|position| position < width)
    {
      return Some(Lookup::Prefix(width));
    }
    return (add == 1 && same_offset(cx, threshold, offset, 8))
      .then_some(Lookup::Element(offset));
  }
  let empty = match (connective, guard.kind) {
    (BinOpKind::And, ExprKind::Unary(UnOp::Not, empty)) => empty,
    (BinOpKind::Or, _) => guard,
    _ => return None,
  };
  (literal_position(cx, offset) == Some(0) && collection_method(cx, empty, base, "is_empty"))
    .then_some(Lookup::Element(offset))
}

fn scalar(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
  let kind = cx.typeck_results().expr_ty(expression);
  kind.is_integral() || kind.is_bool() || kind.is_char() || kind.is_floating_point()
}

// A checked primitive read can be bound by value. Keeping this to scalar
// predicates excludes control flow, mutations and values with destructors.
fn reads(
  cx: &LateContext<'_>,
  value: &Expr<'_>,
  base: &Expr<'_>,
  lookup: Lookup<'_>,
  found: &mut Vec<Read>,
  budget: u8,
) -> bool {
  let Some(next) = budget.checked_sub(1) else {
    return false;
  };
  let types = cx.typeck_results();
  if value.span.from_expansion()
    || !scalar(cx, value)
    || types.expr_adjustments(value).iter().any(|adjustment| {
      !matches!(
        adjustment.kind,
        Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not))
      )
    })
  {
    return false;
  }
  match value.kind {
    ExprKind::Index(received, key, _) if same_place(cx, received, base) => {
      let Some(position) = lookup.position(cx, key) else {
        return false;
      };
      if types
        .type_dependent_def_id(value.hir_id)
        .is_some_and(|id| !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "alloc"))
      {
        return false;
      }
      found.push(Read {
        span: value.span,
        position,
      });
      true
    }
    ExprKind::Lit(_) => true,
    ExprKind::Path(ref path) => matches!(
      cx.qpath_res(path, value.hir_id),
      Res::Local(_)
        | Res::Def(
          DefKind::Const {
            is_type_const: false
          },
          _
        )
    ),
    ExprKind::Binary(_, left, right) => {
      reads(cx, left, base, lookup, found, next) && reads(cx, right, base, lookup, found, next)
    }
    ExprKind::Unary(UnOp::Neg | UnOp::Not, operand) => {
      reads(cx, operand, base, lookup, found, next)
    }
    ExprKind::Call(function, args)
      if !function.span.from_expansion()
        && matches!(function.kind, ExprKind::Path(..))
        && matches!(types.expr_ty(function).kind(), ty::FnDef(..)) =>
    {
      args
        .iter()
        .all(|argument| reads(cx, argument, base, lookup, found, next))
    }
    ExprKind::MethodCall(method, receiver, [], _)
      if method.ident.as_str().starts_with("is_ascii")
        && inherent_receiver(cx, value)
          .is_some_and(|owner| matches!(owner.kind(), ty::Uint(ty::UintTy::U8) | ty::Char)) =>
    {
      reads(cx, receiver, base, lookup, found, next)
    }
    _ => false,
  }
}

fn grouped(expression: &Expr<'_>, connective: BinOpKind, source: String) -> String {
  if connective == BinOpKind::And
    && matches!(expression.kind, ExprKind::Binary(op, ..) if op.node == BinOpKind::Or)
    && !(source.starts_with('(') && source.ends_with(')'))
  {
    format!("({source})")
  } else {
    source
  }
}

fn has_let_condition(expression: &Expr<'_>, budget: u8) -> bool {
  let Some(next) = budget.checked_sub(1) else {
    return true;
  };
  match expression.kind {
    ExprKind::Let(_) => true,
    ExprKind::Binary(op, lhs, rhs) if op.node == BinOpKind::And => {
      has_let_condition(lhs, next) || has_let_condition(rhs, next)
    }
    _ => false,
  }
}

fn predicate_source(
  cx: &LateContext<'_>,
  predicate: &Expr<'_>,
  found: &mut [Read],
  lookup: Lookup<'_>,
  name: &str,
  connective: BinOpKind,
) -> Option<String> {
  let mut source = clean(cx, predicate.span)?;
  found.sort_by_key(|read| std::cmp::Reverse(read.span.lo()));
  for &Read { span, position } in found.iter() {
    if !predicate.span.contains(span) || !predicate.span.eq_ctxt(span) {
      return None;
    }
    let start = usize::try_from((span.lo() - predicate.span.lo()).0).ok()?;
    let end = usize::try_from((span.hi() - predicate.span.lo()).0).ok()?;
    source.get(start..end)?;
    source.replace_range(start..end, &lookup.variable(name, position));
  }
  Some(grouped(predicate, connective, source))
}

pub(super) fn replacement<'tcx>(
  cx: &LateContext<'tcx>,
  access: &'tcx Expr<'tcx>,
) -> Option<(Span, String)> {
  let ExprKind::Index(base, offset, _) = access.kind else {
    return None;
  };
  let types = cx.typeck_results();
  if !scalar(cx, access)
    || !slice_like(cx, types.expr_ty(base).peel_refs())
    || !types.expr_ty(offset).is_usize()
  {
    return None;
  }
  clean(cx, access.span)?;
  for (_, node) in cx.tcx.hir_parent_iter(access.hir_id) {
    let Node::Expr(whole) = node else { return None };
    if matches!(whole.kind, ExprKind::Closure(_)) {
      return None;
    }
    let ExprKind::Binary(operator, left, predicate) = whole.kind else {
      continue;
    };
    let connective = operator.node;
    if !matches!(connective, BinOpKind::And | BinOpKind::Or)
      || !predicate.span.contains(access.span)
    {
      continue;
    }
    let (prefix, guard) = match left.kind {
      ExprKind::Binary(op, prefix, guard) if op.node == connective => (Some(prefix), guard),
      _ => (None, left),
    };
    let Some(lookup) = bounds_check(cx, guard, base, offset, connective) else {
      continue;
    };
    let mut found = Vec::new();
    if !reads(cx, predicate, base, lookup, &mut found, 16)
      || found.first().is_none_or(|read| read.span != access.span)
      || prefix.is_some_and(|value| has_let_condition(value, 16))
    {
      return None;
    }
    let name = binding_name(cx, access, "checked_element")?;
    let mut positions: Vec<_> = found.iter().map(|read| read.position).collect();
    let mut written = vec![predicate_source(
      cx, predicate, &mut found, lookup, &name, connective,
    )?];
    let mut whole = whole;
    // Keep subsequent comparisons of the same checked place in the same
    // branch instead of removing the guard that proved their bounds.
    while let Node::Expr(parent) = cx.tcx.parent_hir_node(whole.hir_id)
      && let ExprKind::Binary(op, earlier, later) = parent.kind
      && op.node == connective
      && earlier.hir_id == whole.hir_id
    {
      let mut later_reads = Vec::new();
      if !reads(cx, later, base, lookup, &mut later_reads, 16) || later_reads.is_empty() {
        break;
      }
      positions.extend(later_reads.iter().map(|read| read.position));
      written.push(predicate_source(
        cx,
        later,
        &mut later_reads,
        lookup,
        &name,
        connective,
      )?);
      whole = parent;
    }
    if clean(cx, whole.span)?.contains('#') {
      return None;
    }
    let borrowed = borrow_base(cx, base, false)?;
    let (lookup, pattern) = lookup.pattern(cx, &borrowed, &name, &positions)?;
    let separator = if connective == BinOpKind::And {
      " && "
    } else {
      " || "
    };
    let predicate = written.join(separator);
    let missing = connective == BinOpKind::Or;
    let mut replacement = format!(
      "match {lookup} {{ ::core::option::Option::Some({pattern}) => {predicate}, ::core::option::Option::None => {missing} }}"
    );
    if let Some(prefix) = prefix {
      let prefix = grouped(prefix, connective, clean(cx, prefix.span)?);
      replacement = format!("{prefix}{separator}{replacement}");
    }
    if matches!(cx.tcx.parent_hir_node(whole.hir_id), Node::Expr(parent) if match parent.kind {
        ExprKind::Binary(..) | ExprKind::Cast(..) | ExprKind::Unary(..) => true,
        ExprKind::Call(receiver, _) | ExprKind::MethodCall(_, receiver, _, _)
            | ExprKind::Field(receiver, _) | ExprKind::Index(receiver, ..) => {
            receiver.hir_id == whole.hir_id
        }
        _ => false,
    }) {
      replacement = format!("({replacement})");
    }
    return Some((whole.span, replacement));
  }
  None
}
