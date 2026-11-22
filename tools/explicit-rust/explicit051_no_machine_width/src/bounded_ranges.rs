//! Express a bounded iteration domain once at its conversion to slice indices.
use super::{CONVERSION_CRATE, boundary_conversion, fixes::clean};
use clippy_utils::higher::{ForLoop, Range};
use clippy_utils::macros::{MacroCall, macro_backtrace};
use clippy_utils::visitors::for_each_expr_without_closures;
use rustc_ast::{LitIntType, LitKind, RangeLimits};
use rustc_hir::{BindingMode, Expr, ExprKind, HirId, Node, PatKind, QPath, def::Res};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use std::ops::ControlFlow;

pub(super) fn permits_edit(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    cx.tcx
        .hir_parent_id_iter(call.hir_id)
        .take_while(|id| id.owner == call.hir_id.owner)
        .all(|id| {
            macro_backtrace(cx.tcx.hir_span(id)).all(|mac| {
                cx.tcx
                    .is_diagnostic_item(clippy_utils::sym::vec_macro, mac.def_id)
                    || archmage_body(cx, mac)
            })
        })
}

// These two attribute entry points in archmage-macros 0.9.31 pass the
// function body to a target-feature implementation. Their token rewriter
// handles their own incant/attune calls, not ranges or integer operations.
// Check the defining package recorded by rustc. A namesake macro, an
// arbitrary macro inside the body, or a different release needs review.
fn archmage_body(cx: &LateContext<'_>, mac: MacroCall) -> bool {
    let id = mac.def_id;
    if cx.tcx.crate_name(id.krate).as_str() != "archmage_macros"
        || !matches!(mac.kind, rustc_span::hygiene::MacroKind::Attr)
        || !matches!(cx.tcx.item_name(id).as_str(), "arcane" | "rite")
        || !clean(cx, mac.span).is_some_and(|attr| matches!(attr.as_str(), "#[arcane]" | "#[rite]"))
    {
        return false;
    }
    let Some(file) = cx
        .sess()
        .source_map()
        .span_to_filename(cx.tcx.def_span(id))
        .into_local_path()
    else {
        return false;
    };
    if !file.ends_with("archmage-macros-0.9.31/src/lib.rs") {
        return false;
    }
    file.ancestors().nth(3).is_some_and(|registry| {
        registry
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|s| s.starts_with("index.crates.io-"))
            && registry
                .parent()
                .is_some_and(|p| p.ends_with("registry/src"))
    })
}

fn small_literal(expr: &Expr<'_>) -> bool {
    matches!(expr.kind, ExprKind::Lit(lit) if matches!(
        lit.node, LitKind::Int(n, LitIntType::Unsuffixed) if n.get() <= u128::from(u16::MAX)
    )) && !expr.span.from_expansion()
}

fn operation<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    binding: HirId,
) -> Option<(Span, String)> {
    let ExprKind::Call(callee, [left, right]) = call.kind else {
        return None;
    };
    let ExprKind::Path(QPath::TypeRelative(written, method)) = callee.kind else {
        return None;
    };
    let ExprKind::Path(path) = left.kind else {
        return None;
    };
    let name = method.ident.as_str();
    if cx.qpath_res(&path, left.hir_id) != Res::Local(binding)
        || !matches!(name, "wrapping_mul" | "wrapping_add" | "wrapping_sub")
        || super::written_pointer_sized(written).as_deref() != Some("usize")
        || !small_literal(right)
        || !permits_edit(cx, call)
    {
        return None;
    }
    let types = cx.typeck_results();
    let ty::FnDef(id, _) = *types.expr_ty(callee).kind() else {
        return None;
    };
    let inherent = cx.tcx.inherent_impl_of_assoc(id)?;
    if cx.tcx.crate_name(id.krate).as_str() != "core"
        || cx
            .tcx
            .type_of(inherent)
            .instantiate_identity()
            .skip_norm_wip()
            != cx.tcx.types.usize
    {
        return None;
    }
    clean(cx, call.span)?;
    let receiver = clean(cx, left.span)?;
    let operand = clean(cx, right.span)?;
    Some((call.span, format!("{receiver}.{name}({operand})")))
}

pub(super) fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    current: &'tcx Expr<'tcx>,
    receiver: &Expr<'_>,
) -> Option<Vec<(Span, String)>> {
    let ExprKind::Path(path) = receiver.kind else {
        return None;
    };
    let Res::Local(binding) = cx.qpath_res(&path, receiver.hir_id) else {
        return None;
    };
    let walk = cx.tcx.hir_parent_iter(binding).find_map(|(_, node)| {
        let Node::Expr(expr) = node else { return None };
        ForLoop::hir(expr).filter(|walk| walk.pat.hir_id == binding)
    })?;
    let PatKind::Binding(BindingMode::NONE, _, ident, None) = walk.pat.kind else {
        return None;
    };
    let range = Range::hir(cx, walk.arg)?;
    if !small_literal(range.start?)
        || !small_literal(range.end?)
        || !cx.sess().edition().at_least_rust_2018()
        || clippy_utils::is_in_const_context(cx)
        || walk.pat.span.from_expansion()
        || cx.typeck_results().node_type(binding) != cx.tcx.types.usize
        || !permits_edit(cx, walk.arg)
        || cx
            .sess()
            .source_map()
            .span_to_snippet(walk.body.span)
            .ok()?
            .contains('#')
    {
        return None;
    }
    let helper = match range.limits {
        RangeLimits::HalfOpen => "indices",
        RangeLimits::Closed => "indices_inclusive",
    };
    if !boundary_conversion::available(cx, helper, 1) {
        return None;
    }
    // The lowered range constructor has a desugaring context. The endpoint
    // spans still describe the original source on both sides of its separator.
    let range_span = range.start?.span.to(range.end?.span);
    let range_source = clean(cx, range_span)?;
    let binding_source = clean(cx, ident.span)?;
    let mut edits = Vec::new();
    for_each_expr_without_closures(walk.body, |expr| {
        if let Some(edit) = operation(cx, expr, binding) {
            edits.push(edit);
        }
        ControlFlow::<(), ()>::Continue(())
    });
    // The earliest diagnostic owns the entire edit. A Rustfix caller may
    // apply suggestions independently. Keep one-use loops as they are.
    edits.sort_by_key(|(span, _)| span.lo());
    edits.dedup_by_key(|(span, _)| *span);
    if edits.len() < 2 || edits.first()?.0 != current.span {
        return None;
    }
    // The original binding is a legal identifier in this scope. Reuse it in
    // the match arm so a user constant or static cannot capture a fresh name.
    edits.push((range_span, format!(
        "match ::{CONVERSION_CRATE}::{helper}::<::core::primitive::u16>({range_source}) {{ ::core::result::Result::Ok({binding_source}) => {binding_source} }}"
    )));
    Some(edits)
}
