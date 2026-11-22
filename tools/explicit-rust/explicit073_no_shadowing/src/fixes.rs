//! Rename a binding by its HIR ID and edit only references to that binding.

use std::collections::{HashMap, HashSet};

use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Node, Pat, PatKind, QPath};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::{hir::nested_filter::OnlyBodies, ty::TyCtxt};
use rustc_span::{BytePos, Span, SyntaxContext, hygiene::ExpnKind, symbol::Ident};

#[path = "../../shared/authored_function.rs"]
mod source_function;

// These macros use their expression arguments as values. Keep `assert!` and
// `dbg!` out: they can expose the spelling of an expression at runtime.
const EXPRESSION_MACROS: &[&str] = &[
    "alloc::macros::vec",
    "core::macros::assert_eq",
    "core::macros::assert_ne",
    "core::macros::debug_assert_eq",
    "core::macros::debug_assert_ne",
    "core::macros::matches",
];

// These standard macros pass a format string and its arguments to the
// compiler's format_args expansion. Appending a named argument leaves the
// literal, its formatting flags and all prior argument positions intact.
const FORMAT_MACROS: &[&str] = &[
    "alloc::macros::format",
    "core::macros::builtin::format_args",
    "core::macros::write",
    "core::macros::writeln",
    "std::macros::print",
    "std::macros::println",
    "std::macros::eprint",
    "std::macros::eprintln",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MacroUse {
    Expressions,
    Format,
    Unknown,
}

pub(super) struct Renames<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    function: Span,
    occupied: HashSet<String>,
    bindings: HashMap<HirId, Option<Vec<Occurrence>>>,
    macro_arguments: Vec<MacroArguments>,
    expression_macros: &'a [String],
    body_macros: &'a [String],
    seen_contexts: HashSet<SyntaxContext>,
    reviewed_macros: HashMap<DefId, MacroUse>,
    reviewed_calls: HashMap<BytePos, (BytePos, MacroUse)>,
    resolutions: HashMap<(BytePos, BytePos), Option<HirId>>,
}

struct Occurrence {
    span: Span,
    prefix: String,
    expanded: bool,
}

struct MacroArguments {
    open: BytePos,
    close: BytePos,
    tokens: Vec<ArgumentToken>,
    trailing_comma: bool,
}

struct ArgumentToken {
    span: (BytePos, BytePos),
    spelling: String,
    literal: bool,
    direct: bool,
}

fn macro_path(tcx: TyCtxt<'_>, id: DefId) -> String {
    let path = tcx.def_path(id).to_string_no_crate_verbose();
    format!("{}{path}", tcx.crate_name(id.krate))
}

fn written_context(tcx: TyCtxt<'_>, span: Span, body_macros: &[String]) -> bool {
    if !span.from_expansion() {
        return true;
    }
    if body_macros.is_empty() {
        return false;
    }
    let mut found = false;
    for expansion in span.macro_backtrace() {
        if matches!(expansion.kind, ExpnKind::Macro(..)) {
            let Some(id) = expansion.macro_def_id else {
                return false;
            };
            if !body_macros.contains(&macro_path(tcx, id)) {
                return false;
            }
            found = true;
        }
    }
    found
}

impl<'a, 'tcx> Renames<'a, 'tcx> {
    pub(super) fn new(
        cx: &LateContext<'tcx>,
        body: &Body<'tcx>,
        function: Span,
        hits: &[(HirId, Ident)],
        expression_macros: &'a [String],
        body_macros: &'a [String],
    ) -> Option<Self> {
        // A function declared inside a block can also see that block's items
        // and imports, which are outside both its source and its module scope.
        if !written_context(cx.tcx, function, body_macros)
            || cx.tcx.hir_parent_iter(body.value.hir_id).any(|(_, node)| {
                matches!(node, Node::Block(block)
                        if !function.from_expansion() || !block.span.from_expansion())
            })
        {
            return None;
        }

        let function = if function.from_expansion() {
            // The generated braces can also carry the attribute's span.
            // Recover the authored item, including tokens that cfg removed.
            let required: Vec<_> = hits.iter().map(|(_, ident)| ident.span).collect();
            source_function::authored_function(cx, function, &required)?
        } else {
            function
        };
        let source = cx.sess().source_map().span_to_snippet(function).ok()?;
        let (mut occupied, macro_arguments) = source_names(&source, function.lo())?;
        let module = cx.tcx.parent_module(body.value.hir_id).to_local_def_id();
        occupied.extend(
            cx.tcx
                .module_children_local(module)
                .iter()
                .map(|child| child.ident.as_str().to_owned()),
        );
        if let Some(ambiguous) = cx.tcx.resolutions(()).ambig_module_children.get(&module) {
            occupied.extend(
                ambiguous
                    .iter()
                    .map(|child| child.main.ident.as_str().to_owned()),
            );
        }
        let bindings = hits.iter().map(|(id, _)| (*id, Some(Vec::new()))).collect();
        let mut renames = Self {
            tcx: cx.tcx,
            function,
            occupied,
            bindings,
            macro_arguments,
            expression_macros,
            body_macros,
            seen_contexts: HashSet::new(),
            reviewed_macros: HashMap::new(),
            reviewed_calls: HashMap::new(),
            resolutions: HashMap::new(),
        };
        renames.visit_body(body);
        Some(renames)
    }

