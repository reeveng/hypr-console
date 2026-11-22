//! Return a test failure at the authored read, including reads in assertion operands.
use super::test_results::{self, Unwrap};
use clippy_utils::{
  macros::root_macro_call_first_node, sym, ty::implements_trait,
  visitors::for_each_expr_without_closures,
};
use rustc_hir::{Body, Expr, ExprKind, FnDecl, FnRetTy, HirId, Node, def_id::LocalDefId};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{BytePos, Span};
use std::{
  collections::{BTreeMap, HashSet},
  ops::ControlFlow,
};

struct Read<'tcx> {
  expr: &'tcx Expr<'tcx>,
  access: Unwrap<'tcx>,
}

enum Failure<'tcx> {
  Error(Ty<'tcx>),
  Missing,
}

// Some macros forward their input's HIR untouched, even when their matcher
// requires a particular token sequence. Only rewrite inside actual standard
// assertion invocations. The lexer also sees opaque nested invocations.
fn macro_regions(source: &str, start: BytePos) -> Option<Vec<(BytePos, BytePos)>> {
  let mut regions = Vec::new();
  let mut groups = Vec::new();
  let mut bang = None;
  let mut pos = start;
  for token in tokenize(source, FrontmatterAllowed::No) {
    let lo = pos;
    pos = pos + BytePos(token.len);
    match token.kind {
      TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. } => {
        continue;
      }
      TokenKind::Bang => {
        bang = Some(lo);
        continue;
      }
      TokenKind::OpenParen | TokenKind::OpenBracket | TokenKind::OpenBrace => groups.push(bang),
      TokenKind::CloseParen | TokenKind::CloseBracket | TokenKind::CloseBrace => {
        if let Some(open) = groups.pop()? {
          regions.push((open, pos));
        }
      }
      _ => {}
    }
    bang = None;
  }
  Some(regions)
}

fn boxed_error(cx: &LateContext<'_>, target: Ty<'_>) -> bool {
  let ty::Adt(_, args) = target.kind() else {
    return false;
  };
  if !target.is_box() {
    return false;
  }
  let ty::Dynamic(predicates, _) = args.type_at(0).kind() else {
    return false;
  };
  predicates
    .principal_def_id()
    .is_some_and(|id| cx.tcx.is_diagnostic_item(sym::Error, id))
}

fn diagnostic_error<'tcx>(
  cx: &LateContext<'tcx>,
  failure: &Failure<'tcx>,
  access: &Unwrap<'tcx>,
  error_name: &str,
) -> Option<String> {
  let missing = access.message.as_deref().unwrap_or(
    "::core::concat!(::core::file!(), \":\", ::core::line!(), \": expected Some, found None\")",
  );
  match (failure, access.error) {
    (Failure::Missing, None) => Some(missing.to_owned()),
    (Failure::Error(target), Some(source)) if *target == source && access.message.is_none() => {
      Some(error_name.to_owned())
    }
    (Failure::Error(target), source) if boxed_error(cx, *target) => {
      let (source, message) = match source {
        Some(error) if access.message.is_none() => (error, error_name.to_owned()),
        Some(_) => {
          let string = cx
            .tcx
            .type_of(cx.tcx.lang_items().string()?)
            .instantiate_identity()
            .skip_norm_wip();
          let context = access.message.as_deref()?;
          let message = format!(
            "{{ extern crate alloc as test_report_alloc; test_report_alloc::format!(\"{{}}: {{:?}}\", {context}, {error_name}) }}"
          );
          (string, message)
        }
        None => (
          Ty::new_imm_ref(cx.tcx, cx.tcx.lifetimes.re_static, cx.tcx.types.str_),
          missing.to_owned(),
        ),
      };
      implements_trait(
        cx,
        *target,
        cx.tcx.get_diagnostic_item(sym::From)?,
        &[source.into()],
      )
      .then(|| format!("::core::convert::From::from({message})"))
    }
    _ => None,
  }
}

fn infallible(error: Ty<'_>) -> bool {
  matches!(error.kind(), ty::Adt(adt, _) if adt.is_enum() && adt.variants().is_empty() && !adt.is_variant_list_non_exhaustive())
}

