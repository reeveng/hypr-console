//! Spell a local definition where the suggested edit will use it.
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalModDefId};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Symbol;

pub fn identifier(cx: &LateContext<'_>, name: Symbol) -> String {
    if name.is_reserved(|| cx.sess().edition()) {
        format!("r#{name}")
    } else {
        name.to_ident_string()
    }
}

pub fn local_path(cx: &LateContext<'_>, mut id: DefId, module: LocalModDefId) -> Option<String> {
    let mut segments = Vec::new();
    while !id.is_crate_root() {
        if !cx.tcx.visibility(id).is_accessible_from(module, cx.tcx) {
            return None;
        }
        segments.push(identifier(cx, cx.tcx.item_name(id)));
        id = cx.tcx.opt_parent(id)?;
        if cx.tcx.def_kind(id) != DefKind::Mod {
            return None;
        }
    }
    segments.push("crate".to_owned());
    segments.reverse();
    Some(segments.join("::"))
}
