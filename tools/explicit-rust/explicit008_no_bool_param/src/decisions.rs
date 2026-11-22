//! A configuration names the enum and every signature that shares a decision.

use clippy_utils::visitors::for_each_expr_without_closures;
use rustc_abi::ExternAbi;
use rustc_hir::{
    BindingMode, Expr, ExprKind, HirId, ImplItemImplKind, ImplItemKind, ItemKind, MatchSource,
    Node, PatKind, QPath, VariantData,
    def::{DefKind, Res},
    def_id::{DefId, LOCAL_CRATE, LocalDefId},
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Ident, Span, SyntaxContext, sym};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    ops::ControlFlow,
    path::{Component, Path, PathBuf},
};

type Edits = Vec<(Span, String)>;

#[derive(Default)]
pub(super) struct Decisions {
    families: Vec<Family>,
    edits: HashMap<BytePos, Edits>,
}

struct Family {
    module: String,
    inherent_type: Option<String>,
    enum_path: String,
    when_true: String,
    when_false: String,
    members: BTreeMap<String, String>,
    sources: BTreeSet<String>,
    target: Option<String>,
    reviewed_archmage: bool,
    reviewed_macros: BTreeSet<String>,
    forwarded_macro_arguments: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
struct Member {
    index: usize,
    binding: HirId,
    ident: Span,
    ty: Span,
    generated: bool,
}

struct Plan {
    members: HashMap<LocalDefId, Member>,
    bindings: HashSet<HirId>,
    covered: HashSet<BytePos>,
    other_names: HashSet<BytePos>,
    enum_id: LocalDefId,
    seen_branches: HashSet<BytePos>,
    edits: Edits,
}

impl Decisions {
    pub(super) fn configured() -> Self {
        let Ok(Some(value)) = dylint_linting::config_toml("explicit008_enum_families") else {
            return Self::default();
        };
        let families = value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| {
                let table = value.as_table()?;
                let field = |name| table.get(name)?.as_str().map(str::to_owned);
                let members = table
                    .get("members")?
                    .as_table()?
                    .iter()
                    .map(|(name, arg)| Some((name.clone(), arg.as_str()?.to_owned())))
                    .collect::<Option<BTreeMap<_, _>>>()?;
                let sources = table
                    .get("sources")?
                    .as_array()?
                    .iter()
                    .map(|value| value.as_str().map(str::to_owned))
                    .collect::<Option<BTreeSet<_>>>()?;
                let reviewed_macros =
                    table
                        .get("reviewed_macros")
                        .map_or(Some(BTreeSet::new()), |value| {
                            value
                                .as_array()?
                                .iter()
                                .map(|entry| entry.as_str().map(str::to_owned))
                                .collect::<Option<BTreeSet<_>>>()
                        })?;
                let forwarded_macro_arguments = table.get("forwarded_macro_arguments").map_or(
                    Some(BTreeMap::new()),
                    |value| {
                        value
                            .as_table()?
                            .iter()
                            .map(|(name, argument)| {
                                Some((name.clone(), argument.as_str()?.to_owned()))
                            })
                            .collect::<Option<BTreeMap<_, _>>>()
                    },
                )?;
                let family = Family {
                    module: field("module")?,
                    inherent_type: table
                        .get("inherent_type")
                        .map_or(Some(None), |ty| Some(Some(ty.as_str()?.to_owned())))?,
                    enum_path: field("enum")?,
                    when_true: field("when_true")?,
                    when_false: field("when_false")?,
                    members,
                    sources,
                    reviewed_macros,
                    forwarded_macro_arguments,
                    target: field("target"),
                    reviewed_archmage: table
                        .get("reviewed_archmage")
                        .and_then(|value| value.as_bool())
                        .unwrap_or_default(),
                };
                family.valid().then_some(family)
            })
            .collect();
        Self {
            families,
            edits: HashMap::new(),
        }
    }

    pub(super) fn plan(&mut self, cx: &LateContext<'_>) {
        let mut occupied = Vec::new();
        for family in &self.families {
            if let Some((anchor, edits)) = family.edits(cx)
                && edits.iter().all(|(span, _)| {
                    occupied.iter().all(|claimed: &Span| {
                        span.lo() >= claimed.hi() || claimed.lo() >= span.hi()
                    })
                })
            {
                occupied.extend(edits.iter().map(|(span, _)| *span));
                self.edits.insert(anchor, edits);
            }
        }
    }