pub(super) fn edits<'tcx>(
  cx: &LateContext<'tcx>,
  id: LocalDefId,
  decl: &FnDecl<'_>,
  body: &'tcx Body<'tcx>,
) -> Option<(HirId, Vec<(Span, String)>)> {
  if !test_results::ordinary_test(cx, id) {
    return None;
  }
  let ExprKind::Block(block, None) = body.value.kind else {
    return None;
  };
  let map = cx.sess().source_map();
  let source = map.span_to_snippet(block.span).ok()?;
  if !test_results::supported_source(cx, block, &source) {
    return None;
  }
  let regions = macro_regions(&source, block.span.lo())?;
  let mut standard = Vec::new();
  let mut direct = HashSet::new();
  let mut sites = Vec::new();
  let mut explicit_return = false;
  for_each_expr_without_closures(body, |expr| {
    explicit_return |= matches!(expr.kind, ExprKind::Ret(_));
    if expr.span.from_expansion()
      && let Some(call) = root_macro_call_first_node(cx, expr)
      && let Some(name) = cx.tcx.get_diagnostic_name(call.def_id)
      && matches!(
        name,
        sym::assert_macro
          | sym::assert_eq_macro
          | sym::assert_ne_macro
          | sym::debug_assert_macro
          | sym::debug_assert_eq_macro
          | sym::debug_assert_ne_macro
      )
    {
      standard.push(call.span);
      if let Some((_, site, _)) = super::test_assertions::edits(cx, expr) {
        direct.insert(site);
      }
    }
    if let Some(access) = test_results::standard_access(cx, expr)
      && block.span.contains(expr.span)
      && cx.tcx.hir_enclosing_body_owner(expr.hir_id) == id
      && !access.error.is_some_and(infallible)
    {
      sites.push(Read { expr, access });
    }
    ControlFlow::<(), ()>::Continue(())
  });
  sites.retain(|site| {
    !direct.contains(&site.expr.hir_id)
      && regions.iter().all(|&(bang, end)| {
        !(bang < site.expr.span.lo() && site.expr.span.hi() < end)
          || standard
            .iter()
            .any(|span| span.lo() <= bang && span.hi() == end)
      })
  });
  if sites.is_empty() {
    return None;
  }
  let (failure, mut changes) = match decl.output {
    FnRetTy::Return(_) => {
      let declared = cx.tcx.fn_sig(id).skip_binder().output().skip_binder();
      let ty::Adt(adt, args) = declared.kind() else {
        return None;
      };
      if !cx.tcx.is_diagnostic_item(sym::Result, adt.did()) {
        return None;
      }
      let error = args.type_at(1);
      let selected = if matches!(error.kind(), ty::Ref(_, inner, ty::Mutability::Not) if inner.is_str())
      {
        Failure::Missing
      } else {
        Failure::Error(error)
      };
      (selected, Vec::new())
    }
    FnRetTy::DefaultReturn(_) => {
      if explicit_return || block.expr.is_some() {
        return None;
      }
      let options = sites
        .iter()
        .filter(|read| read.access.error.is_none())
        .count();
      let mut counts = Vec::<(Ty<'tcx>, usize)>::new();
      for site in &sites {
        if let Some(error) = site.access.error
          && site.access.message.is_none()
          && matches!(error.kind(), ty::Adt(_, args) if args.is_empty())
        {
          if let Some((_, n)) = counts.iter_mut().find(|(other, _)| *other == error) {
            *n += 1;
          } else {
            counts.push((error, 1));
          }
        }
      }
      let (failure, name) = if let Some((error, n)) = counts.into_iter().max_by_key(|(_, n)| *n)
        && n >= options
      {
        let ty::Adt(adt, _) = error.kind() else {
          return None;
        };
        let path = adt
          .did()
          .is_local()
          .then(|| {
            super::local_type_path::local_path(
              cx,
              adt.did(),
              cx.tcx.parent_module_from_def_id(id),
            )
          })
          .flatten()
          .unwrap_or_else(|| "impl ::core::fmt::Debug".to_owned());
        (Failure::Error(error), path)
      } else if options > 0 {
        (Failure::Missing, "&'static str".to_owned())
      } else {
        return None;
      };
      (failure, test_results::signature_edits(cx, block, &name)?)
    }
  };
  let mut first = None;
  let module = cx.tcx.parent_module_from_def_id(id);
  let children = cx.tcx.module_children_local(module.to_local_def_id());
  let name = |stem: &str| {
    (1_u32..)
      .map(|n| {
        if n == 1 {
          stem.to_owned()
        } else {
          format!("{stem}_{n}")
        }
      })
      .find(|candidate| {
        !source.contains(candidate)
          && !children
            .iter()
            .any(|child| child.ident.as_str() == candidate)
      })
  };
  let success_name = name("read_value")?;
  let error_name = name("read_error")?;
  let mut prefixes = BTreeMap::<BytePos, Vec<(BytePos, Span, &str)>>::new();
  let mut seen = HashSet::new();
  for read in sites {
    let Some(error) = diagnostic_error(cx, &failure, &read.access, &error_name) else {
      continue;
    };
    let span = read.expr.span;
    if !seen.insert((span.lo(), span.hi())) {
      continue;
    }
    first.get_or_insert(read.expr.hir_id);
    let parentheses = matches!(cx.tcx.parent_hir_node(read.expr.hir_id), Node::Expr(parent) if matches!(
      parent.kind, ExprKind::Binary(_, left, _) | ExprKind::Cast(left, _) if left.hir_id == read.expr.hir_id));
    let prefix = if parentheses { "(match " } else { "match " };
    let end = if parentheses { ")" } else { "" };
    let (success, failed) = if read.access.error.is_some() {
      (
        "::core::result::Result::Ok",
        format!("::core::result::Result::Err({error_name})"),
      )
    } else {
      (
        "::core::option::Option::Some",
        "::core::option::Option::None".to_owned(),
      )
    };
    prefixes
      .entry(span.lo())
      .or_default()
      .push((span.hi(), span.shrink_to_lo(), prefix));
    changes.push((read.access.suffix, format!(" {{ {success}({success_name}) => {success_name}, {failed} => return ::core::result::Result::Err({error}) }}{end}")));
  }
  // Chained reads can begin at the same byte. Rustfix needs one insertion
  // there, with the outermost match first.
  for mut entries in prefixes.into_values() {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let span = entries.first()?.1;
    changes.push((span, entries.into_iter().map(|entry| entry.2).collect()));
  }
  Some((first?, changes))
}
