//! Propagate setup failures to the Rust test harness as one edit per test.
use clippy_utils::{is_test_function, sym, visitors::for_each_expr_without_closures};
use rustc_hir::{
  AttrArgs, Attribute, Block, Body, Expr, ExprKind, FnDecl, FnRetTy, HirId, Node, StmtKind,
  def_id::LocalDefId,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{BytePos, Span};
use std::{collections::HashSet, ops::ControlFlow};

// A setup routine called by another test must still fail that caller. The
// harness is the only caller that converts a returned Err into a test failure.
pub(super) fn referenced_functions(cx: &LateContext<'_>) -> HashSet<LocalDefId> {
  use rustc_hir::{
    QPath,
    def::{DefKind, Res},
  };
  let mut referenced = HashSet::new();
  for owner in cx.tcx.hir_body_owners() {
    if cx
      .tcx
      .hir_parent_iter(cx.tcx.local_def_id_to_hir_id(owner))
      .any(|(id, _)| rustc_hir::find_attr!(cx.tcx, id, RustcTestMarker(..)))
      || rustc_hir::find_attr!(cx.tcx, owner.to_def_id(), RustcTestMarker(..))
    {
      continue;
    }
    for_each_expr_without_closures(cx.tcx.hir_body_owned_by(owner), |expr| {
      if let ExprKind::Path(QPath::Resolved(_, path)) = expr.kind
        && let Res::Def(DefKind::Fn, id) = path.res
        && let Some(id) = id.as_local()
      {
        referenced.insert(id);
      }
      ControlFlow::<(), ()>::Continue(())
    });
  }
  referenced
}

pub(super) struct Unwrap<'tcx> {
  pub receiver: &'tcx Expr<'tcx>,
  pub error: Option<Ty<'tcx>>,
  pub suffix: Span,
  pub message: Option<String>,
}

pub(super) fn standard_unwrap<'tcx>(
  cx: &LateContext<'tcx>,
  call: &'tcx Expr<'tcx>,
) -> Option<Unwrap<'tcx>> {
  let access = standard_access(cx, call)?;
  access.message.is_none().then_some(access)
}

pub(super) fn standard_access<'tcx>(
  cx: &LateContext<'tcx>,
  call: &'tcx Expr<'tcx>,
) -> Option<Unwrap<'tcx>> {
  let ExprKind::MethodCall(method, receiver, args, _) = call.kind else {
    return None;
  };
  if !matches!(method.ident.name, sym::unwrap | sym::expect)
    || call.span.from_expansion()
    || receiver.span.from_expansion()
    || !call.span.contains(receiver.span)
  {
    return None;
  }
  let types = cx.typeck_results();
  let ty::Adt(adt, parameters) = types.expr_ty(receiver).kind() else {
    return None;
  };
  let error = if cx.tcx.is_diagnostic_item(sym::Result, adt.did()) {
    Some(parameters.type_at(1))
  } else if cx.tcx.is_diagnostic_item(sym::Option, adt.did()) {
    None
  } else {
    return None;
  };
  let id = types.type_dependent_def_id(call.hir_id)?;
  if !types.expr_adjustments(receiver).is_empty()
    || cx.tcx.inherent_impl_of_assoc(id).is_none()
    || id.krate != adt.did().krate
  {
    return None;
  }
  let suffix = receiver.span.shrink_to_hi().with_hi(call.span.hi());
  let map = cx.sess().source_map();
  let compact = |span| {
    map
      .span_to_snippet(span)
      .ok()
      .map(|s| s.split_whitespace().collect::<String>())
  };
  let message = match (method.ident.name, args) {
    (sym::unwrap, []) if compact(suffix)?.as_str() == ".unwrap()" => None,
    (sym::expect, [message])
      if matches!(message.kind, ExprKind::Lit(_))
        && !message.span.from_expansion()
        && types.expr_ty(message).peel_refs().is_str()
        && suffix.contains(message.span)
        && compact(suffix.with_hi(message.span.lo()))?.as_str() == ".expect("
        && matches!(
          compact(suffix.with_lo(message.span.hi()))?.as_str(),
          ")" | ",)"
        ) =>
    {
      Some(map.span_to_snippet(message.span).ok()?)
    }
    _ => return None,
  };
  Some(Unwrap {
    receiver,
    error,
    suffix,
    message,
  })
}

fn unwrap_suffix<'tcx>(
  cx: &LateContext<'tcx>,
  call: &'tcx Expr<'tcx>,
  block: HirId,
) -> Option<(Ty<'tcx>, Span)> {
  let statement = match cx.tcx.parent_hir_node(call.hir_id) {
    Node::Stmt(statement) => statement,
    Node::LetStmt(local) => {
      let Node::Stmt(statement) = cx.tcx.parent_hir_node(local.hir_id) else {
        return None;
      };
      statement
    }
    _ => return None,
  };
  if cx.tcx.parent_hir_id(statement.hir_id) != block
    || statement.span.from_expansion()
    || !cx.tcx.hir_attrs(statement.hir_id).is_empty()
  {
    return None;
  }
  let extracted = standard_unwrap(cx, call)?;
  Some((extracted.error?, extracted.suffix))
}

pub(super) fn ordinary_test(cx: &LateContext<'_>, id: LocalDefId) -> bool {
  cx.sess().opts.test
    && cx.sess().edition().at_least_rust_2018()
    && is_test_function(cx.tcx, id)
    && !cx.tcx.def_span(id).from_expansion()
    && !rustc_hir::find_attr!(cx.tcx, id.to_def_id(), ShouldPanic { .. } | CfgAttrTrace)
}