    pub(super) fn take(&mut self, span: Span) -> Option<Edits> {
        self.edits.remove(&span.lo())
    }
}

fn identifier(text: &str) -> bool {
    let mut tokens = tokenize(text, FrontmatterAllowed::No);
    tokens
        .next()
        .is_some_and(|token| token.kind == TokenKind::Ident && token.len as usize == text.len())
        && tokens.next().is_none()
}

fn identifiers(source: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    tokenize(source, FrontmatterAllowed::No).filter_map(move |token| {
        let start = offset;
        offset += token.len as usize;
        matches!(token.kind, TokenKind::Ident | TokenKind::RawIdent)
            .then(|| (start, source[start..offset].trim_start_matches("r#")))
    })
}

fn significant_tokens(source: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    tokenize(source, FrontmatterAllowed::No).filter_map(move |token| {
        let start = offset;
        offset += token.len as usize;
        (!matches!(
            token.kind,
            TokenKind::Whitespace
                | TokenKind::LineComment { .. }
                | TokenKind::BlockComment { .. }
        ))
        .then(|| (start, &source[start..offset]))
    })
}

fn method_names(source: &str) -> Vec<(usize, &str)> {
    let tokens = significant_tokens(source).collect::<Vec<_>>();
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, &(offset, name))| {
            let previous = index.checked_sub(1).map(|prev| tokens[prev].1)?;
            let next = tokens.get(index + 1).map(|next| next.1);
            let qualified = previous == ":" && index >= 2 && tokens[index - 2].1 == ":";
            (previous == "fn"
                || (previous == "." && matches!(next, Some("(" | ":")))
                || qualified)
                .then(|| (offset, name.trim_start_matches("r#")))
        })
        .collect()
}

fn crate_path(path: &str) -> bool {
    path.starts_with("crate::") && path.split("::").all(identifier)
}

fn qualified(cx: &LateContext<'_>, id: DefId) -> String {
    format!(
        "{}{}",
        cx.tcx.crate_name(id.krate),
        cx.tcx.def_path(id).to_string_no_crate_verbose()
    )
}

fn snippet(cx: &LateContext<'_>, span: Span) -> Option<String> {
    cx.sess().source_map().span_to_snippet(span).ok()
}

fn authored(span: Span) -> Span {
    span.with_ctxt(SyntaxContext::root())
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir if !normalized.pop() => return None,
            Component::ParentDir | Component::CurDir => {}
            _ => normalized.push(part),
        }
    }
    Some(normalized)
}

fn path_binding(expr: &Expr<'_>) -> Option<HirId> {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(_, path)) => match path.res {
            Res::Local(binding) => Some(binding),
            _ => None,
        },
        _ => None,
    }
}

fn boolean_value_path(expr: &Expr<'_>) -> bool {
    path_binding(expr).is_some()
        || matches!(
            expr.kind,
            ExprKind::Path(QPath::Resolved(_, path))
                if matches!(path.res, Res::Def(DefKind::ConstParam, _))
        )
}

fn single_macro_rule(source: &str, argument: &str) -> bool {
    let words = significant_tokens(source)
        .map(|(_, word)| word)
        .collect::<Vec<_>>();
    if !matches!(
        words.as_slice(),
        ["macro_rules", "!", _, "{" | "(" | "[", ..]
    ) {
        return false;
    }

    let mut depth = 0;
    let mut rules = 0;
    for (index, word) in words.iter().enumerate() {
        match *word {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "=" if depth == 1 && words.get(index + 1) == Some(&">") => rules += 1,
            _ => {}
        }
    }

    // The metavariable is bound as an expression and appears once in the template.
    // A single rule prevents the edited token from selecting a different matcher.
    depth == 0
        && rules == 1
        && words
            .windows(4)
            .any(|tokens| tokens == ["$", argument, ":", "expr"])
        && words
            .windows(2)
            .filter(|tokens| *tokens == ["$", argument])
            .count()
            == 2
}

