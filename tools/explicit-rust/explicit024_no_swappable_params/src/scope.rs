//! Check the source that can name a private item, including disabled code.
use clippy_utils::macros::macro_backtrace;
use clippy_utils::visitors::for_each_expr_without_closures;
use rustc_hir::{
    ExprKind, HirId, QPath,
    def::{DefKind, Res},
    def_id::LocalDefId,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

#[derive(Default)]
pub(super) struct References {
    pub paths: HashMap<LocalDefId, Vec<HirId>>,
    standard_macros: HashSet<(BytePos, String)>,
    leaf_calls: HashMap<(BytePos, String), LocalDefId>,
    leaf_names: HashMap<LocalDefId, Option<HashSet<String>>>,
    archmage: HashSet<BytePos>,
    module_sources: HashMap<BytePos, Span>,
    derives: HashSet<BytePos>,
}
impl References {
    pub fn collect(cx: &LateContext<'_>) -> Self {
        let mut found = Self::default();
        let mut recorded_standard = HashSet::new();
        for id in cx.tcx.hir_crate_items(()).free_items() {
            let item = cx.tcx.hir_item(id);
            if let rustc_hir::ItemKind::Mod(ident, module) = item.kind
                && !ident.span.from_expansion()
            {
                found
                    .module_sources
                    .insert(ident.span.lo(), module.spans.inner_span);
            }
            for expansion in item.span.macro_backtrace() {
                if matches!(
                    expansion.kind,
                    rustc_span::hygiene::ExpnKind::Macro(
                        rustc_span::hygiene::MacroKind::Derive,
                        _
                    )
                ) && let Some(id) = expansion.macro_def_id
                    && matches!(
                        cx.tcx.crate_name(id.krate).as_str(),
                        "core" | "std" | "alloc"
                    )
                {
                    found.derives.insert(expansion.call_site.lo());
                }
            }
        }
        for owner in cx.tcx.hir_body_owners() {
            for expn in cx
                .tcx
                .hir_span(cx.tcx.local_def_id_to_hir_id(owner))
                .macro_backtrace()
            {
                if let Some(id) = expn.macro_def_id
                    && !expn.call_site.from_expansion()
                    && cx.tcx.crate_name(id.krate).as_str() == "archmage_macros"
                    && matches!(cx.tcx.item_name(id).as_str(), "arcane" | "rite")
                {
                    found.archmage.insert(expn.call_site.lo());
                }
            }
            for_each_expr_without_closures(cx.tcx.hir_body_owned_by(owner), |expr| {
                if let ExprKind::Path(QPath::Resolved(_, path)) = expr.kind
                    && let Res::Def(DefKind::Fn, id) = path.res
                    && let Some(id) = id.as_local()
                {
                    found.paths.entry(id).or_default().push(expr.hir_id);
                }
                for mac in macro_backtrace(expr.span) {
                    if matches!(
                        cx.tcx.crate_name(mac.def_id.krate).as_str(),
                        "core" | "std" | "alloc"
                    ) {
                        if recorded_standard.insert(mac.expn) {
                            let position = if mac.span.from_expansion() {
                                mac.span.lo()
                            } else {
                                macro_name_position(cx, mac.span).unwrap_or(mac.span.lo())
                            };
                            found
                                .standard_macros
                                .insert((position, cx.tcx.item_name(mac.def_id).to_string()));
                        }
                    } else if !mac.span.from_expansion()
                        && let Some(id) = mac.def_id.as_local()
                    {
                        found.leaf_calls.insert(
                            (mac.span.lo(), cx.tcx.item_name(mac.def_id).to_string()),
                            id,
                        );
                    }
                }
                ControlFlow::<(), ()>::Continue(())
            });
        }
        for &id in found.leaf_calls.values() {
            found
                .leaf_names
                .entry(id)
                .or_insert_with(|| leaf_names(cx, id, &found.standard_macros));
        }
        found
    }
}

// A qualified invocation's span starts at the path, while the source scan
// recognizes the identifier immediately before the bang.
fn macro_name_position(cx: &LateContext<'_>, span: Span) -> Option<BytePos> {
    cx.sess()
        .source_map()
        .span_to_source(span, |source, start, end| {
            Ok(source.get(start..end).and_then(|source| {
                let mut position = span.lo();
                let mut name = None;
                for token in tokenize(source, FrontmatterAllowed::No) {
                    match token.kind {
                        TokenKind::Ident | TokenKind::RawIdent => name = Some(position),
                        TokenKind::Colon => name = None,
                        TokenKind::Bang => return name,
                        TokenKind::Whitespace
                        | TokenKind::LineComment { .. }
                        | TokenKind::BlockComment { .. } => {}
                        _ => return None,
                    }
                    position = position + BytePos(token.len);
                }
                None
            }))
        })
        .ok()?
}

pub(super) struct Scope {
    pub names: HashMap<String, HashSet<BytePos>>,
    pub generated_names: HashSet<String>,
    pub gates: HashMap<BytePos, Vec<String>>,
    pub collisions: HashSet<String>,
}

fn function_gates(tokens: &[&str], index: usize) -> Vec<String> {
    let mut cursor = index;
    while cursor > 0 && matches!(tokens[cursor - 1], "pub" | "const" | "async" | "unsafe") {
        cursor -= 1;
    }
    let mut gates = Vec::new();
    while cursor > 0 && tokens[cursor - 1] == "]" {
        let end = cursor;
        let mut depth = 0;
        while cursor > 0 {
            cursor -= 1;
            match tokens[cursor] {
                "]" => depth += 1,
                "[" => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => (),
            }
        }
        if cursor == 0 || tokens[cursor - 1] != "#" {
            break;
        }
        cursor -= 1;
        if matches!(&tokens[cursor..end], ["#", "[", "cfg", "(", ..]) {
            gates.push(tokens[cursor..end].concat());
        }
    }
    gates
}

// A local declarative macro can name a helper that its input never spells.
// Account for every identifier in its definition, including inactive arms.
// Exclude macros that construct identifiers, invoke more macros, or generate
// items or outer attributes with arbitrary expansion semantics. Standard macro
// calls are accepted when rustc resolves their written position.
fn leaf_names(
    cx: &LateContext<'_>,
    id: LocalDefId,
    standard: &HashSet<(BytePos, String)>,
) -> Option<HashSet<String>> {
    let rustc_hir::Node::Item(item) = cx.tcx.hir_node_by_def_id(id) else {
        return None;
    };
    let rustc_hir::ItemKind::Macro(_, mac, _) = item.kind else {
        return None;
    };
    let source = cx
        .sess()
        .source_map()
        .span_to_snippet(mac.body.dspan.entire())
        .ok()?;
    let mut offset = 0;
    let mut tokens = Vec::new();
    let mut offsets = Vec::new();
    let mut names = HashSet::new();
    for token in tokenize(&source, FrontmatterAllowed::No) {
        let start = offset;
        offset += token.len as usize;
        if matches!(
            token.kind,
            TokenKind::Whitespace
                | TokenKind::LineComment { .. }
                | TokenKind::BlockComment { .. }
        ) {
            continue;
        }
        let text = source.get(start..offset)?;
        if matches!(token.kind, TokenKind::Ident | TokenKind::RawIdent) {
            names.insert(text.trim_start_matches("r#").to_owned());
        }
        tokens.push(text);
        offsets.push(mac.body.dspan.open.lo() + BytePos(u32::try_from(start).ok()?));
    }
    for (i, word) in tokens.iter().enumerate() {
        let tail = &tokens[i..];
        if matches!(tail, ["!", "(" | "{" | "[", ..])
            && !i.checked_sub(1).is_some_and(|previous| {
                standard.contains(&(offsets[previous], tokens[previous].to_owned()))
            })
            || matches!(tail, ["$", "{", ..] | ["mod", _, ";", ..])
            || (*word == "#"
                && !matches!(
                    tail,
                    [
                        "#",
                        "[",
                        "cfg" | "allow" | "deny" | "forbid" | "expect" | "warn",
                        "(",
                        ..
                    ]
                ))
        {
            return None;
        }
    }
    Some(names)
}

// Keep a closed lexical scope. A submodule in another file or code from an
// include or an opaque macro can introduce a call without spelling the name
// in the parent source. Account for every token in one level of external
// children. An inactive caller still contributes its written identifier.
impl Scope {
    pub fn read(cx: &LateContext<'_>, span: Span, refs: &References) -> Option<Self> {
        Self::read_module(cx, span, refs, false)
    }

    fn read_module(
        cx: &LateContext<'_>,
        span: Span,
        refs: &References,
        child: bool,
    ) -> Option<Self> {
        use rustc_lint::LintContext;
        let source = cx.sess().source_map().span_to_snippet(span).ok()?;
        let mut names = HashMap::<String, HashSet<BytePos>>::new();
        let mut tokens = Vec::new();
        let mut offset = 0;
        let mut delimiters = Vec::new();
        let mut after_bang = false;
        let mut input_names = HashSet::new();
        for token in tokenize(&source, FrontmatterAllowed::No) {
            let start = offset;
            offset += token.len as usize;
            if matches!(
                token.kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
            ) {
                continue;
            }
            let text = source.get(start..offset)?;
            let position = span.lo() + BytePos(u32::try_from(start).ok()?);
            match token.kind {
                TokenKind::OpenParen | TokenKind::OpenBrace | TokenKind::OpenBracket => {
                    delimiters.push(after_bang || delimiters.last().copied().unwrap_or(false));
                }
                TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket => {
                    delimiters.pop();
                }
                _ => {}
            }
            after_bang = token.kind == TokenKind::Bang;
            if matches!(token.kind, TokenKind::Ident | TokenKind::RawIdent) {
                if delimiters.last() == Some(&true) {
                    input_names.insert(text.trim_start_matches("r#").to_owned());
                }
                names
                    .entry(text.trim_start_matches("r#").to_owned())
                    .or_default()
                    .insert(position);
            }
            tokens.push((text, position));
        }
        let text: Vec<_> = tokens.iter().map(|x| x.0).collect();
        let mut generated_names = input_names;
        let mut visited = HashSet::new();
        let mut gates = HashMap::new();
        let mut structures = HashSet::new();
        let mut collisions = HashSet::new();
        for (i, &(word, position)) in tokens.iter().enumerate() {
            let tail = &text[i..];
            if word == "fn"
                && let Some((name, ident)) = tokens.get(i + 1)
            {
                gates.insert(*ident, function_gates(&text, i));
                if let Some(structure) = super::fixes::argument_type(name)
                    && !structures.insert(structure.clone())
                {
                    collisions.insert(structure);
                }
            }
            if matches!(tail, ["mod", _, ";", ..]) {
                if child {
                    return None;
                }
                let inner = refs.module_sources.get(&tokens[i + 1].1)?;
                let child_scope = Self::read_module(cx, *inner, refs, true)?;
                generated_names.extend(child_scope.names.into_keys());
                generated_names.extend(child_scope.generated_names);
            }
            if matches!(tail, ["include" | "macro_rules", "!", ..]) {
                return None;
            }
            if matches!(tail, [_, "!", "(" | "{" | "[", ..])
                && word
                    .chars()
                    .next()
                    .is_some_and(|c| c == '_' || c.is_alphabetic())
                && !matches!(word, "if" | "while" | "match" | "return")
                && !refs.standard_macros.contains(&(position, word.to_owned()))
            {
                let id = refs.leaf_calls.get(&(position, word.to_owned()))?;
                if visited.insert(id) {
                    generated_names.extend(refs.leaf_names.get(id)?.as_ref()?.iter().cloned());
                }
            }
            if let ["#", rest @ ..] = tail {
                let rest = rest.strip_prefix(&["!"]).unwrap_or(rest);
                if let ["[", attr, args @ ..] = rest {
                    match *attr {
                        "cfg" => {}
                        "cfg_attr" if lint_cfg(args) => {}
                        "arcane" | "rite" if refs.archmage.contains(&position) => {}
                        "path" if matches!(args, ["=", _, "]", "mod", _, ";", ..]) => {}
                        "derive"
                            if args.first() == Some(&"(")
                                && tokens[i + 4..]
                                    .iter()
                                    .take_while(|(word, _)| *word != ")")
                                    .all(|(word, pos)| {
                                        *word == "," || refs.derives.contains(pos)
                                    }) => {}
                        "inline" | "must_use" | "cold" | "test" | "should_panic" | "ignore"
                        | "doc" | "allow" | "warn" | "deny" | "forbid" | "expect" | "repr" => {}
                        _ => return None,
                    }
                }
            }
        }
        Some(Self {
            names,
            generated_names,
            gates,
            collisions,
        })
    }
}

fn lint_cfg(tokens: &[&str]) -> bool {
    let Some(condition) = tokens.strip_prefix(&["("]) else {
        return false;
    };
    let mut nesting = 0usize;
    let Some(split) = condition.iter().position(|t| match *t {
        "(" => {
            nesting += 1;
            false
        }
        ")" => {
            nesting = nesting.saturating_sub(1);
            false
        }
        "," => nesting == 0,
        _ => false,
    }) else {
        return false;
    };
    let [
        "allow" | "warn" | "expect" | "deny" | "forbid",
        "(",
        rest @ ..,
    ] = &condition[split + 1..]
    else {
        return false;
    };
    let mut depth = 0usize;
    for (i, token) in rest.iter().enumerate() {
        match *token {
            "(" => depth += 1,
            ")" if depth == 0 => return rest[i + 1..].starts_with(&[")", "]"]),
            ")" => depth -= 1,
            _ => {}
        }
    }
    false
}
