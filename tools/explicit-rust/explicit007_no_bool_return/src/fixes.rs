use clippy_utils::{macros::macro_backtrace, visitors::for_each_expr_without_closures};
use rustc_hir::{
  Body, ExprKind, FnDecl, FnRetTy, ItemKind, Node, QPath,
  def::{CtorKind, DefKind, Res},
  def_id::{LocalDefId, LocalModDefId},
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span, Symbol};
use std::{
  collections::{HashMap, HashSet},
  ops::ControlFlow,
};

pub(super) type Edits = Vec<(Span, String)>;

pub(super) fn function_path(cx: &LateContext<'_>, id: LocalDefId) -> String {
  let id = id.to_def_id();
  let path = cx.tcx.def_path(id).to_string_no_crate_verbose();
  format!("{}{path}", cx.tcx.crate_name(id.krate))
}

fn core_macro(cx: &LateContext<'_>, id: rustc_hir::def_id::DefId) -> bool {
  !id.is_local()
    && matches!(
      cx.tcx.crate_name(id.krate).as_str(),
      "core" | "alloc" | "std"
    )
}

fn standard_core(cx: &LateContext<'_>) -> bool {
  use rustc_hir::{Attribute, CRATE_HIR_ID, attrs::AttributeKind};
  let (root, _, _) = cx
    .tcx
    .hir_get_module(rustc_hir::def_id::LocalModDefId::CRATE_DEF_ID);
  cx.sess().opts.externs.get("core").is_none()
    && !cx.tcx.hir_attrs(CRATE_HIR_ID).iter().any(|attr| {
      matches!(attr, Attribute::Parsed(AttributeKind::NoCore))
    })
    && !root.item_ids.iter().any(|&id| {
      matches!(cx.tcx.hir_item(id).kind, ItemKind::ExternCrate(Some(_), ident) if ident.as_str() == "core")
    })
}

