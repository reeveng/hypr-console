//! Copy primitive byte arrays into destinations whose checked ranges have that width.

use clippy_utils::higher::Range;
use clippy_utils::res::MaybeResPath;
use clippy_utils::source::indent_of;
use clippy_utils::{fn_def_id, sym};
use rustc_ast::{LitKind, RangeLimits};
use rustc_hir::{
  BinOpKind, BindingMode, BlockCheckMode, BorrowKind, Expr, ExprKind, MatchSource, Mutability,
  Node, PatKind, StmtKind, UnOp,
  def::{DefKind, Res},
  def_id::DefId,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

pub(super) fn core_inherent_type<'tcx>(
  cx: &LateContext<'tcx>,
  method: DefId,
) -> Option<Ty<'tcx>> {
  if cx.tcx.crate_name(method.krate) != sym::core {
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

fn endian_array<'tcx>(
  cx: &LateContext<'tcx>,
  from: &'tcx Expr<'tcx>,
) -> Option<(&'tcx Expr<'tcx>, u64)> {
  let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Not, source) = from.kind else {
    return None;
  };
  let method = fn_def_id(cx, source)?;
  if !matches!(
    cx.tcx.item_name(method).as_str(),
    "to_le_bytes" | "to_be_bytes" | "to_ne_bytes"
  ) {
    return None;
  }
  let scalar = core_inherent_type(cx, method)?;
  if !(scalar.is_integral() || scalar.is_floating_point())
    || matches!(
      scalar.kind(),
      ty::Uint(ty::UintTy::Usize) | ty::Int(ty::IntTy::Isize)
    )
  {
    return None;
  }
  let ty::Array(element, size) = *cx.typeck_results().expr_ty(source).kind() else {
    return None;
  };
  if !matches!(element.kind(), ty::Uint(ty::UintTy::U8)) {
    return None;
  }
  Some((source, size.try_to_target_usize(cx.tcx)?))
}

fn wrapping_operand<'tcx>(
  cx: &LateContext<'tcx>,
  expr: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
  let ExprKind::Call(callee, [operand]) = expr.kind else {
    return None;
  };
  let ty::FnDef(constructor, _) = *cx.typeck_results().expr_ty(callee).kind() else {
    return None;
  };
  if !matches!(cx.tcx.def_kind(constructor), DefKind::Ctor(..))
    || !cx
      .tcx
      .is_diagnostic_item(sym::Wrapping, cx.tcx.parent(constructor))
  {
    return None;
  }
  Some(operand)
}

fn wrapping_add<'tcx>(
  cx: &LateContext<'tcx>,
  expr: &'tcx Expr<'tcx>,
) -> Option<(&'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
  wrapping_operands(cx, expr, BinOpKind::Add)
}

pub(super) fn wrapping_operands<'tcx>(
  cx: &LateContext<'tcx>,
  expr: &'tcx Expr<'tcx>,
  operator: BinOpKind,
) -> Option<(&'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
  match expr.kind {
    ExprKind::Field(binary, field) if field.name.as_str() == "0" => {
      let ExprKind::Binary(operation, left, right) = binary.kind else {
        return None;
      };
      if operation.node != operator {
        return None;
      }
      Some((wrapping_operand(cx, left)?, wrapping_operand(cx, right)?))
    }
    ExprKind::MethodCall(_, start, [width], _) | ExprKind::Call(_, [start, width]) => {
      let method = fn_def_id(cx, expr)?;
      let operation = match cx.tcx.item_name(method).as_str() {
        "wrapping_add" => BinOpKind::Add,
        "wrapping_sub" => BinOpKind::Sub,
        _ => return None,
      };
      (operation == operator && core_inherent_type(cx, method)?.is_usize())
        .then_some((start, width))
    }
    _ => None,
  }
}

pub(super) fn checked_question<'tcx>(base: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
  let ExprKind::Unary(UnOp::Deref, checked) = base.kind else {
    return None;
  };
  // EXPLICIT017 can already have moved the check into a one-binding block.
  let ExprKind::Block(block, None) = checked.kind else {
    return Some(checked);
  };
  let [statement] = block.stmts else {
    return None;
  };
  let StmtKind::Let(binding) = statement.kind else {
    return None;
  };
  let PatKind::Binding(BindingMode::NONE, local, _, None) = binding.pat.kind else {
    return None;
  };
  if block.rules != BlockCheckMode::DefaultBlock
    || binding.ty.is_some()
    || binding.els.is_some()
    || block.expr?.res_local_id()? != local
  {
    return None;
  }
  binding.init
}