    pub(super) fn take(
        &mut self,
        id: HirId,
        ident: Ident,
    ) -> Option<(String, Vec<(Span, String)>)> {
        let mut occurrences = self.bindings.remove(&id)??;
        let scope_end = self
            .tcx
            .hir_parent_iter(id)
            .find_map(|(_, node)| match node {
                Node::Block(block) if self.function.contains(block.span) => Some(block.span.hi()),
                Node::Arm(arm) if self.function.contains(arm.span) => Some(arm.span.hi()),
                _ => None,
            })
            .unwrap_or(self.function.hi());
        let stem = ident.as_str();
        let mut aliases = Vec::new();
        for arguments in &self.macro_arguments {
            if arguments.close <= ident.span.hi() || arguments.open >= scope_end {
                continue;
            }
            let usage = self
                .reviewed_calls
                .get(&arguments.close)
                .filter(|(begin, _)| *begin < arguments.open)
                .map_or(MacroUse::Unknown, |(_, kind)| *kind);
            let mut captured = false;
            for token in &arguments.tokens {
                if token.span.0 < ident.span.hi() || token.span.1 > scope_end {
                    continue;
                }
                if token.literal && token.spelling.contains(stem) {
                    if usage != MacroUse::Format {
                        return None;
                    }
                    if token.direct {
                        // rustc resolves one local path inside a direct format
                        // literal, even for repeated placeholders and counts.
                        // Supplying that capture by name covers them all.
                        for occurrence in &occurrences {
                            let pos = (occurrence.span.lo(), occurrence.span.hi());
                            if token.span.0 < pos.0 && pos.1 < token.span.1 {
                                if self.resolutions.get(&pos) != Some(&Some(id)) {
                                    return None;
                                }
                                captured = true;
                            }
                        }
                        occurrences.retain(|occurrence| {
                            !(token.span.0 < occurrence.span.lo()
                                && occurrence.span.hi() < token.span.1)
                        });
                    }
                }
                if !token.literal
                    && token.spelling == stem
                    && (usage == MacroUse::Unknown
                        || !self
                            .resolutions
                            .get(&token.span)
                            .is_some_and(Option::is_some))
                {
                    return None;
                }
            }
            if captured {
                let close = arguments.close - BytePos(1);
                let span = self.function.with_lo(close).with_hi(close);
                let comma = if arguments.trailing_comma { " " } else { ", " };
                let spelling = self
                    .tcx
                    .sess
                    .source_map()
                    .span_to_snippet(ident.span)
                    .ok()?;
                aliases.push(Occurrence {
                    span,
                    prefix: format!("{comma}{spelling} = "),
                    expanded: false,
                });
            }
        }
        if occurrences.iter().any(|occurrence| {
            let pos = (occurrence.span.lo(), occurrence.span.hi());
            // A shorthand capture left inside a literal can represent several
            // placeholders. Editing just its first identifier is incomplete.
            self.macro_arguments.iter().any(|arguments| {
                arguments.open < pos.0
                    && pos.1 < arguments.close
                    && arguments.tokens.iter().any(|token| {
                        token.literal && token.span.0 < pos.0 && pos.1 < token.span.1
                    })
            }) || occurrence.expanded
                && !self.macro_arguments.iter().any(|arguments| {
                    arguments.open < occurrence.span.lo()
                        && occurrence.span.hi() < arguments.close
                        && self
                            .reviewed_calls
                            .get(&arguments.close)
                            .is_some_and(|(begin, _)| *begin < arguments.open)
                        && arguments.tokens.iter().any(|token| {
                            !token.literal
                                && token.span == (occurrence.span.lo(), occurrence.span.hi())
                                && self.resolutions.get(&token.span) == Some(&Some(id))
                        })
                })
        }) {
            return None;
        }
        occurrences.extend(aliases);
        if !occurrences
            .iter()
            .any(|occurrence| occurrence.span.contains(ident.span))
        {
            return None;
        }

        occurrences.sort_by_key(|occurrence| (occurrence.span.lo(), occurrence.span.hi()));
        occurrences.dedup_by(|a, b| a.span == b.span && a.prefix == b.prefix);
        if occurrences
            .windows(2)
            .any(|pair| pair[0].span.hi() > pair[1].span.lo())
        {
            return None;
        }

        let name = (2..)
            .map(|suffix| format!("{stem}_{suffix}"))
            .find(|name| self.occupied.insert(name.clone()))?;
        let edits = occurrences
            .into_iter()
            .map(|occurrence| (occurrence.span, format!("{}{name}", occurrence.prefix)))
            .collect();
        Some((name, edits))
    }

