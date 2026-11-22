//! Preserve platform-width public signatures at the compatibility boundary.

use rustc_abi::ExternAbi;
use rustc_hir::def_id::CRATE_DEF_ID;
use rustc_hir::{
    ItemKind, Node, QPath, Ty, TyKind,
    def::{DefKind, Res},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;

const CRATE: &str = "console_core_api_compatibility";

fn exported_signature(cx: &LateContext<'_>, written: &Ty<'_>) -> bool {
    for (_, parent) in cx.tcx.hir_parent_iter(written.hir_id) {
        match parent {
            Node::Expr(_) | Node::Block(_) | Node::AnonConst(_) | Node::LetStmt(_) => {
                return false;
            }
            Node::ForeignItem(_) => return true,
            Node::Field(field) => return cx.effective_visibilities.is_exported(field.def_id),
            Node::ImplItem(item) => {
                return cx.effective_visibilities.is_exported(item.owner_id.def_id)
                    || cx
                        .tcx
                        .associated_item(item.owner_id)
                        .trait_item_def_id()
                        .is_some_and(|id| {
                            !id.is_local()
                                || id.as_local().is_some_and(|local| {
                                    cx.effective_visibilities.is_exported(local)
                                })
                        });
            }
            Node::TraitItem(item) => {
                return cx.effective_visibilities.is_exported(item.owner_id.def_id);
            }
            Node::Item(item) => {
                return cx.effective_visibilities.is_exported(item.owner_id.def_id)
                    || matches!(item.kind, ItemKind::Fn { sig, .. } if sig.header.abi != ExternAbi::Rust);
            }
            _ => {}
        }
    }
    false
}

pub(super) fn internal_alias(cx: &LateContext<'_>, written: &Ty<'_>) -> Option<String> {
    let TyKind::Path(QPath::Resolved(_, path)) = written.kind else {
        return None;
    };
    let Res::Def(DefKind::TyAlias, id) = path.res else {
        return None;
    };
    if cx.tcx.crate_name(id.krate).as_str() != CRATE || exported_signature(cx, written) {
        return None;
    }
    match cx.tcx.item_name(id).as_str() {
        "NativeSize" => Some("usize".to_owned()),
        "NativeOffset" => Some("isize".to_owned()),
        _ => None,
    }
}

pub(super) fn replacement(cx: &LateContext<'_>, written: &Ty<'_>, width: &str) -> Option<String> {
    let name = match width {
        "usize" => "NativeSize",
        "isize" => "NativeOffset",
        _ => return None,
    };
    if !cx.sess().edition().at_least_rust_2018()
        || !exported_signature(cx, written)
        || !written.span.can_be_used_for_suggestions()
    {
        return None;
    }
    super::fixes::clean(cx, written.span)?;
    let krate = cx
        .tcx
        .crates(())
        .iter()
        .find(|id| cx.tcx.crate_name(**id).as_str() == CRATE)?;
    if !cx
        .sess()
        .opts
        .externs
        .get(CRATE)
        .is_some_and(|entry| entry.add_prelude)
        && !cx
            .tcx
            .module_children_local(CRATE_DEF_ID)
            .iter()
            .any(|child| {
                child.ident.as_str() == CRATE && child.res.opt_def_id() == Some(krate.as_def_id())
            })
    {
        return None;
    }
    let alias = cx
        .tcx
        .module_children(krate.as_def_id())
        .iter()
        .find(|child| child.ident.as_str() == name && child.vis.is_public())?;
    let Res::Def(DefKind::TyAlias, id) = alias.res else {
        return None;
    };
    if cx.tcx.generics_of(id).count() != 0
        || !matches!(
            (
                cx.tcx
                    .type_of(id)
                    .instantiate_identity()
                    .skip_normalization()
                    .kind(),
                width
            ),
            (ty::Uint(ty::UintTy::Usize), "usize") | (ty::Int(ty::IntTy::Isize), "isize")
        )
    {
        return None;
    }
    Some(format!("::{CRATE}::{name}"))
}