pub(super) fn snippet(cx: &LateContext<'_>, outer: Span, inner: Span) -> Option<String> {
  if inner.from_expansion() || !outer.contains(inner) {
    return None;
  }
  cx.sess().source_map().span_to_snippet(inner).ok()
}

pub(super) fn replacement<'tcx>(
  cx: &LateContext<'tcx>,
  call: &'tcx Expr<'tcx>,
  base: &'tcx Expr<'tcx>,
  from: &'tcx Expr<'tcx>,
) -> Option<String> {
  let outer = call.span;
  let original = snippet(cx, outer, outer)?;
  if original.contains("//") || original.contains("/*") || original.contains('#') {
    return None;
  }
  if !matches!(cx.tcx.parent_hir_node(call.hir_id), Node::Stmt(_))
    || !core_inherent_type(cx, fn_def_id(cx, call)?)?.is_slice()
    || cx
      .typeck_results()
      .type_dependent_def_id(base.hir_id)
      .is_some()
  {
    return None;
  }
  let (source, size) = endian_array(cx, from)?;
  let checked = checked_question(base)?;
  let ExprKind::Match(branch, _, MatchSource::TryDesugar(_)) = checked.kind else {
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
  // A path to a fieldless variant or unit has no temporary destructor
  // whose scope would change when the question mark moves to a statement.
  if !matches!(error.kind, ExprKind::Tup([]))
    && !matches!(error.kind, ExprKind::Path(ref path)
            if matches!(cx.qpath_res(path, error.hir_id),
                Res::Def(DefKind::Ctor(rustc_hir::def::CtorOf::Variant, rustc_hir::def::CtorKind::Const), _)))
  {
    return None;
  }
  let ExprKind::Call(callee, [destination, bounds]) = lookup.kind else {
    return None;
  };
  let method = fn_def_id(cx, lookup)?;
  if cx.tcx.item_name(method).as_str() != "get_mut" || !core_inherent_type(cx, method)?.is_slice()
  {
    return None;
  }

  let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, place) = destination.kind else {
    return None;
  };
  let local = match place.kind {
    ExprKind::Unary(UnOp::Deref, local)
      if cx
        .typeck_results()
        .type_dependent_def_id(place.hir_id)
        .is_none() =>
    {
      local
    }
    _ => place,
  };
  local.res_local_id()?;

  let Range {
    start: Some(start),
    end: Some(end),
    limits: RangeLimits::HalfOpen,
    ..
  } = Range::hir(cx, bounds)?
  else {
    return None;
  };
  let (repeated_start, width) = wrapping_add(cx, end)?;
  if start.res_local_id()? != repeated_start.res_local_id()?
    || !matches!(width.kind, ExprKind::Lit(lit) if matches!(lit.node, LitKind::Int(value, _) if value.get() == u128::from(size)))
  {
    return None;
  }

  // When start + size wraps, the old inverted range fails. The tail also
  // has fewer than size bytes in that case, so first_chunk_mut fails.
  // Keep the early return before evaluating the source.
  let callee = snippet(cx, outer, callee.span)?;
  let destination = snippet(cx, outer, destination.span)?;
  let start = snippet(cx, outer, start.span)?;
  let error = snippet(cx, outer, error.span)?;
  let source = snippet(cx, outer, source.span)?;
  let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
  let module = cx.tcx.parent_module(call.hir_id).to_local_def_id();
  let body = cx.tcx.hir_body_owned_by(owner).value.span;
  let body_source = cx.sess().source_map().span_to_snippet(body).ok()?;
  let target = "explicit_byte_copy_destination";
  if body_source.contains(target)
    || cx
      .tcx
      .module_children_local(module)
      .iter()
      .any(|child| child.ident.as_str() == target)
  {
    return None;
  }
  let indent = " ".repeat(indent_of(cx, outer)?);
  Some(format!(
    "{{\n{indent}  let {target} = {callee}({destination}, {start}..)\n{indent}    \
         .and_then(<[_]>::first_chunk_mut::<{size}>)\n{indent}    .ok_or({error})?;\n\
         {indent}  *{target} = {source};\n{indent}}}"
  ))
}