    fn review_expansion(&mut self, span: Span) {
        if !span.from_expansion() || !self.seen_contexts.insert(span.ctxt()) {
            return;
        }
        for expansion in span.macro_backtrace() {
            let Some(id) = expansion.macro_def_id else {
                continue;
            };
            let reviewed = self.reviewed_macros.entry(id).or_insert_with(|| {
                let path = macro_path(self.tcx, id);
                if FORMAT_MACROS.contains(&path.as_str()) {
                    MacroUse::Format
                } else if EXPRESSION_MACROS.contains(&path.as_str())
                    || self.expression_macros.contains(&path)
                {
                    MacroUse::Expressions
                } else {
                    MacroUse::Unknown
                }
            });
            if *reviewed != MacroUse::Unknown
                && self.function.contains(expansion.call_site)
                && written_context(self.tcx, expansion.call_site, self.body_macros)
            {
                self.reviewed_calls.insert(
                    expansion.call_site.hi(),
                    (expansion.call_site.lo(), *reviewed),
                );
            }
        }
    }

    fn record(&mut self, id: HirId, ident: Ident, span: Span, shorthand: bool, node_span: Span) {
        let Some(entry) = self.bindings.get_mut(&id) else {
            return;
        };
        let Some(occurrences) = entry else {
            return;
        };
        let source_map = self.tcx.sess.source_map();
        let occurrence = (|| {
            let expanded = [node_span, ident.span, span]
                .into_iter()
                .any(|span| !written_context(self.tcx, span, self.body_macros));
            if (expanded && shorthand)
                || !self.function.contains(span)
                || !span.contains(ident.span)
            {
                return None;
            }

            let spelling = source_map.span_to_snippet(ident.span).ok()?;
            if spelling.strip_prefix("r#").unwrap_or(&spelling) != ident.as_str() {
                return None;
            }

            let prefix = if shorthand {
                let modifiers = source_map
                    .span_to_snippet(span.with_hi(ident.span.lo()))
                    .ok()?;
                format!("{spelling}: {modifiers}")
            } else {
                String::new()
            };
            Some(Occurrence {
                span: span.with_ctxt(SyntaxContext::root()),
                prefix,
                expanded,
            })
        })();

        match occurrence {
            Some(occurrence) => occurrences.push(occurrence),
            None => *entry = None,
        }
    }

    fn resolve_local_token(&mut self, span: Span, id: HirId) {
        self.resolutions
            .entry((span.lo(), span.hi()))
            .and_modify(|previous| {
                if *previous != Some(id) {
                    *previous = None;
                }
            })
            .or_insert(Some(id));
    }
}

