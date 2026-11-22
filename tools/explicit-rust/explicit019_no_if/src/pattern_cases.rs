use clippy_utils::{is_refutable, sym};
use rustc_hir::def::{CtorKind, DefKind, Res};
use rustc_hir::{
    HirId, ItemKind, Node, Pat, PatExpr, PatExprKind, PatKind, Path, PathSegment, QPath, UseKind,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{AdtDef, TyKind, VariantDef};

use super::{conditional_definition, conditional_import, conditional_type};

/// Name the cases outside this pattern. A tuple fails when any field fails.
pub(super) fn complement<'tcx>(
    cx: &LateContext<'tcx>,
    pattern: &'tcx Pat<'tcx>,
) -> Option<Vec<String>> {
    if pattern.span.from_expansion() {
        return None;
    }
    if !is_refutable(cx, pattern) {
        return Some(Vec::new());
    }
    let ty = cx.typeck_results().pat_ty(pattern).peel_refs();
    let (path, fields, id) = match pattern.kind {
        PatKind::Tuple(fields, _) => {
            let TyKind::Tuple(types) = ty.kind() else {
                return None;
            };
            return (fields.len() == types.len())
                .then(|| field_cases(cx, fields, ""))
                .flatten();
        }
        PatKind::TupleStruct(ref path, fields, _) => (path, Some(fields), pattern.hir_id),
        PatKind::Expr(PatExpr {
            kind: PatExprKind::Path(path),
            hir_id,
            ..
        }) => (path, None, *hir_id),
        _ => return None,
    };
    let adt = ty.ty_adt_def().filter(|adt| adt.is_enum())?;
    let constructor = cx.qpath_res(path, id).opt_def_id()?;
    let variant = adt
        .variants()
        .iter()
        .find(|variant| variant.ctor_def_id() == Some(constructor))?;
    let prefix = enum_prefix(cx, adt, path, pattern.hir_id)?;
    let mut cases: Vec<_> = adt
        .variants()
        .iter()
        .filter(|sibling| sibling.def_id != variant.def_id)
        .map(|sibling| variant_case(cx, &prefix, sibling))
        .collect();
    if let Some(fields) = fields
        && fields.iter().any(|field| is_refutable(cx, field))
    {
        if fields.len() != variant.fields.len() {
            return None;
        }
        let constructor = format!("{prefix}::{}", variant_name(cx, variant));
        cases.extend(field_cases(cx, fields, &constructor)?);
    }
    Some(cases)
}

fn field_cases<'tcx>(
    cx: &LateContext<'tcx>,
    fields: &'tcx [Pat<'tcx>],
    constructor: &str,
) -> Option<Vec<String>> {
    let mut cases = Vec::new();
    let mut patterns = vec!["_".to_owned(); fields.len()];
    let comma = if fields.len() == 1 && constructor.is_empty() {
        ","
    } else {
        ""
    };
    for (index, field) in fields.iter().enumerate() {
        let failures = complement(cx, field)?;
        if !failures.is_empty() {
            patterns[index] = failures.join(" | ");
            cases.push(format!("{constructor}({}{comma})", patterns.join(", ")));
            patterns[index] = "_".to_owned();
        }
    }
    Some(cases)
}

fn enum_prefix(
    cx: &LateContext<'_>,
    adt: AdtDef<'_>,
    path: &QPath<'_>,
    use_site: HirId,
) -> Option<String> {
    match path {
        QPath::Resolved(None, path) if unstable_import_path(cx, use_site, path) => return None,
        QPath::TypeRelative(ty, _) if unstable_type_origin(cx, ty) => return None,
        _ => {}
    }
    match cx.tcx.get_diagnostic_name(adt.did()) {
        Some(sym::Option) => return Some("::core::option::Option".to_owned()),
        Some(sym::Result) => return Some("::core::result::Result".to_owned()),
        _ => {}
    }
    let definition = adt.did().as_local()?;
    let Node::Item(item) = cx.tcx.hir_node_by_def_id(definition) else {
        return None;
    };
    let map = cx.sess().source_map();
    let declaration = map.span_to_snippet(item.span).ok()?;
    if item.span.from_expansion()
        || conditional_definition(cx, definition)
        || adt.variants().len() > 16
        || tokenize(&declaration, FrontmatterAllowed::No)
            .any(|token| token.kind == TokenKind::Pound)
    {
        return None;
    }
    let prefix = match path {
        QPath::Resolved(None, path) => {
            let text = map.span_to_snippet(path.span).ok()?;
            text.rsplit_once("::")?.0.trim_end().to_owned()
        }
        QPath::TypeRelative(ty, _) => map.span_to_snippet(ty.span).ok()?,
        _ => return None,
    };
    (!prefix.is_empty() && !prefix.contains(['<', '/', '\n'])).then_some(prefix)
}