fn forwards_expression(source: &str, index: usize, count: usize, argument: &str) -> bool {
    let words = significant_tokens(source)
        .map(|(_, word)| word)
        .collect::<Vec<_>>();
    let ["(", inner @ .., ")"] = words.as_slice() else {
        return false;
    };
    let mut arguments = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (index, word) in inner.iter().enumerate() {
        match *word {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => {
                arguments.push(&inner[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    if start != inner.len() {
        arguments.push(&inner[start..]);
    }
    depth == 0 && arguments.len() == count && arguments.get(index) == Some(&&["$", argument][..])
}

fn path_resolution(cx: &LateContext<'_>, path: &QPath<'_>, id: HirId) -> Res {
    match *path {
        QPath::Resolved(_, path) => path.res,
        QPath::TypeRelative(..) => cx.tcx.typeck(id.owner.def_id).qpath_res(path, id),
    }
}

fn path_ident(path: &QPath<'_>) -> Option<Ident> {
    match *path {
        QPath::Resolved(_, path) => Some(path.segments.last()?.ident),
        QPath::TypeRelative(_, segment) => Some(segment.ident),
    }
}

fn integer_from_bool(cx: &LateContext<'_>, call: &Expr<'_>, argument: HirId) -> Option<String> {
    let ExprKind::Call(callee, [arg]) = call.kind else {
        return None;
    };
    let ExprKind::Path(ref path) = callee.kind else {
        return None;
    };
    let Res::Def(DefKind::AssocFn, method) = path_resolution(cx, path, callee.hir_id) else {
        return None;
    };
    if arg.hir_id != argument
        || !cx
            .tcx
            .is_diagnostic_item(sym::From, cx.tcx.trait_of_assoc(method)?)
    {
        return None;
    }
    let typeck = cx.tcx.typeck(call.hir_id.owner.def_id);
    let ty = typeck.expr_ty(call);
    if !typeck.expr_ty(arg).is_bool() || !ty.is_integral() {
        return None;
    }

    // Rust supplies the conversion because both the trait and the types are
    // primitive or standard library items. Preserve the integer type and 0/1 values.
    let source = snippet(cx, call.span)?;
    if tokenize(&source, FrontmatterAllowed::No).any(|token| {
        matches!(
            token.kind,
            TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        )
    }) {
        return None;
    }
    Some(ty.to_string())
}

fn direct_call<'hir>(
    cx: &LateContext<'_>,
    call: &Expr<'hir>,
) -> Option<(DefId, &'hir [Expr<'hir>], usize)> {
    match call.kind {
        ExprKind::Call(callee, arguments)
            if let ExprKind::Path(ref path) = callee.kind
                && let Res::Def(DefKind::Fn | DefKind::AssocFn, id) =
                    path_resolution(cx, path, callee.hir_id) =>
        {
            Some((id, arguments, 0))
        }
        ExprKind::MethodCall(_, _, arguments, _) => Some((
            cx.tcx
                .typeck(call.hir_id.owner.def_id)
                .type_dependent_def_id(call.hir_id)?,
            arguments,
            1,
        )),
        _ => None,
    }
}

impl Family {
    fn valid(&self) -> bool {
        crate_path(&self.enum_path)
            && self.inherent_type.as_deref().is_none_or(crate_path)
            && identifier(&self.when_true)
            && identifier(&self.when_false)
            && self.when_true != self.when_false
            && !self.members.is_empty()
            && self
                .members
                .iter()
                .all(|(name, arg)| identifier(name) && identifier(arg))
            && !self.sources.is_empty()
            && self
                .forwarded_macro_arguments
                .iter()
                .all(|(name, argument)| {
                    self.reviewed_macros.contains(name) && identifier(argument)
                })
            && ((!self.reviewed_archmage && self.reviewed_macros.is_empty())
                || self.target.is_some())
    }

    fn variant(&self, value: bool) -> String {
        let name = if value {
            &self.when_true
        } else {
            &self.when_false
        };
        format!("{}::{name}", self.enum_path)
    }

    fn enum_id(&self, cx: &LateContext<'_>) -> Option<LocalDefId> {
        let path = self
            .enum_path
            .replacen("crate", cx.tcx.crate_name(LOCAL_CRATE).as_str(), 1);
        if path.rsplit_once("::")?.0 != self.module {
            return None;
        }
        cx.tcx.hir_crate_items(()).free_items().find_map(|id| {
            let item = cx.tcx.hir_item(id);
            let ItemKind::Enum(_, generics, definition) = item.kind else {
                return None;
            };
            let id = item.owner_id.def_id;
            (qualified(cx, id.to_def_id()) == path
                && generics.params.is_empty()
                && definition.variants.len() == 2
                && definition.variants.iter().all(|variant| {
                    matches!(variant.data, VariantData::Unit(..))
                        && [self.when_true.as_str(), self.when_false.as_str()]
                            .contains(&variant.ident.name.as_str())
                })
                && cx.type_is_copy_modulo_regions(
                    cx.tcx.type_of(id).instantiate_identity().skip_norm_wip(),
                ))
            .then_some(id)
        })
    }

    fn member(&self, cx: &LateContext<'_>, id: LocalDefId) -> Option<Member> {
        let (ident, sig, body, span) = match cx.tcx.hir_node_by_def_id(id) {
            Node::Item(item) if self.inherent_type.is_none() => {
                let ItemKind::Fn {
                    ident, sig, body, ..
                } = item.kind
                else {
                    return None;
                };
                let name = ident.name.as_str();
                if item.eii
                    || !item.vis_span.is_empty()
                    || (!self.members.contains_key(name)
                        && !self
                            .reviewed_archmage
                            .then(|| name.strip_prefix("__arcane_"))
                            .flatten()
                            .is_some_and(|name| self.members.contains_key(name)))
                    || qualified(cx, id.to_def_id()) != format!("{}::{name}", self.module)
                {
                    return None;
                }
                (ident, sig, body, item.span)
            }
            Node::ImplItem(item) if let Some(owner) = &self.inherent_type => {
                let ImplItemKind::Fn(sig, body) = item.kind else {
                    return None;
                };
                if !matches!(item.impl_kind, ImplItemImplKind::Inherent { .. })
                    || !self.members.contains_key(item.ident.name.as_str())
                    || cx.effective_visibilities.is_reachable(id)
                {
                    return None;
                }
                let implementation = cx.tcx.impl_of_assoc(id.to_def_id())?;
                let ty = cx
                    .tcx
                    .type_of(implementation)
                    .instantiate_identity()
                    .skip_norm_wip();
                let owner_path =
                    owner.replacen("crate", cx.tcx.crate_name(LOCAL_CRATE).as_str(), 1);
                if qualified(cx, ty.ty_adt_def()?.did()) != owner_path
                    || qualified(cx, cx.tcx.parent(implementation)) != self.module
                {
                    return None;
                }
                (item.ident, sig, body, item.span)
            }
            _ => return None,
        };

        let name = ident.name.as_str();
        let generated_name = self
            .reviewed_archmage
            .then(|| name.strip_prefix("__arcane_"))
            .flatten();
        let written_name = generated_name.unwrap_or(name);
        let parameter = self.members.get(written_name)?;
        if cx.tcx.codegen_fn_attrs(id).contains_extern_indicator()
            || sig.header.abi != ExternAbi::Rust
            || sig.header.is_const()
            || sig.header.is_async()
            || snippet(cx, ident.span)?.as_str() != written_name
            || !self.reviewed_span(cx, span, false)
        {
            return None;
        }
        let body = cx.tcx.hir_body(body);
        // An inactive statement or arm can use the old decision in a way
        // this compilation cannot see.
        if identifiers(&snippet(cx, body.value.span)?)
            .any(|(_, name)| matches!(name, "cfg" | "cfg_attr"))
        {
            return None;
        }
        for (index, (param, ty)) in body.params.iter().zip(sig.decl.inputs).enumerate() {
            if let PatKind::Binding(BindingMode::NONE, binding, written, None) = param.pat.kind
                && written.name.as_str() == parameter
                && super::hir_ty_is_bool(ty)
                && snippet(cx, ty.span)? == "bool"
            {
                return Some(Member {
                    index,
                    binding,
                    ident: ident.span,
                    ty: ty.span,
                    generated: generated_name.is_some(),
                });
            }
        }
        None
    }

    fn reviewed_span(&self, cx: &LateContext<'_>, span: Span, call: bool) -> bool {
        span.macro_backtrace().all(|expansion| {
            let Some(id) = expansion.macro_def_id else {
                return false;
            };
            let name = cx.tcx.item_name(id);
            let krate = cx.tcx.crate_name(id.krate);
            (matches!(krate.as_str(), "std" | "core") && name.as_str() == "include")
                || (self.reviewed_archmage
                    && krate.as_str() == "archmage_macros"
                    && matches!(name.as_str(), "rite" | "arcane"))
                || (call && id.is_local() && self.reviewed_macros.contains(&qualified(cx, id)))
        })
    }

    fn edits(&self, cx: &LateContext<'_>) -> Option<(BytePos, Edits)> {
        if self.module.split("::").next()? != cx.tcx.crate_name(LOCAL_CRATE).as_str()
            || self
                .target
                .as_ref()
                .is_some_and(|target| target.as_str() != &*cx.sess().target.llvm_target)
            || !cx.sess().edition().at_least_rust_2018()
        {
            return None;
        }

        let enum_id = self.enum_id(cx)?;
        let members = cx
            .tcx
            .hir_body_owners()
            .filter_map(|id| self.member(cx, id).map(|member| (id, member)))
            .collect::<HashMap<_, _>>();
        // An Archmage wrapper and its implementation share one written signature.
        let authored_members = members.values().filter(|member| !member.generated);
        if authored_members.clone().count() != self.members.len()
            || members
                .values()
                .filter(|member| member.generated)
                .any(|generated| {
                    !authored_members.clone().any(|member| {
                        member.ident.source_equal(generated.ident)
                            && member.ty.source_equal(generated.ty)
                            && member.index == generated.index
                    })
                })
        {
            return None;
        }
        let anchor = authored_members
            .clone()
            .map(|member| member.ty.lo())
            .min()?;
        let mut plan = Plan {
            bindings: members.values().map(|member| member.binding).collect(),
            covered: authored_members
                .clone()
                .map(|member| member.ident.lo())
                .collect(),
            edits: authored_members
                .map(|member| (authored(member.ty), self.enum_path.clone()))
                .collect(),
            members,
            other_names: HashSet::new(),
            enum_id,
            seen_branches: HashSet::new(),
        };

        for owner in cx.tcx.hir_body_owners() {
            if self.inherent_type.is_some() && !plan.members.contains_key(&owner) {
                let ident = match cx.tcx.hir_node_by_def_id(owner) {
                    Node::ImplItem(item) if matches!(item.kind, ImplItemKind::Fn(..)) => {
                        Some(item.ident)
                    }
                    Node::Item(item) if let ItemKind::Fn { ident, .. } = item.kind => Some(ident),
                    _ => None,
                };
                if let Some(ident) = ident {
                    self.record_other(cx, ident, &mut plan);
                }
            }
            let unsupported =
                for_each_expr_without_closures(cx.tcx.hir_body_owned_by(owner), |expr| {
                    if self.walk(cx, expr, &mut plan).is_some() {
                        ControlFlow::Continue(())
                    } else {
                        ControlFlow::Break(())
                    }
                });
            if unsupported.is_some() {
                return None;
            }
        }

        if !plan.covered.is_disjoint(&plan.other_names)
            || self.written_names(cx, &plan.other_names)? != plan.covered
        {
            return None;
        }
        plan.edits.sort_by_key(|(span, _)| (span.lo(), span.hi()));
        plan.edits.dedup();
        if plan
            .edits
            .windows(2)
            .any(|pair| pair[0].0.hi() > pair[1].0.lo())
        {
            return None;
        }
        for (span, _) in &plan.edits {
            let file = cx.sess().source_map().lookup_source_file(span.lo());
            let name = file.name.prefer_local_unconditionally().to_string();
            let path = normalize(Path::new(&name))?;
            if !self.sources.iter().any(|source| path.ends_with(source)) {
                return None;
            }
        }
        Some((anchor, plan.edits))
    }

    fn record_other(&self, cx: &LateContext<'_>, ident: Ident, plan: &mut Plan) {
        let name = ident.name.as_str();
        if self.members.contains_key(name) && snippet(cx, ident.span).as_deref() == Some(name) {
            plan.other_names.insert(ident.span.lo());
        }
    }

    fn forwarded_literal(
        &self,
        cx: &LateContext<'_>,
        call: &Expr<'_>,
        argument: &Expr<'_>,
    ) -> Option<()> {
        if self.forwarded_macro_arguments.is_empty()
            || !matches!(argument.kind, ExprKind::Lit(..))
            || !self.reviewed_span(cx, argument.span, false)
        {
            return None;
        }
        let ExprKind::Call(callee, arguments) = call.kind else {
            return None;
        };
        if !call.span.contains(callee.span) {
            return None;
        }
        let index = arguments
            .iter()
            .position(|arg| arg.hir_id == argument.hir_id)?;
        let template = snippet(cx, call.span.with_lo(callee.span.hi()))?;
        call.span.macro_backtrace().find_map(|expansion| {
            let id = expansion.macro_def_id?.as_local()?;
            let name = self
                .forwarded_macro_arguments
                .get(&qualified(cx, id.to_def_id()))?;
            let Node::Item(item) = cx.tcx.hir_node_by_def_id(id) else {
                return None;
            };
            (matches!(item.kind, ItemKind::Macro(..))
                && expansion.call_site.contains(argument.span)
                && single_macro_rule(&snippet(cx, item.span)?, name)
                && forwards_expression(&template, index, arguments.len(), name))
            .then_some(())
        })
    }

    fn rewrite_call(
        &self,
        cx: &LateContext<'_>,
        call: &Expr<'_>,
        ident: Ident,
        argument: &Expr<'_>,
        plan: &mut Plan,
    ) -> Option<()> {
        if !self.reviewed_span(cx, call.span, true)
            || snippet(cx, ident.span)?.as_str() != ident.name.as_str()
            || (!call.span.contains(argument.span)
                && self.forwarded_literal(cx, call, argument).is_none())
        {
            return None;
        }

        let from = cx.tcx.parent_module_from_def_id(call.hir_id.owner.def_id);
        let mut item = plan.enum_id.to_def_id();
        while let Some(parent) = cx.tcx.opt_parent(item) {
            if !cx.tcx.visibility(item).is_accessible_from(from, cx.tcx) {
                return None;
            }
            item = parent;
        }

        plan.covered.insert(ident.span.lo());
        if path_binding(argument).is_some_and(|binding| plan.bindings.contains(&binding)) {
            return Some(());
        }
        let source = snippet(cx, argument.span)?;
        let replacement = match source.as_str() {
            "true" => self.variant(true),
            "false" => self.variant(false),
            _ if boolean_value_path(argument) && identifier(&source) => format!(
                "match ::core::primitive::bool::then_some({source}, ()) {{ ::core::option::Option::Some(()) => {}, ::core::option::Option::None => {} }}",
                self.variant(true),
                self.variant(false),
            ),
            _ => return None,
        };
        plan.edits.push((authored(argument.span), replacement));
        Some(())
    }

    fn walk(&self, cx: &LateContext<'_>, expr: &Expr<'_>, plan: &mut Plan) -> Option<()> {
        if self.inherent_type.is_some()
            && let ExprKind::MethodCall(segment, _, _, _) = expr.kind
            && self.members.contains_key(segment.ident.name.as_str())
        {
            let (id, args, receiver) = direct_call(cx, expr)?;
            let Some(member) = id.as_local().and_then(|id| plan.members.get(&id)) else {
                self.record_other(cx, segment.ident, plan);
                return Some(());
            };
            let argument = args.get(member.index.checked_sub(receiver)?)?;
            return self.rewrite_call(cx, expr, segment.ident, argument, plan);
        }

        let ExprKind::Path(ref path) = expr.kind else {
            return Some(());
        };
        if let QPath::TypeRelative(_, segment) = path
            && (self.inherent_type.is_none()
                || !self.members.contains_key(segment.ident.name.as_str()))
        {
            return Some(());
        }
        match path_resolution(cx, path, expr.hir_id) {
            Res::Def(DefKind::Fn | DefKind::AssocFn, id)
                if id
                    .as_local()
                    .is_some_and(|id| plan.members.contains_key(&id)) =>
            {
                let member = plan.members.get(&id.as_local()?)?;
                let Node::Expr(parent) = cx.tcx.parent_hir_node(expr.hir_id) else {
                    return None;
                };
                let ExprKind::Call(callee, arguments) = parent.kind else {
                    // A function pointer or callback shares the old signature.
                    return None;
                };
                if callee.hir_id != expr.hir_id {
                    return None;
                }
                let argument = arguments.get(member.index)?;
                if member.generated {
                    // The generated call has no separate authored argument to edit.
                    let wrapper = plan.members.get(&expr.hir_id.owner.def_id)?;
                    return (!wrapper.generated
                        && self.reviewed_span(cx, parent.span, true)
                        && member.ident.source_equal(wrapper.ident)
                        && member.ty.source_equal(wrapper.ty)
                        && member.ident.source_equal(expr.span)
                        && path_binding(argument) == Some(wrapper.binding))
                    .then_some(());
                }
                return self.rewrite_call(cx, parent, path_ident(path)?, argument, plan);
            }
            Res::Local(binding) if plan.bindings.contains(&binding) => {
                let Node::Expr(parent) = cx.tcx.parent_hir_node(expr.hir_id) else {
                    return None;
                };
                if let Some((target, arguments, receiver)) = direct_call(cx, parent)
                    && let Some(member) = target.as_local().and_then(|id| plan.members.get(&id))
                    && arguments.get(member.index.checked_sub(receiver)?)?.hir_id == expr.hir_id
                {
                    return Some(());
                }

                if self.reviewed_span(cx, parent.span, false)
                    && let Some(integer) = integer_from_bool(cx, parent, expr.hir_id)
                {
                    let value = snippet(cx, expr.span)?;
                    plan.edits.push((
                        authored(parent.span),
                        format!(
                            "(match {value} {{ {} => 1_{integer}, {} => 0_{integer} }})",
                            self.variant(true),
                            self.variant(false),
                        ),
                    ));
                    return Some(());
                }

                let (scrutinee, branch) = if let ExprKind::Call(callee, [arg, unit]) = parent.kind
                    && arg.hir_id == expr.hir_id
                    && matches!(unit.kind, ExprKind::Tup([]))
                    && snippet(cx, callee.span)? == "::core::primitive::bool::then_some"
                    && let Node::Expr(branch) = cx.tcx.parent_hir_node(parent.hir_id)
                {
                    (parent, branch)
                } else {
                    (expr, parent)
                };
                let ExprKind::Match(matched, [first, second], MatchSource::Normal) = branch.kind
                else {
                    return None;
                };
                if matched.hir_id != scrutinee.hir_id
                    || !self.reviewed_span(cx, branch.span, false)
                {
                    return None;
                }
                if plan.seen_branches.insert(scrutinee.span.lo()) {
                    let mut outcomes = Vec::new();
                    for arm in [first, second] {
                        if arm.guard.is_some() || !branch.span.contains(arm.pat.span) {
                            return None;
                        }
                        let pattern = snippet(cx, arm.pat.span)?;
                        let value = match (scrutinee.hir_id == expr.hir_id, pattern.as_str()) {
                            (true, "true") | (false, "::core::option::Option::Some(())") => true,
                            (true, "false") | (false, "::core::option::Option::None") => false,
                            _ => return None,
                        };
                        outcomes.push(value);
                        plan.edits
                            .push((authored(arm.pat.span), self.variant(value)));
                    }
                    if outcomes[0] == outcomes[1] {
                        return None;
                    }
                    if scrutinee.hir_id != expr.hir_id {
                        plan.edits
                            .push((authored(scrutinee.span), snippet(cx, expr.span)?));
                    }
                }
            }
            _ => {
                let ident = path_ident(path)?;
                let name = ident.name.as_str();
                if self.members.contains_key(name) {
                    if self.inherent_type.is_none()
                        && snippet(cx, ident.span).as_deref() == Some(name)
                    {
                        return None;
                    }
                    match path {
                        QPath::TypeRelative(..) => self.record_other(cx, ident, plan),
                        QPath::Resolved(_, path) if path.segments.len() > 1 => {
                            self.record_other(cx, ident, plan);
                        }
                        _ => {}
                    }
                }
            }
        }
        Some(())
    }

    fn written_names(
        &self,
        cx: &LateContext<'_>,
        other: &HashSet<BytePos>,
    ) -> Option<HashSet<BytePos>> {
        let map = cx.sess().source_map();
        let mut expected = self.sources.clone();
        let mut sites = HashSet::new();
        for file in map.files().iter().filter(|file| file.cnum == LOCAL_CRATE) {
            let Some(source) = file.src.as_ref() else {
                continue;
            };
            let filename = file.name.prefer_local_unconditionally().to_string();
            let filename = normalize(Path::new(&filename))?;
            let reviewed = self
                .sources
                .iter()
                .find(|expected| filename.ends_with(Path::new(expected)))
                .cloned();
            if let Some(name) = &reviewed {
                expected.remove(name);
            }
            let names = if self.inherent_type.is_some() {
                method_names(source)
            } else {
                identifiers(source).collect()
            };
            for (offset, name) in names {
                if self.reviewed_archmage
                    && name
                        .strip_prefix("__arcane_")
                        .is_some_and(|name| self.members.contains_key(name))
                {
                    return None;
                }
                if self.members.contains_key(name) {
                    let pos = file.start_pos + BytePos(u32::try_from(offset).ok()?);
                    if !other.contains(&pos) {
                        reviewed.as_ref()?;
                        sites.insert(pos);
                    }
                }
            }
        }
        expected.is_empty().then_some(sites)
    }
}
