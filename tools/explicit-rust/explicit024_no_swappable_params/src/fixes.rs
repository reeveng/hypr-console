//! Group adjacent primitive arguments only when every reference is a known call.
use crate::scope::{References, Scope};
use rustc_hir::{
    BindingMode, Body, ExprKind, FnDecl, ItemKind, Node, PatKind, QPath, TyKind,
    def::Res,
    def_id::{LocalDefId, LocalModDefId},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};
use std::collections::{HashMap, HashSet};

type Edits = Vec<(Span, String)>;

#[derive(Default)]
pub(super) struct Fixes {
    references: Option<References>,
    modules: HashMap<LocalModDefId, Option<Scope>>,
}

struct Field {
    name: String,
    ty: String,
    span: Span,
    mutable: bool,
}

fn field(
    cx: &LateContext<'_>,
    param: &rustc_hir::Param<'_>,
    ty: &rustc_hir::Ty<'_>,
) -> Option<Field> {
    let PatKind::Binding(mode @ (BindingMode::NONE | BindingMode::MUT), _, name, None) =
        param.pat.kind
    else {
        return None;
    };
    let TyKind::Path(QPath::Resolved(None, path)) = ty.kind else {
        return None;
    };
    if !matches!(
        path.res,
        Res::PrimTy(
            rustc_hir::PrimTy::Int(_)
                | rustc_hir::PrimTy::Uint(_)
                | rustc_hir::PrimTy::Float(_)
                | rustc_hir::PrimTy::Char
        )
    ) || !cx.tcx.hir_attrs(param.hir_id).is_empty()
        || param.span.from_expansion()
        || ty.span.from_expansion()
    {
        return None;
    }
    let map = cx.sess().source_map();
    let ty = map.span_to_snippet(ty.span).ok()?;
    if map.span_to_snippet(name.span).ok()?.as_str() != name.as_str() {
        return None;
    }
    let name = name.as_str();
    if !name.bytes().next().is_some_and(|c| c.is_ascii_lowercase())
        || !ty.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(Field {
        name: name.to_owned(),
        ty,
        span: param.span,
        mutable: mode == BindingMode::MUT,
    })
}

fn group(fields: &[Option<Field>]) -> Option<(usize, usize, usize)> {
    let mut start = 0;
    while start < fields.len() {
        let end = start + fields[start..].iter().take_while(|f| f.is_some()).count();
        let mut first = HashMap::new();
        let mut pivot = None;
        let mut low = end;
        let mut high = start;
        for (index, field) in fields[start..end].iter().flatten().enumerate() {
            let index = start + index;
            let previous = *first.entry(&field.ty).or_insert(index);
            if previous != index {
                pivot.get_or_insert(index);
                low = low.min(previous);
                high = index + 1;
            }
        }
        if let Some(pivot) = pivot {
            return Some((low, high, pivot));
        }
        start = end + 1;
    }
    None
}

pub(super) fn argument_type(function: &str) -> Option<String> {
    let mut name = String::new();
    for segment in function.split('_') {
        let mut chars = segment.chars();
        let first = chars.next()?;
        if !(first.is_ascii_lowercase() || !name.is_empty() && first.is_ascii_digit())
            || !segment
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return None;
        }
        name.push(first.to_ascii_uppercase());
        name.extend(chars);
    }
    name.push_str("Arguments");
    Some(name)
}

