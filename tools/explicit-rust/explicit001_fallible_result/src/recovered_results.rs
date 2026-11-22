//! Explicit projections for uninhabited errors and optional reads.

use rustc_hir::{
    BindingMode, Expr, ExprKind, ItemKind, MatchSource, Node, PatKind,
  def::{DefKind, Res},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::sym;

fn core_path_is_standard(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.externs.get("core").is_none()
        && !cx.tcx.hir_root_module().item_ids.iter().any(|&item| {
            matches!(cx.tcx.hir_item(item).kind, ItemKind::ExternCrate(Some(original), ident)
                if ident.name == sym::core && original != sym::core)
        })
}

fn empty_handler(cx: &LateContext<'_>, closure: &Expr<'_>) -> bool {
  let ExprKind::Closure(closure) = closure.kind else {
    return false;
  };
  let body = cx.tcx.hir_body(closure.body);
  let [parameter] = body.params else {
    return false;
  };
  let PatKind::Binding(BindingMode::NONE, binding, _, None) = parameter.pat.kind else {
    return false;
  };
  let ExprKind::Match(value, [], MatchSource::Normal) = body.value.kind else {
    return false;
  };
  matches!(value.kind, ExprKind::Path(ref path) if cx.qpath_res(path, value.hir_id) == Res::Local(binding))
}

fn returns_search_position(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
  let ExprKind::MethodCall(segment, _, _, _) = receiver.kind else {
    return false;
  };
  if !matches!(
    segment.ident.as_str(),
    "binary_search" | "binary_search_by" | "binary_search_by_key"
  ) {
    return false;
  }
  let Some(method) = cx.typeck_results().type_dependent_def_id(receiver.hir_id) else {
    return false;
  };
  cx.tcx.crate_name(method.krate) == sym::core
    && cx
      .tcx
      .inherent_impl_of_assoc(method)
      .is_some_and(|implementation| {
        cx.tcx
          .type_of(implementation)
          .instantiate_identity()
          .skip_norm_wip()
          .is_slice()
      })
}

fn failure_name(
  cx: &LateContext<'_>,
  call: &Expr<'_>,
  receiver: &Expr<'_>,
  error: ty::Ty<'_>,
  method: &str,
) -> Option<&'static str> {
  if returns_search_position(cx, receiver) {
    return Some("_insertion_position");
  }
  let ty::Adt(error, _) = error.kind() else {
    return None;
  };
  let cause = match (
    cx.tcx.crate_name(error.did().krate).as_str(),
    cx.tcx.item_name(error.did()).as_str(),
  ) {
    ("core", "TryFromIntError") => "_outside_integer_range",
    ("core", "TryFromSliceError") => "_wrong_slice_length",
    ("core", "Utf8Error") | ("alloc", "FromUtf8Error") => "_invalid_utf8",
    ("core", "ParseIntError") => "_invalid_integer",
    ("core", "ParseFloatError") => "_invalid_float",
    ("core", "ParseBoolError") => "_invalid_boolean",
    ("console_core_font_reading", "ReadError")
      if method == "ok"
        && cx.tcx.def_path(error.did()).to_string_no_crate_verbose() == "::read::ReadError" =>
    {
      // The caller already publishes absence as its answer for an unreadable field.
      let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
      let output = cx
        .typeck_results()
        .expr_ty(cx.tcx.hir_body_owned_by(owner).value);
      if !output
        .ty_adt_def()
        .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Option, adt.did()))
      {
        return None;
      }
      "_unreadable_font_data"
    }
    _ => return None,
  };
  Some(cause)
}

pub(super) fn replacement<'tcx>(
  cx: &LateContext<'tcx>,
  call: &'tcx Expr<'tcx>,
  receiver: &'tcx Expr<'tcx>,
  method: &str,
) -> Option<String> {
  let types = cx.typeck_results();
  let ty::Adt(result, parameters) = types.expr_ty(receiver).kind() else {
    return None;
  };
  let id = types.type_dependent_def_id(call.hir_id)?;
  if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
    || id.krate != result.did().krate
    || cx.tcx.inherent_impl_of_assoc(id).is_none()
    || !cx.sess().edition().at_least_rust_2018()
        || !core_path_is_standard(cx)
    || call.span.from_expansion()
    || receiver.span.from_expansion()
  {
    return None;
  }
  let error = parameters.type_at(1);
  let inhabited = !error.is_never()
    && !matches!(error.kind(), ty::Adt(adt, _)
        if adt.is_enum() && adt.variants().is_empty() && !adt.is_variant_list_non_exhaustive());
  let mut arms = if !inhabited
    && method == "unwrap_or_else"
    && let ExprKind::MethodCall(_, _, [handler], _) = call.kind
    && empty_handler(cx, handler)
  {
    "::core::result::Result::Ok(value) => value".to_owned()
  } else {
    let cause = failure_name(cx, call, receiver, error, method)?;
    match method {
      "ok" => format!(
        "::core::result::Result::Ok(value) => ::core::option::Option::Some(value), ::core::result::Result::Err({cause}) => ::core::option::Option::None"
      ),
      "is_ok" => format!(
        "::core::result::Result::Ok(_) => true, ::core::result::Result::Err({cause}) => false"
      ),
      "is_err" => format!(
        "::core::result::Result::Ok(_) => false, ::core::result::Result::Err({cause}) => true"
      ),
      _ => return None,
    }
  };
  if !super::macro_input::outside_macro_input(cx, call.span) {
    return None;
  }
  let map = cx.sess().source_map();
  let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
  let module = cx.tcx.parent_module_from_def_id(owner).to_local_def_id();
  let file = map.lookup_source_file(call.span.lo());
  let source = file.src.as_ref()?;
  for binding in [
    "value",
    "_invalid_utf8",
    "_invalid_integer",
    "_invalid_float",
    "_invalid_boolean",
    "_unreadable_font_data",
    "_outside_integer_range",
    "_wrong_slice_length",
    "_insertion_position",
  ] {
    if !arms.contains(binding) {
      continue;
    }
    let mut fresh = binding.to_owned();
    while source.contains(&fresh)
      || cx.tcx.module_children_local(module).iter().any(|child| {
        child.ident.as_str() == fresh
          && matches!(
            child.res,
            Res::Def(
              DefKind::Const { .. } | DefKind::Static { .. } | DefKind::Ctor(..),
              _
            )
          )
      })
    {
      fresh.push('_');
    }
    if fresh != binding {
      arms = arms.replace(binding, &fresh);
    }
  }
  let suffix = map
    .span_to_snippet(receiver.span.shrink_to_hi().to(call.span.shrink_to_hi()))
    .ok()?;
  if suffix.contains("//") || suffix.contains("/*") {
    return None;
  }
  let input = map.span_to_snippet(receiver.span).ok()?;
  let scrutinee = match method {
    "is_ok" | "is_err" => format!("&({input})"),
    _ => input,
  };
  let output = format!("match {scrutinee} {{ {arms} }}");
  let grouped = matches!(cx.tcx.parent_hir_node(call.hir_id), Node::Expr(parent)
        if matches!(parent.kind, ExprKind::Binary(_, left, _) | ExprKind::Index(left, _, _) | ExprKind::Cast(left, _) if left.hir_id == call.hir_id));
  Some(if grouped {
    format!("({output})")
  } else {
    output
  })
}
