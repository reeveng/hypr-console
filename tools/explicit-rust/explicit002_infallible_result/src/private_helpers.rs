//! Migrate a private helper together with every caller inside its closed module.
use clippy_utils::visitors::for_each_expr_without_closures;
use rustc_hir::{
    BindingMode, Body, ExprKind, FnDecl, FnRetTy, HirId, ItemKind, MatchSource, Node, PatKind,
    QPath, StmtKind,
    def::{DefKind, Res},
    def_id::{LocalDefId, LocalModDefId},
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span, Symbol, sym};
use std::{
    collections::{HashMap, HashSet},
    ops::ControlFlow,
};

#[derive(Default)]
pub(super) struct Helpers {
    error_type: Option<Option<&'static str>>,
    uses: Option<HashMap<LocalDefId, Vec<(Span, HirId)>>>,
    scopes: HashMap<LocalModDefId, Option<Scope>>,
}

fn error_type(cx: &LateContext<'_>, core_infallible: bool) -> Option<&'static str> {
    if has_never(cx) {
        return Some("::console_core_never::Never");
    }
    if !core_infallible || cx.sess().opts.externs.get("core").is_some() {
        return None;
    }
    let tcx = cx.tcx;
    let core = tcx.get_diagnostic_item(sym::Result)?.krate;
    // The absolute path names the extern prelude. A crate-root alias can
    // change which crate that path resolves to.
    if tcx.crate_name(core) != sym::core
        || tcx.hir_attrs(rustc_hir::CRATE_HIR_ID).iter().any(|attr| {
            matches!(attr, rustc_hir::Attribute::Parsed(rustc_hir::attrs::AttributeKind::NoCore))
        })
        || tcx.hir_root_module().item_ids.iter().any(|&item| {
            matches!(tcx.hir_item(item).kind, ItemKind::ExternCrate(Some(original), ident)
                if ident.name == sym::core && original != sym::core)
        })
    {
        return None;
    }
    Some("::core::convert::Infallible")
}

fn has_never(cx: &LateContext<'_>) -> bool {
    let name = "console_core_never";
    let defines_never = |id: rustc_hir::def_id::DefId| {
        cx.tcx.crate_name(id.krate).as_str() == name && cx.tcx.module_children(id).iter().any(|child| {
            child.ident.as_str() == "Never" && child.vis.is_public()
                && matches!(child.res, Res::Def(DefKind::Enum, ty) if cx.tcx.adt_def(ty).variants().is_empty() && !cx.tcx.adt_def(ty).is_variant_list_non_exhaustive())
        })
    };
    (cx.sess().opts.externs.get(name).is_some_and(|entry| entry.add_prelude)
        || cx.tcx.module_children_local(rustc_hir::def_id::CRATE_DEF_ID).iter().any(|child|
            child.ident.as_str() == name && matches!(child.res, Res::Def(DefKind::Mod, id) if !id.is_local() && defines_never(id))))
        && cx.tcx.crates(()).iter().any(|id| defines_never(id.as_def_id()))
}

fn uses(cx: &LateContext<'_>) -> HashMap<LocalDefId, Vec<(Span, HirId)>> {
    let mut uses = HashMap::<LocalDefId, Vec<(Span, HirId)>>::new();
    for owner in cx.tcx.hir_body_owners() {
        for_each_expr_without_closures(cx.tcx.hir_body_owned_by(owner), |expr| {
            if let ExprKind::Path(QPath::Resolved(_, path)) = expr.kind
                && let Res::Def(DefKind::Fn, id) = path.res
                && let Some(id) = id.as_local()
                && let Some(name) = path.segments.last()
            {
                uses.entry(id)
                    .or_default()
                    .push((name.ident.span, expr.hir_id));
            }
            ControlFlow::<(), ()>::Continue(())
        });
    }
    uses
}

// HIR alone cannot see a caller compiled under another configuration or a use
// in an opaque macro. Only accept modules with a single source and account for
// every identifier spelling, including uses hidden by the test harness.
#[derive(Default)]
struct Scope {
    names: HashMap<Symbol, HashSet<BytePos>>,
    macro_inputs: HashSet<Symbol>,
}

fn trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    )
}