impl Fixes {
    pub fn for_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        id: LocalDefId,
        decl: &FnDecl<'tcx>,
        body: &Body<'tcx>,
    ) -> Option<(usize, Edits)> {
        let Node::Item(item) = cx.tcx.hir_node_by_def_id(id) else {
            return None;
        };
        let ItemKind::Fn { ident, sig, .. } = item.kind else {
            return None;
        };
        let module = cx.tcx.parent_module_from_def_id(id);
        let map = cx.sess().source_map();
        let declaration = if item.span.from_expansion() {
            let mut archmage = false;
            for expansion in item.span.macro_backtrace() {
                let macro_id = expansion.macro_def_id?;
                if cx.tcx.crate_name(macro_id.krate).as_str() != "archmage_macros"
                    || !matches!(cx.tcx.item_name(macro_id).as_str(), "arcane" | "rite")
                {
                    return None;
                }
                archmage = true;
            }
            if !archmage || ident.span.from_expansion() {
                return None;
            }
            super::authored_function::authored_function(cx, item.span, &[ident.span])?
        } else {
            item.span
        };
        if cx.tcx.parent(id.to_def_id()) != module.to_def_id()
            || cx.tcx.visibility(id) != ty::Visibility::Restricted(module.to_def_id())
            || map
                .span_to_snippet(declaration.with_hi(ident.span.lo()))
                .ok()?
                .trim()
                != "fn"
            || cx.tcx.codegen_fn_attrs(id).contains_extern_indicator()
            || sig.header.abi != rustc_abi::ExternAbi::Rust
        {
            return None;
        }
        let fields: Vec<_> = body
            .params
            .iter()
            .zip(decl.inputs)
            .map(|(p, t)| field(cx, p, t))
            .collect();
        let (start, end, pivot) = group(&fields)?;
        let fields: Vec<_> = fields[start..end]
            .iter()
            .map(Option::as_ref)
            .collect::<Option<_>>()?;
        let group_span = fields.first()?.span.to(fields.last()?.span);
        let original = map.span_to_snippet(group_span).ok()?;
        if original.contains('#') || original.contains("//") || original.contains("/*") {
            return None;
        }
        let name = argument_type(ident.as_str())?;
        let refs = self
            .references
            .get_or_insert_with(|| References::collect(cx));
        let scope = self
            .modules
            .entry(module)
            .or_insert_with(|| {
                Scope::read(cx, cx.tcx.hir_get_module(module).0.spans.inner_span, refs)
            })
            .as_ref()?;
        if scope.generated_names.contains(ident.as_str())
            || scope.generated_names.contains(&name)
            || scope.collisions.contains(&name)
            || scope.names.contains_key(&name)
            || cx
                .tcx
                .module_children_local(module.to_local_def_id())
                .iter()
                .any(|child| child.ident.as_str() == name)
        {
            return None;
        }
        let mut written = HashSet::from([ident.span.lo()]);
        let gates = scope.gates.get(&ident.span.lo())?;
        let last = body.params.last()?.span;
        if !declaration.contains(last)
            || map.span_to_snippet(ident.span.to(last)).ok()?.contains('#')
        {
            return None;
        }
        let mut edits = Vec::new();
        for &reference in refs.paths.get(&id)? {
            let Node::Expr(callee) = cx.tcx.hir_node(reference) else {
                return None;
            };
            let Node::Expr(call) = cx.tcx.parent_hir_node(reference) else {
                return None;
            };
            let ExprKind::Call(f, args) = call.kind else {
                return None;
            };
            let ExprKind::Path(QPath::Resolved(None, path)) = callee.kind else {
                return None;
            };
            let [segment] = path.segments else {
                return None;
            };
            if !gates.is_empty() {
                let caller_gates =
                    cx.tcx
                        .hir_parent_iter(reference)
                        .find_map(|(_, node)| match node {
                            Node::Item(item) if let ItemKind::Fn { ident, .. } = item.kind => {
                                scope.gates.get(&ident.span.lo())
                            }
                            _ => None,
                        })?;
                if !gates.iter().all(|gate| caller_gates.contains(gate)) {
                    return None;
                }
            }
            if f.hir_id != reference
                || callee.span.from_expansion()
                || call.span.from_expansion()
                || map.span_to_snippet(call.span).ok()?.contains('#')
                || segment.ident.name != ident.name
                || args.len() != decl.inputs.len()
                || cx.tcx.parent_module_from_def_id(reference.owner.def_id) != module
                || cx
                    .tcx
                    .hir_parent_id_iter(call.hir_id)
                    .take_while(|p| p.owner == call.hir_id.owner)
                    .any(|p| clippy_utils::macros::root_macro_call(cx.tcx.hir_span(p)).is_some())
                || !written.insert(segment.ident.span.lo())
            {
                return None;
            }
            for (index, (field, argument)) in fields.iter().zip(&args[start..end]).enumerate() {
                if argument.span.from_expansion() || !call.span.contains(argument.span) {
                    return None;
                }
                let prefix = if index == 0 {
                    format!("self::{name} {{ ")
                } else {
                    String::new()
                };
                let same = matches!(argument.kind, ExprKind::Path(QPath::Resolved(None, path)) if matches!(path.res, Res::Local(_)))
                    && map.span_to_snippet(argument.span).ok()?.as_str() == field.name;
                let last = index + 1 == fields.len();
                if same {
                    // Shorthand depends on this expression staying an identifier.
                    // Claim its bytes so rustfix defers a competing projection.
                    let close = if last { " }" } else { "" };
                    edits.push((argument.span, format!("{prefix}{}{close}", field.name)));
                } else {
                    edits.push((
                        argument.span.shrink_to_lo(),
                        format!("{prefix}{}: ", field.name),
                    ));
                    if last {
                        edits.push((argument.span.shrink_to_hi(), " }".to_owned()));
                    }
                }
            }
        }
        if &written != scope.names.get(ident.as_str())? || written.len() < 2 {
            return None;
        }
        let patterns: Vec<_> = fields
            .iter()
            .map(|f| {
                if f.mutable {
                    format!("mut {}", f.name)
                } else {
                    f.name.clone()
                }
            })
            .collect();
        edits.push((
            group_span,
            format!("{name} {{ {} }}: {name}", patterns.join(", ")),
        ));
        let declarations: Vec<_> = fields
            .iter()
            .map(|f| format!("{}: {},", f.name, f.ty))
            .collect();
        let indent = " ".repeat(clippy_utils::source::indent_of(cx, ident.span)?);
        let ExprKind::Block(block, _) = body.value.kind else {
            return None;
        };
        let body_indent = block
            .stmts
            .first()
            .map(|s| s.span)
            .or_else(|| block.expr.map(|e| e.span))
            .and_then(|s| clippy_utils::source::indent_of(cx, s))
            .filter(|&n| n > indent.len());
        let field_indent = " ".repeat(body_indent.unwrap_or(indent.len() + 2));
        let gated = gates
            .iter()
            .map(|gate| format!("{indent}{gate}\n"))
            .collect::<String>();
        let structure = format!(
            "\n\n{gated}{indent}struct {name} {{\n{field_indent}{}\n{indent}}}",
            declarations.join(&format!("\n{field_indent}"))
        );
        let closing = declaration.with_lo(declaration.hi() - BytePos(1));
        if map.span_to_snippet(closing).ok()? != "}" {
            return None;
        }
        edits.push((declaration.shrink_to_hi(), structure));
        Some((pivot, edits))
    }
}
