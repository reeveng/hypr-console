use clippy_utils::macros::root_macro_call;
use rustc_hir::{
  Attribute, ItemKind, Node, Pat, QPath,
  attrs::AttributeKind,
  def::{CtorKind, DefKind, Res},
  def_id::LocalDefId,
};
use rustc_lexer::{FrontmatterAllowed, LiteralKind, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;
use std::path::{Component, Path, PathBuf};

use super::local_type_path::{identifier, local_path};

fn tokens(text: &str) -> impl Iterator<Item = (TokenKind, &str)> {
  let mut offset = 0;
  tokenize(text, FrontmatterAllowed::No).filter_map(move |token| {
    let start = offset;
    offset += token.len as usize;
    (!matches!(
      token.kind,
      TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    ))
    .then(|| (token.kind, &text[start..offset]))
  })
}

fn conditional_source(text: &str) -> bool {
  let words: Vec<_> = tokens(text).map(|(_, word)| word).collect();
  words
    .windows(3)
    .any(|parts| matches!(parts, ["#", "[", "cfg" | "cfg_attr"]))
    || words
      .windows(4)
      .any(|parts| matches!(parts, ["#", "!", "[", "cfg" | "cfg_attr"]))
}

fn conditional_item(cx: &LateContext<'_>, id: LocalDefId) -> bool {
  cx.tcx
    .hir_attrs(cx.tcx.local_def_id_to_hir_id(id))
    .iter()
    .any(|attr| {
      matches!(
        attr,
        Attribute::Parsed(AttributeKind::CfgTrace(..) | AttributeKind::CfgAttrTrace)
      )
    })
}

fn normalized(path: &Path) -> Option<PathBuf> {
  let mut result = PathBuf::new();
  for component in path.components() {
    match component {
      Component::ParentDir if !result.pop() => return None,
      Component::ParentDir | Component::CurDir => {}
      _ => result.push(component),
    }
  }
  Some(result)
}

fn literal_include(cx: &LateContext<'_>, module: Span, declaration: Span) -> Option<()> {
  if declaration.from_expansion() {
    let call = root_macro_call(declaration)?;
    if !module.contains(call.span)
      || call.span.from_expansion()
      || cx.tcx.crate_name(call.def_id.krate).as_str() != "core"
      || cx.tcx.item_name(call.def_id).as_str() != "include"
    {
      return None;
    }
  }
  let map = cx.sess().source_map();
  let source = map.span_to_snippet(module).ok()?;
  let source: Vec<_> = tokens(&source).collect();
  let mut includes = source
    .windows(2)
    .enumerate()
    .filter_map(|(index, pair)| matches!(pair, [(_, "include"), (_, "!")]).then_some(index));
  let index = includes.next()?;
  if includes.next().is_some()
    || matches!(
      source[..index].last(),
      Some((TokenKind::CloseBracket | TokenKind::Colon, _))
    )
  {
    return None;
  }
  let depth = source[..index]
    .iter()
    .fold(0isize, |depth, (kind, _)| match kind {
      TokenKind::OpenBrace | TokenKind::OpenParen | TokenKind::OpenBracket => depth + 1,
      TokenKind::CloseBrace | TokenKind::CloseParen | TokenKind::CloseBracket => depth - 1,
      _ => depth,
    });
  let module_brace = isize::from(matches!(source.first(), Some((TokenKind::OpenBrace, _))));
  if depth != module_brace {
    return None;
  }
  let [
    (_, "include"),
    (_, "!"),
    (_, "("),
    (
      TokenKind::Literal {
        kind: LiteralKind::Str { terminated: true },
        ..
      },
      quoted,
    ),
    (_, ")"),
    ..,
  ] = &source[index..]
  else {
    return None;
  };
  let literal = quoted.strip_prefix('"')?.strip_suffix('"')?;
  if literal.contains('\\') {
    return None;
  }
  let containing = map
    .lookup_source_file(module.lo())
    .name
    .clone()
    .into_local_path()?;
  let included = map
    .lookup_source_file(declaration.lo())
    .name
    .clone()
    .into_local_path()?;
  (normalized(&containing.parent()?.join(literal))? == normalized(&included)?).then_some(())
}

pub(super) fn siblings(
  cx: &LateContext<'_>,
  pattern: &Pat<'_>,
  path: &QPath<'_>,
) -> Option<Vec<String>> {
  let QPath::Resolved(None, path) = path else {
    return None;
  };
  let [qualifier, _] = path.segments else {
    return None;
  };
  let adt = cx
    .typeck_results()
    .pat_ty(pattern)
    .peel_refs()
    .ty_adt_def()?;
  let definition = adt.did().as_local()?;
  if !adt.is_enum()
    || qualifier.res != Res::Def(DefKind::Enum, adt.did())
    || qualifier.ident.name != cx.tcx.item_name(adt.did())
    || conditional_item(cx, definition)
  {
    return None;
  }
  let Node::Item(item) = cx.tcx.hir_node_by_def_id(definition) else {
    return None;
  };
  let ItemKind::Enum(..) = item.kind else {
    return None;
  };
  let map = cx.sess().source_map();
  let declaration = map.span_to_snippet(item.span).ok()?;
  if tokens(&declaration).any(|(kind, _)| kind == TokenKind::Pound) {
    return None;
  }
  // Keep the written qualifier tied to its stable declaration in the
  // module or an ancestor, including configurations absent from HIR.
  // A gate on a shared ancestor governs both the enum and the use.
  let module = cx
    .tcx
    .parent_module_from_def_id(pattern.hir_id.owner.def_id);
  let definition_module = cx.tcx.parent_module_from_def_id(definition);
  let (defining, _, _) = cx.tcx.hir_get_module(definition_module);
  let definition_source = defining.spans.inner_span;
  if item.span.from_expansion() || !definition_source.contains(item.span) {
    // rustc gives included items authored spans. Check the literal source
    // path even when the enum has no macro expansion in its backtrace.
    literal_include(cx, definition_source, item.span)?;
  }
  let mut at = module;
  while at != definition_module {
    if at.is_top_level_module() {
      return None;
    }
    let (inner, _, _) = cx.tcx.hir_get_module(at);
    if conditional_source(&map.span_to_snippet(inner.spans.inner_span).ok()?) {
      return None;
    }
    at = cx.tcx.parent_module_from_def_id(at.to_local_def_id());
  }
  for (_, node) in cx.tcx.hir_parent_iter(pattern.hir_id) {
    if let Node::Block(block) = node
      && (!block.span.contains(pattern.span)
        || conditional_source(
          &map
            .span_to_snippet(block.span.with_hi(pattern.span.lo()))
            .ok()?,
        ))
    {
      return None;
    }
  }

  let constructor = path.res.opt_def_id()?;
  let matched = adt
    .variants()
    .iter()
    .find(|entry| entry.def_id == constructor || entry.ctor_def_id() == Some(constructor))?;
  let prefix = local_path(cx, adt.did(), module)?;
  Some(
    adt
      .variants()
      .iter()
      .filter(|entry| entry.def_id != matched.def_id)
      .map(|entry| {
        let name = identifier(cx, entry.name);
        let fields = match entry.ctor_kind() {
          Some(CtorKind::Const) => "",
          Some(CtorKind::Fn) => "(..)",
          None => " { .. }",
        };
        format!("{prefix}::{name}{fields}")
      })
      .collect(),
  )
}