impl<'tcx> Visitor<'tcx> for Renames<'_, 'tcx> {
    type MaybeTyCtxt = TyCtxt<'tcx>;
    type NestedFilter = OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_ident(&mut self, ident: Ident) {
        // Expanded code and Unicode normalization can introduce other spellings.
        self.occupied.insert(ident.as_str().to_owned());
    }

    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        if let PatKind::Binding(_, id, ident, _) = pat.kind {
            let shorthand = matches!(
                self.tcx.parent_hir_node(pat.hir_id),
                Node::PatField(field) if field.is_shorthand
            );
            let span = if shorthand { pat.span } else { ident.span };
            self.resolve_local_token(ident.span, id);
            self.record(id, ident, span, shorthand, pat.span);
        }
        intravisit::walk_pat(self, pat);
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        self.review_expansion(expr.span);
        if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind
            && let Res::Local(id) = path.res
            && let [segment] = path.segments
        {
            let shorthand = matches!(
                self.tcx.parent_hir_node(expr.hir_id),
                Node::ExprField(field) if field.is_shorthand
            );
            let span = segment.ident.span;
            self.resolve_local_token(span, id);
            self.record(id, segment.ident, span, shorthand, expr.span);
        }
        intravisit::walk_expr(self, expr);
    }
}

// Conditional attributes can change which binding an unchanged name refers to
// in another build. Scoped imports can introduce names through globs. Macro
// arguments may use a spelling as both code and data.
fn source_names(source: &str, origin: BytePos) -> Option<(HashSet<String>, Vec<MacroArguments>)> {
    let mut names = HashSet::new();
    let mut macros = Vec::new();
    let mut offset = 0;
    let mut depth = 0;
    let mut in_import = false;
    let mut macro_groups: Vec<(usize, MacroArguments)> = Vec::new();
    let mut previous = TokenKind::Whitespace;
    let mut before_previous = TokenKind::Whitespace;

    for token in tokenize(source, FrontmatterAllowed::No) {
        let start = offset;
        offset += token.len as usize;
        let from = origin + BytePos(u32::try_from(start).ok()?);
        let to = origin + BytePos(u32::try_from(offset).ok()?);
        let spelling = &source[start..offset];
        match token.kind {
            TokenKind::Whitespace
            | TokenKind::LineComment { .. }
            | TokenKind::BlockComment { .. } => continue,
            TokenKind::Ident | TokenKind::RawIdent | TokenKind::UnknownPrefix => {
                let name = spelling.strip_prefix("r#").unwrap_or(spelling);
                if matches!(name, "cfg_attr" | "macro_rules" | "macro")
                    || (name == "cfg" && previous == TokenKind::OpenBracket)
                {
                    return None;
                }
                if name == "use" && token.kind == TokenKind::Ident {
                    if !macro_groups.is_empty() {
                        return None;
                    }
                    in_import = true;
                }
                names.insert(name.to_owned());
                for (group_depth, arguments) in &mut macro_groups {
                    arguments.tokens.push(ArgumentToken {
                        span: (from, to),
                        spelling: name.to_owned(),
                        literal: false,
                        direct: *group_depth == depth,
                    });
                }
            }
            TokenKind::Literal { .. }
            | TokenKind::Lifetime { .. }
            | TokenKind::RawLifetime
            | TokenKind::UnknownPrefixLifetime => {
                for (group_depth, arguments) in &mut macro_groups {
                    arguments.tokens.push(ArgumentToken {
                        span: (from, to),
                        spelling: spelling.to_owned(),
                        literal: true,
                        direct: *group_depth == depth,
                    });
                }
            }
            TokenKind::OpenParen | TokenKind::OpenBrace | TokenKind::OpenBracket => {
                depth += 1;
                if previous == TokenKind::Bang
                    && matches!(before_previous, TokenKind::Ident | TokenKind::RawIdent)
                {
                    macro_groups.push((
                        depth,
                        MacroArguments {
                            open: from,
                            close: to,
                            tokens: Vec::new(),
                            trailing_comma: false,
                        },
                    ));
                }
            }
            TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket => {
                if macro_groups
                    .last()
                    .is_some_and(|(group_depth, _)| *group_depth == depth)
                {
                    let (_, mut arguments) = macro_groups.pop()?;
                    arguments.close = to;
                    arguments.trailing_comma = previous == TokenKind::Comma;
                    macros.push(arguments);
                }
                depth -= 1;
            }
            TokenKind::Star if in_import => return None,
            TokenKind::Semi => in_import = false,
            _ => {}
        }
        before_previous = previous;
        previous = token.kind;
    }

    Some((names, macros))
}