fn unstable_type_origin(cx: &LateContext<'_>, ty: &rustc_hir::Ty<'_>) -> bool {
    if conditional_type(cx, ty) {
        return true;
    }
    let rustc_hir::TyKind::Path(QPath::Resolved(None, path)) = ty.kind else {
        return false;
    };
    if unstable_import_path(cx, ty.hir_id, path) {
        return true;
    }
    match path.res {
        Res::SelfTyAlias { alias_to, .. } => {
            let Some(implementation) = alias_to.as_local() else {
                return true;
            };
            match cx.tcx.hir_node_by_def_id(implementation) {
                Node::Item(item) => match item.kind {
                    ItemKind::Impl(implementation) => unstable_type_origin(cx, implementation.self_ty),
                    _ => true,
                },
                _ => true,
            }
        }
        Res::Def(DefKind::TyAlias, definition) => definition.as_local().is_some_and(|definition| {
            matches!(cx.tcx.hir_node_by_def_id(definition), Node::Item(item)
                if matches!(item.kind, ItemKind::TyAlias(_, _, alias) if unstable_type_origin(cx, alias)))
        }),
        _ => false,
    }
}

fn unstable_import_path(cx: &LateContext<'_>, use_site: HirId, path: &Path<'_>) -> bool {
    if conditional_import(cx, use_site, path) || qualified_reexport(cx, path.segments) {
        return true;
    }
    let Some(first) = path.segments.first() else {
        return false;
    };
    let scope = cx.tcx.parent_module_from_def_id(use_site.owner.def_id);
    cx.tcx.hir_module_free_items(scope).any(|id| {
        matches!(cx.tcx.hir_item(id).kind, ItemKind::Use(origin, UseKind::Single(name))
            if name.name == first.ident.name && qualified_reexport(cx, origin.segments))
    })
}

fn qualified_reexport(cx: &LateContext<'_>, segments: &[PathSegment<'_>]) -> bool {
    segments
        .iter()
        .zip(segments.iter().skip(1))
        .any(|(prefix, member)| {
            let Res::Def(DefKind::Mod, module) = prefix.res else {
                return false;
            };
            let Some(local_module) = module.as_local() else {
                return false;
            };
            let items = match cx.tcx.hir_node_by_def_id(local_module) {
                Node::Crate(module) => module.item_ids,
                Node::Item(item) => match item.kind {
                    ItemKind::Mod(_, module) => module.item_ids,
                    _ => return true,
                },
                _ => return true,
            };
            if conditional_definition(cx, local_module) {
                return true;
            }
            let declared_here = member.res.opt_def_id().is_some_and(|definition| {
                cx.tcx.opt_parent(definition) == Some(module)
                    && cx.tcx.item_name(definition) == member.ident.name
            });

            // A resolved segment points to its final declaration. A re-export can
            // select a different declaration through another module in a cfg build.
            items.iter().any(|&id| match cx.tcx.hir_item(id).kind {
                ItemKind::Use(_, UseKind::Single(name)) => name.name == member.ident.name,
                ItemKind::Use(_, UseKind::Glob) => !declared_here,
                _ => false,
            })
        })
}

fn variant_case(cx: &LateContext<'_>, prefix: &str, variant: &VariantDef) -> String {
    let fields = match variant.ctor_kind() {
        Some(CtorKind::Const) => "",
        Some(CtorKind::Fn) if variant.fields.len() == 1 => "(_)",
        Some(CtorKind::Fn) => "(..)",
        None => " { .. }",
    };
    format!("{prefix}::{}{fields}", variant_name(cx, variant))
}

fn variant_name(cx: &LateContext<'_>, variant: &VariantDef) -> String {
    if variant.name.is_reserved(|| cx.sess().edition()) {
        format!("r#{}", variant.name)
    } else {
        variant.name.to_string()
    }
}