fn scope_names(cx: &LateContext<'_>, span: Span) -> Option<Scope> {
    if span.from_expansion() {
        return None;
    }
    let contents = cx.sess().source_map().span_to_snippet(span).ok()?;
    let mut scope = Scope::default();
    let mut pos = 0;
    let mut previous = None;
    let mut delimiters = Vec::new();
    for token in tokenize(&contents, FrontmatterAllowed::No) {
        let start = pos;
        pos += token.len as usize;
        if trivia(token.kind) {
            continue;
        }
        match token.kind {
            TokenKind::OpenParen | TokenKind::OpenBrace | TokenKind::OpenBracket => {
                delimiters.push(previous == Some(TokenKind::Bang))
            }
            TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket => {
                delimiters.pop();
            }
            TokenKind::Ident | TokenKind::RawIdent => {
                let text = &contents[start..pos];
                let name = text.strip_prefix("r#").unwrap_or(text);
                let remainder = &contents[pos..];
                match name {
                    // Every declaration and caller must be visible in this module's source.
                    "include" | "cfg_attr" | "macro_rules" | "macro" | "global_asm" => {
                        return None;
                    }
                    "cfg"
                        if !(cx.sess().opts.test
                            && remainder.trim_start().starts_with("(test)]")) =>
                    {
                        return None;
                    }
                    "mod" => {
                        let mut after = tokenize(remainder, FrontmatterAllowed::No)
                            .filter(|t| !trivia(t.kind));
                        if !after.next().is_some_and(|t| {
                            matches!(t.kind, TokenKind::Ident | TokenKind::RawIdent)
                        }) || !after.next().is_some_and(|t| t.kind == TokenKind::OpenBrace)
                        {
                            return None;
                        }
                    }
                    _ => {}
                }
                let name = Symbol::intern(name);
                if delimiters.contains(&true) {
                    scope.macro_inputs.insert(name);
                }
                scope
                    .names
                    .entry(name)
                    .or_default()
                    .insert(span.lo() + BytePos(u32::try_from(start).ok()?));
            }
            _ => {}
        }
        previous = Some(token.kind);
    }
    Some(scope)
}

fn edit_caller(
    cx: &LateContext<'_>,
    use_id: HirId,
    unit: bool,
    edits: &mut Vec<(Span, String)>,
) -> Option<()> {
    let Node::Expr(call) = cx.tcx.parent_hir_node(use_id) else {
        return None;
    };
    if call.span.from_expansion()
        || !matches!(call.kind, ExprKind::Call(callee, _) if callee.hir_id == use_id)
    {
        // Taking the helper as a value fixes its function-pointer signature.
        return None;
    }
    let ok = "::core::result::Result::Ok";
    match cx.tcx.parent_hir_node(call.hir_id) {
        Node::LetStmt(local)
            if local.els.is_none()
                && local.ty.is_none()
                && !local.span.from_expansion()
                && !local.pat.span.from_expansion()
                && local.span.contains(call.span)
                && matches!(
                    local.pat.kind,
                    PatKind::Binding(BindingMode::NONE | BindingMode::MUT, _, _, None)
                        | PatKind::Wild
                ) =>
        {
            let pattern = cx
                .sess()
                .source_map()
                .span_to_snippet(local.pat.span)
                .ok()?;
            edits.push((local.pat.span, format!("{ok}({pattern})")));
        }
        Node::Stmt(stmt)
            if !stmt.span.from_expansion()
                && stmt.span.lo() == call.span.lo()
                && stmt.span.contains(call.span)
                && matches!(stmt.kind, StmtKind::Semi(_)) =>
        {
            let pattern = if unit { "()" } else { "_" };
            edits.push((call.span.shrink_to_lo(), format!("let {ok}({pattern}) = ")));
        }
        Node::Expr(parent) if !parent.span.from_expansion() => {
            let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = parent.kind else {
                return None;
            };
            if scrutinee.hir_id != call.hir_id || arms.is_empty() {
                return None;
            }
            // Preserve the scrutinee's temporary scope and the order, guards,
            // and bindings of its original arms.
            for arm in arms {
                let pattern = arm.pat.span;
                if pattern.from_expansion() || !parent.span.contains(pattern) {
                    return None;
                }
                edits.push((pattern.shrink_to_lo(), format!("{ok}(")));
                edits.push((pattern.shrink_to_hi(), ")".into()));
            }
        }
        _ => return None,
    }
    Some(())
}

