//! Compiler checks for the workspace's named numeric boundaries.

use rustc_hir::{
    Expr, ExprKind, Node,
    def::{DefKind, Res},
    def_id::{CRATE_DEF_ID, DefId},
};
use rustc_lint::{LateContext, LintContext};

pub const CRATE: &str = "console_core_number_conversion";

pub fn exported(cx: &LateContext<'_>, root: DefId, name: &str, parameters: usize) -> bool {
    !root.is_local()
        && root.is_crate_root()
        && cx.tcx.crate_name(root.krate).as_str() == CRATE
        && cx.tcx.module_children(root).iter().any(|child| {
            child.ident.as_str() == name && child.vis.is_public()
                && matches!(child.res, Res::Def(DefKind::Fn, id) if cx.tcx.generics_of(id).own_params.len() == parameters)
        })
}

pub fn available(cx: &LateContext<'_>, name: &str, parameters: usize) -> bool {
    let visible = cx.sess().opts.externs.get(CRATE).is_some_and(|entry| entry.add_prelude)
        || cx.tcx.module_children_local(CRATE_DEF_ID).iter().any(|child| {
            child.ident.as_str() == CRATE
                && matches!(child.res, Res::Def(DefKind::Mod, id) if exported(cx, id, name, parameters))
        });
    visible
        && cx
            .tcx
            .crates(())
            .iter()
            .any(|id| exported(cx, id.as_def_id(), name, parameters))
}

/// Match the infallible result without introducing an inner statement scope
/// around temporaries created while evaluating the argument.
pub fn extraction(cx: &LateContext<'_>, expression: &Expr<'_>) -> (&'static str, String) {
    extraction_named(cx, expression, "converted_position")
}

pub fn extraction_named(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    binding: &str,
) -> (&'static str, String) {
    let arms = format!(" {{ ::core::result::Result::Ok({binding}) => {binding} }}");
    let needs_parentheses = match cx.tcx.parent_hir_node(expression.hir_id) {
        Node::Expr(parent) => match parent.kind {
            ExprKind::Binary(_, left, _) | ExprKind::Index(left, _, _) => {
                left.hir_id == expression.hir_id
            }
            ExprKind::Cast(..) => true,
            _ => false,
        },
        _ => false,
    };
    if needs_parentheses {
        ("(match ", format!("{arms})"))
    } else {
        ("match ", arms)
    }
}