fn unit_enum(
  cx: &LateContext<'_>,
  module: LocalModDefId,
  [name, yes, no]: &[String; 3],
) -> Option<Span> {
  if yes == no
    || [name, yes, no].into_iter().any(|word| {
      !word.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
        || !word
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
  {
    return None;
  }
  let id = cx
    .tcx
    .hir_module_items(module)
    .free_items()
    .find_map(|id| {
      let item = cx.tcx.hir_item(id);
      let ItemKind::Enum(ident, ..) = item.kind else {
        return None;
      };
      (ident.as_str() == name && cx.tcx.parent(id.owner_id.to_def_id()) == module.to_def_id())
        .then_some(id.owner_id.def_id)
    })?;
  let Node::Item(item) = cx.tcx.hir_node_by_def_id(id) else {
    return None;
  };
  let ItemKind::Enum(ident, ..) = item.kind else {
    return None;
  };
  let enum_type = cx.tcx.adt_def(id);
  let variants = enum_type.variants();
  let map = cx.sess().source_map();
  if item.span.from_expansion()
    || map
      .span_to_snippet(item.span.with_hi(ident.span.lo()))
      .ok()?
      .trim()
      != "enum"
    || cx.tcx.generics_of(id).count() != 0
    || enum_type.has_dtor(cx.tcx)
    || variants.len() != 2
    || variants.iter().any(|variant| {
      variant.ctor_kind() != Some(CtorKind::Const)
        || (variant.name.as_str() != yes && variant.name.as_str() != no)
    })
  {
    return None;
  }
  Some(item.span)
}

#[derive(Default)]
pub(super) struct Uses {
  paths: HashMap<LocalDefId, Vec<rustc_hir::HirId>>,
  standard_macros: HashSet<(BytePos, Symbol)>,
  standard_derives: HashSet<BytePos>,
}

impl Uses {
  fn collect(cx: &LateContext<'_>) -> Self {
    let mut uses = Self::default();
    for owner in cx.tcx.hir_body_owners() {
      for_each_expr_without_closures(cx.tcx.hir_body_owned_by(owner), |expr| {
        if let ExprKind::Path(QPath::Resolved(_, path)) = expr.kind
          && let Res::Def(DefKind::Fn, id) = path.res
          && let Some(id) = id.as_local()
        {
          uses.paths.entry(id).or_default().push(expr.hir_id);
        }
        for mac in macro_backtrace(expr.span) {
          if core_macro(cx, mac.def_id) {
            uses
              .standard_macros
              .insert((mac.span.lo(), cx.tcx.item_name(mac.def_id)));
          }
        }
        ControlFlow::<(), ()>::Continue(())
      });
    }
    for id in cx.tcx.hir_crate_items(()).free_items() {
      for expn in cx.tcx.hir_item(id).span.macro_backtrace() {
        if matches!(
          expn.kind,
          rustc_span::hygiene::ExpnKind::Macro(rustc_span::hygiene::MacroKind::Derive, _)
        ) && expn.macro_def_id.is_some_and(|id| core_macro(cx, id))
        {
          uses.standard_derives.insert(expn.call_site.lo());
        }
      }
    }
    uses
  }
}

// Check every written occurrence, including code that the current build has
// configured out. Reject source that can introduce unaccounted references.
fn proven_source(
  cx: &LateContext<'_>,
  source: Span,
  name: Symbol,
  mentions: &HashSet<BytePos>,
  declarations: [Span; 2],
  uses: &Uses,
) -> Option<()> {
  let source_text = cx.sess().source_map().span_to_snippet(source).ok()?;
  let mut offset = 0;
  let mut written = HashSet::new();
  let mut tokens = Vec::new();
  for token in tokenize(&source_text, FrontmatterAllowed::No) {
    let start = offset;
    offset += token.len as usize;
    if matches!(
      token.kind,
      TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    ) {
      continue;
    }
    let word = source_text.get(start..offset)?;
    let position = source.lo() + BytePos(u32::try_from(start).ok()?);
    if matches!(token.kind, TokenKind::Ident | TokenKind::RawIdent)
      && word.trim_start_matches("r#") == name.as_str()
    {
      written.insert(position);
    }
    tokens.push((word, position));
  }
  if &written != mentions {
    return None;
  }
  let words: Vec<_> = tokens.iter().map(|(word, _)| *word).collect();
  for (i, &(word, position)) in tokens.iter().enumerate() {
    let tail = &words[i..];
    if matches!(
      tail,
      ["mod", _, ";", ..] | ["include" | "macro_rules", "!", ..]
    ) {
      return None;
    }
    // These absolute core macros expand to literals. In particular, a line!
    // inside concat! leaves no HIR expression with its own expansion span.
    let absolute_core = words[..i].ends_with(&[":", ":", "core", ":", ":"]);
    let literal_macro = absolute_core && matches!(word, "concat" | "file" | "line");
    if !literal_macro
      && matches!(tail, [_, "!", "(" | "{" | "[", ..])
      && word.starts_with(|c: char| c == '_' || c.is_alphabetic())
      && !matches!(word, "if" | "while" | "match" | "return")
      && !uses
        .standard_macros
        .contains(&(position, Symbol::intern(word)))
      && !(absolute_core
        && uses
          .standard_macros
          .contains(&(tokens[i - 5].1, Symbol::intern(word))))
    {
      return None;
    }
    let ["#", attrs @ ..] = tail else { continue };
    let attrs = attrs.strip_prefix(&["!"]).unwrap_or(attrs);
    match attrs {
      ["[", "cfg", ..] => {
        if declarations
          .iter()
          .any(|span| span.lo() <= position && position < span.hi())
        {
          return None;
        }
        // A configured declaration may resolve differently in a
        // different build, even when it currently has only one use.
        let mut depth = 0;
        let mut end = i + 1;
        for &token in &words[i + 1..] {
          end += 1;
          match token {
            "[" => depth += 1,
            "]" => {
              depth -= 1;
              if depth == 0 {
                break;
              }
            }
            _ => {}
          }
        }
        // Only inspect the decorated item, skipping its other outer attrs.
        while words
          .get(end..)
          .is_some_and(|rest| rest.starts_with(&["#", "["]))
        {
          end += words[end..].iter().position(|word| *word == "]")? + 1;
        }
        if tokens
          .get(end)
          .is_some_and(|(_, pos)| declarations.iter().any(|span| span.lo() == *pos))
        {
          return None;
        }
      }
      ["[", "derive", "(", ..] => {
        let start = i + (tail.get(1) == Some(&"!")) as usize + 4;
        if !tokens[start..]
          .iter()
          .take_while(|(word, _)| *word != ")")
          .all(|(word, pos)| *word == "," || uses.standard_derives.contains(pos))
        {
          return None;
        }
      }
      [
        "[",
        "allow" | "deny" | "doc" | "expect" | "forbid" | "must_use" | "test" | "track_caller"
        | "warn" | "inline" | "cold" | "ignore" | "should_panic" | "repr",
        ..,
      ] => {}
      _ => return None,
    }
  }
  Some(())
}

pub(super) fn plan(
  cx: &LateContext<'_>,
  id: LocalDefId,
  decl: &FnDecl<'_>,
  body: &Body<'_>,
  outcomes @ [name, yes, no]: &[String; 3],
  uses: &mut Option<Uses>,
) -> Option<Edits> {
  let Node::Item(item) = cx.tcx.hir_node_by_def_id(id) else {
    return None;
  };
  let ItemKind::Fn { ident, sig, .. } = item.kind else {
    return None;
  };
  let ExprKind::Block(block, _) = body.value.kind else {
    return None;
  };
  let FnRetTy::Return(result) = decl.output else {
    return None;
  };
  let module = cx.tcx.parent_module_from_def_id(id);
  let map = cx.sess().source_map();
  let body_span = body.value.span;
  let tail = block.expr?.span.source_callsite();
  if !cx.sess().edition().at_least_rust_2018()
    || !standard_core(cx)
    || item.span.from_expansion()
    || body_span.from_expansion()
    || result.span.from_expansion()
    || sig.header.abi != rustc_abi::ExternAbi::Rust
    || cx.tcx.parent(id.to_def_id()) != module.to_def_id()
    || cx.tcx.visibility(id) != ty::Visibility::Restricted(module.to_def_id())
    || cx.tcx.codegen_fn_attrs(id).contains_extern_indicator()
    || cx.tcx.generics_of(id).count() != 0
    || map
      .span_to_snippet(item.span.with_hi(ident.span.lo()))
      .ok()?
      .trim()
      != "fn"
    || map.span_to_snippet(result.span).ok()? != "bool"
    || tail == body_span
    || !body_span.contains(tail)
  {
    return None;
  }
  let explicit_return = for_each_expr_without_closures(body, |expr| {
    if matches!(expr.kind, ExprKind::Ret(_)) {
      return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
  });
  if explicit_return.is_some() {
    return None;
  }
  let enum_span = unit_enum(cx, module, outcomes)?;
  let (module_node, outer_span, _) = cx.tcx.hir_get_module(module);
  let source = module_node.spans.inner_span;
  if outer_span.from_expansion()
    || source.from_expansion()
    || !source.contains(item.span)
    || !source.contains(enum_span)
  {
    return None;
  }

  let uses = uses.get_or_insert_with(|| Uses::collect(cx));
  let enum_path = format!("self::{name}");
  let original = map.span_to_snippet(tail).ok()?;
  let indent = " ".repeat(map.lookup_char_pos(tail.lo()).col.0);
  let body_indent = map.lookup_char_pos(item.span.lo()).col.0;
  let step = " ".repeat(indent.len().saturating_sub(body_indent).clamp(2, 4));
  let replacement = format!(
    "match ::core::primitive::bool::then_some({original}, ()) {{\n\
     {indent}{step}::core::option::Option::Some(()) => {enum_path}::{yes},\n\
     {indent}{step}::core::option::Option::None => {enum_path}::{no},\n\
     {indent}}}"
  );
  let mut edits = vec![(result.span, enum_path.clone()), (tail, replacement)];
  let mut mentions = HashSet::from([ident.span.lo()]);
  for &reference in uses.paths.get(&id).into_iter().flatten() {
    let Node::Expr(callee) = cx.tcx.hir_node(reference) else {
      return None;
    };
    let Node::Expr(call) = cx.tcx.parent_hir_node(reference) else {
      return None;
    };
    if !matches!(call.kind, ExprKind::Call(function, _) if function.hir_id == reference)
      || callee.span.from_expansion()
      || call.span.from_expansion()
      || cx.tcx.parent_module_from_def_id(reference.owner.def_id) != module
      || !source.contains(call.span)
      || edits.iter().any(|(span, _)| span.overlaps(call.span))
      || map.span_to_snippet(callee.span).ok()?.as_str() != ident.as_str()
    {
      return None;
    }
    mentions.insert(callee.span.lo());
    let call_text = map.span_to_snippet(call.span).ok()?;
    edits.push((
      call.span,
      format!("::core::matches!({call_text}, {enum_path}::{yes})"),
    ));
  }
  proven_source(
    cx,
    source,
    ident.name,
    &mentions,
    [item.span, enum_span],
    uses,
  )?;
  Some(edits)
}