impl Helpers {
    pub(super) fn edits<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        id: LocalDefId,
        decl: &FnDecl<'_>,
        body: &'tcx Body<'tcx>,
        core_infallible: bool,
    ) -> Option<Vec<(Span, String)>> {
        if !cx.sess().edition().at_least_rust_2018() {
            return None;
        }
        let hir_id = cx.tcx.local_def_id_to_hir_id(id);
        let Node::Item(item) = cx.tcx.hir_node(hir_id) else {
            return None;
        };
        let ItemKind::Fn { sig, ident, .. } = item.kind else {
            return None;
        };
        if !item.vis_span.is_empty()
            || item.span.from_expansion()
            || ident.span.from_expansion()
            || sig.header.is_const()
            || sig.header.is_async()
            || !sig.header.is_safe()
            || item.eii
            || !matches!(cx.tcx.def_kind(cx.tcx.parent(id.to_def_id())), DefKind::Mod)
        {
            return None;
        }
        let error = (*self.error_type.get_or_insert_with(|| error_type(cx, core_infallible)))?;
        let map = cx.sess().source_map();
        let mut edits = Vec::new();
        for attr in cx.tcx.hir_attrs(hir_id) {
            use rustc_hir::{Attribute, attrs::AttributeKind};
            match attr {
                Attribute::Parsed(
                    AttributeKind::DocComment { .. }
                    | AttributeKind::Inline(..)
                    | AttributeKind::MustUse {
                        reason: Some(_), ..
                    },
                ) => {}
                Attribute::Parsed(AttributeKind::MustUse { span, reason: None })
                    if !span.from_expansion() && span.hi() <= item.span.lo() =>
                {
                    let gap = span.shrink_to_hi().with_hi(item.span.lo());
                    if !map.span_to_snippet(gap).ok()?.trim().is_empty() {
                        return None;
                    }
                    edits.push((span.with_hi(item.span.lo()), String::new()));
                }
                _ => return None,
            }
        }
        let module = cx.tcx.parent_module_from_def_id(id);
        let span = cx.tcx.hir_get_module(module).0.spans.inner_span;
        let scope = self
            .scopes
            .entry(module)
            .or_insert_with(|| scope_names(cx, span))
            .as_ref()?;
        if scope.macro_inputs.contains(&ident.name) {
            return None;
        }
        let sites = scope.names.get(&ident.name)?;
        let uses = self.uses.get_or_insert_with(|| uses(cx)).get(&id)?;
        if uses.is_empty() || !span.contains(item.span) {
            return None;
        }
        let unit = cx
            .tcx
            .fn_sig(id)
            .skip_binder()
            .output()
            .skip_binder()
            .is_unit();
        let mut covered = HashSet::from([ident.span.lo()]);
        for &(name, site) in uses {
            if name.from_expansion() || !span.contains(name) || !covered.insert(name.lo()) {
                return None;
            }
            edit_caller(cx, site, unit, &mut edits)?;
        }
        if &covered != sites {
            return None;
        }

        let ExprKind::Block(block, None) = body.value.kind else {
            return None;
        };
        if block.span.from_expansion() || !item.span.contains(block.span) {
            return None;
        }
        let source = map.span_to_snippet(block.span).ok()?;
        if source.contains('#')
            || for_each_expr_without_closures(body, |e| {
                if matches!(
                    e.kind,
                    ExprKind::Ret(_)
                        | ExprKind::Match(_, _, MatchSource::TryDesugar(_))
                ) {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
            .is_some()
        {
            return None;
        }
        let ok = "::core::result::Result::Ok";
        match decl.output {
            FnRetTy::Return(ty) if !ty.span.from_expansion() && item.span.contains(ty.span) => {
                let original = map.span_to_snippet(ty.span).ok()?;
                if original.contains("impl ") || original.contains('#') {
                    return None;
                }
                edits.push((
                    ty.span,
                    format!("::core::result::Result<{original}, {error}>"),
                ));
            }
            FnRetTy::DefaultReturn(sp) if sp.is_empty() => {
                edits.push((sp, format!(" -> ::core::result::Result<(), {error}>")));
            }
            _ => return None,
        }
        if let Some(tail) = block.expr {
            if tail.span.from_expansion() || !block.span.contains(tail.span) {
                return None;
            }
            edits.push((tail.span.shrink_to_lo(), format!("{ok}(")));
            edits.push((tail.span.shrink_to_hi(), ")".into()));
        } else if unit {
            let close = block.span.with_lo(block.span.hi() - BytePos(1));
            if map.span_to_snippet(close).ok()? != "}" {
                return None;
            }
            let outer = clippy_utils::source::indent_of(cx, close)?;
            let indent = block
                .stmts
                .first()
                .and_then(|stmt| clippy_utils::source::indent_of(cx, stmt.span))
                .unwrap_or(outer + 2);
            edits.push((
                close.shrink_to_lo(),
                format!(
                    "{}{ok}(())\n{}",
                    " ".repeat(indent.saturating_sub(outer)),
                    " ".repeat(outer)
                ),
            ));
        } else {
            return None;
        }
        Some(edits)
    }
}