pub(super) fn supported_source(cx: &LateContext<'_>, block: &Block<'_>, source: &str) -> bool {
  if !source.contains('#') {
    return true;
  }
  let formatting: HashSet<_> = block
    .stmts
    .iter()
    .filter(|statement| matches!(statement.kind, StmtKind::Let(_)))
    .flat_map(|statement| cx.tcx.hir_attrs(statement.hir_id))
    .filter_map(|attribute| {
      let Attribute::Unparsed(attribute) = attribute else {
        return None;
      };
      let [tool, name] = attribute.path.segments.as_ref() else {
        return None;
      };
      (tool.as_str() == "rustfmt"
        && name.as_str() == "skip"
        && matches!(attribute.args, AttrArgs::Empty)
        && !attribute.span.from_expansion()
        && block.span.contains(attribute.span))
      .then_some(attribute.span.lo())
    })
    .collect();
  // An inactive statement can disappear from HIR. Account for every written
  // attribute, accepting only a retained formatting directive on a local.
  let mut position = block.span.lo();
  for token in tokenize(source, FrontmatterAllowed::No) {
    if token.kind == TokenKind::Pound && !formatting.contains(&position) {
      return false;
    }
    position = position + BytePos(token.len);
  }
  true
}

pub(super) fn signature_edits(
  cx: &LateContext<'_>,
  block: &rustc_hir::Block<'_>,
  name: &str,
) -> Option<Vec<(Span, String)>> {
  let map = cx.sess().source_map();
  let mut edits = Vec::new();
  let closing = block.span.with_lo(block.span.hi() - BytePos(1));
  if map.span_to_snippet(closing).ok()? != "}" {
    return None;
  }
  let spaces = clippy_utils::source::indent_of(cx, closing)?;
  let indent = " ".repeat(spaces);
  let statement_indent = block
    .stmts
    .iter()
    .filter(|statement| !statement.span.from_expansion())
    .filter_map(|statement| clippy_utils::source::indent_of(cx, statement.span))
    .filter(|&indent| indent > spaces)
    .min()
    .unwrap_or(spaces + 4)
    .saturating_sub(spaces);
  edits.push((
    block.span.shrink_to_lo(),
    format!("-> ::core::result::Result<(), {name}> "),
  ));
  edits.push((
    closing.shrink_to_lo(),
    format!(
      "{}::core::result::Result::Ok(())\n{indent}",
      " ".repeat(statement_indent)
    ),
  ));
  Some(edits)
}

pub(super) fn propagate<'tcx>(
  cx: &LateContext<'tcx>,
  id: LocalDefId,
  decl: &FnDecl<'_>,
  body: &'tcx Body<'tcx>,
) -> Option<(HirId, Vec<(Span, String)>)> {
  if !ordinary_test(cx, id) {
    return None;
  }
  let ExprKind::Block(block, None) = body.value.kind else {
    return None;
  };
  let signature = cx.tcx.def_span(id);
  if signature.from_expansion() || block.span.from_expansion() {
    return None;
  }
  let mut calls = Vec::new();
  let source = cx.sess().source_map().span_to_snippet(block.span).ok()?;
  let edits_inside_block = supported_source(cx, block, &source);
  let mut asserted = 0;
  let mut early_return = false;
  for_each_expr_without_closures(body, |expr| {
    early_return |= matches!(expr.kind, ExprKind::Ret(_));
    if let Some((error, span)) = unwrap_suffix(cx, expr, block.hir_id) {
      calls.push((expr.hir_id, error, vec![(span, "?".to_owned())]));
    } else if edits_inside_block
      && expr.span.from_expansion()
      && let Some(call) = super::test_assertions::setup(cx, expr, block.hir_id, || {
        loop {
          asserted += 1;
          let name = if asserted == 1 {
            "asserted_value".to_owned()
          } else {
            format!("asserted_value_{asserted}")
          };
          if !source.contains(&name) {
            break name;
          }
        }
      })
    {
      calls.push(call);
    }
    ControlFlow::<(), ()>::Continue(())
  });
  let mut edits = Vec::new();
  let error = match decl.output {
    FnRetTy::DefaultReturn(_) => {
      if early_return || block.expr.is_some() {
        return None;
      }
      if !edits_inside_block {
        return None;
      }
      let module = cx.tcx.parent_module_from_def_id(id);
      let (error, name) = calls.iter().find_map(|&(_, error, _)| {
        let ty::Adt(adt, args) = error.kind() else {
          return None;
        };
        if !args.is_empty() || !adt.did().is_local() || adt.is_enum() && adt.variants().is_empty()
        {
          return None;
        }
        Some((
          error,
          super::local_type_path::local_path(cx, adt.did(), module)?,
        ))
      })?;
      edits = signature_edits(cx, block, &name)?;
      error
    }
    FnRetTy::Return(_) => {
      let result = cx.tcx.fn_sig(id).skip_binder().output().skip_binder();
      let ty::Adt(adt, args) = result.kind() else {
        return None;
      };
      if !cx.tcx.is_diagnostic_item(sym::Result, adt.did()) {
        return None;
      }
      args.type_at(1)
    }
  };
  // Infallible unwraps are edited individually. Grouping them too would
  // offer overlapping changes when the test already returns this error.
  if matches!(error.kind(), ty::Adt(adt, _) if adt.is_enum()
        && adt.variants().is_empty() && !adt.is_variant_list_non_exhaustive())
  {
    return None;
  }
  let first = calls.iter().find(|(_, ty, _)| *ty == error)?.0;
  edits.extend(
    calls
      .into_iter()
      .filter(|(_, ty, _)| *ty == error)
      .flat_map(|(_, _, changes)| changes),
  );
  Some((first, edits))
}
