use rustc_hir::attrs::AttributeKind;
use rustc_hir::def::CtorKind;
use rustc_hir::def_id::{DefId, LocalDefId, LocalModDefId};
use rustc_hir::intravisit::{Visitor, VisitorExt, walk_qpath};
use rustc_hir::{Attribute, ExprKind, FnRetTy, HirId, Node, QPath, Ty as HirTy};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, sym};
use std::ops::ControlFlow;

use super::local_type_path::{identifier, local_path};

fn conditional(cx: &LateContext<'_>, definition: LocalDefId) -> bool {
    let id = cx.tcx.local_def_id_to_hir_id(definition);
    std::iter::once(id)
        .chain(cx.tcx.hir_parent_id_iter(id))
        .flat_map(|id| cx.tcx.hir_attrs(id))
        .any(|attr| {
            matches!(
                attr,
                Attribute::Parsed(AttributeKind::CfgTrace(..) | AttributeKind::CfgAttrTrace)
            )
        })
}

struct ConditionalType<'cx, 'tcx>(&'cx LateContext<'tcx>);

impl<'tcx> Visitor<'tcx> for ConditionalType<'_, 'tcx> {
    type Result = ControlFlow<()>;

    fn visit_qpath(&mut self, path: &'tcx QPath<'tcx>, id: HirId, _: Span) -> Self::Result {
        if let Some(definition) = self
            .0
            .qpath_res(path, id)
            .opt_def_id()
            .and_then(DefId::as_local)
        {
            if conditional(self.0, definition) {
                return ControlFlow::Break(());
            }
            if let Some(alias) = self.0.tcx.hir_node_by_def_id(definition).alias_ty() {
                self.visit_ty_unambig(alias)?;
            }
        }
        walk_qpath(self, path, id)
    }
}

fn conditional_context<'tcx>(cx: &LateContext<'tcx>, use_site: HirId) -> bool {
    let configured = |ty: &'tcx HirTy<'tcx>| ConditionalType(cx).visit_ty_unambig(ty).is_break();
    let owner = cx.tcx.hir_enclosing_body_owner(use_site);
    if let Some(declaration) = cx.tcx.hir_node_by_def_id(owner).fn_decl()
        && (declaration.inputs.iter().any(configured)
            || matches!(declaration.output, FnRetTy::Return(ty) if configured(ty)))
    {
        return true;
    }
    cx.tcx
        .hir_parent_iter(use_site)
        .find_map(|(_, node)| match node {
            Node::LetStmt(binding) => Some(binding.ty),
            _ => None,
        })
        .flatten()
        .is_some_and(configured)
}

fn accessible_path(
    cx: &LateContext<'_>,
    definition: DefId,
    module: LocalModDefId,
) -> Option<String> {
    local_path(cx, definition, module).or_else(|| {
        let root = LocalModDefId::CRATE_DEF_ID;
        // An unconditional public crate-root re-export can expose a type
        // declared under a private module.
        let export = cx
            .tcx
            .module_children_local(root.to_local_def_id())
            .iter()
            .find(|child| {
                child.res.opt_def_id() == Some(definition)
                    && child.vis.is_public()
                    && !child.reexport_chain.is_empty()
                    && child.reexport_chain.iter().all(|reexport| {
                        reexport
                            .id()
                            .and_then(DefId::as_local)
                            .is_some_and(|id| !conditional(cx, id))
                    })
            })?;
        Some(format!("crate::{}", identifier(cx, export.ident.name)))
    })
}

pub(super) fn local_unit_variant<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    use_site: HirId,
) -> Option<String> {
    let ty::Adt(adt, arguments) = ty.kind() else {
        return None;
    };
    if !adt.is_enum()
        || !arguments.is_empty()
        || conditional(cx, adt.did().as_local()?)
        || conditional_context(cx, use_site)
    {
        return None;
    }
    let default_trait = cx.tcx.get_diagnostic_item(sym::Default)?;
    let default_fn = cx
        .tcx
        .associated_items(default_trait)
        .in_definition_order()
        .find(|item| item.name().as_str() == "default")?;
    let mut exact = Vec::new();
    cx.tcx
        .for_each_relevant_impl(default_trait, ty, |implementation| {
            if cx
                .tcx
                .type_of(implementation)
                .instantiate_identity()
                .skip_norm_wip()
                == ty
            {
                exact.push(implementation);
            }
        });
    let [implementation] = exact[..] else {
        return None;
    };
    let method = cx
        .tcx
        .impl_item_implementor_ids(implementation)
        .get(&default_fn.def_id)?
        .as_local()?;
    if conditional(cx, method) {
        return None;
    }

    let body = cx.tcx.hir_maybe_body_owned_by(method)?;
    let ExprKind::Block(block, None) = body.value.kind else {
        return None;
    };
    if !body.params.is_empty() || !block.stmts.is_empty() {
        return None;
    }
    let tail = block.expr?;
    let ExprKind::Path(ref path) = tail.kind else {
        return None;
    };
    let constructor = cx.qpath_res(path, tail.hir_id).opt_def_id()?;
    let variant = adt.variants().iter().find(|variant| {
        variant.ctor_def_id() == Some(constructor) && variant.ctor_kind() == Some(CtorKind::Const)
    })?;
    if conditional(cx, variant.def_id.as_local()?) {
        return None;
    }
    let path = accessible_path(cx, adt.did(), cx.tcx.parent_module(use_site))?;
    Some(format!("{path}::{}", identifier(cx, variant.name)))
}
