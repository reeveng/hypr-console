//! Follow a fixed-width loop position through the infallible native-index boundary.

use super::fixes::clean;
use clippy_utils::is_lang_item_or_ctor;
use clippy_utils::res::MaybeResPath;
use rustc_hir::{
    BindingMode, Expr, ExprKind, HirId, LangItem, MatchSource, Pat, PatKind, StmtKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{Span, def_id::CRATE_DEF_INDEX};

pub(super) struct Position<'tcx> {
    pub(super) binding: HirId,
    pub(super) input: &'tcx Expr<'tcx>,
    statement: Span,
}

fn ok_binding(cx: &LateContext<'_>, pat: &Pat<'_>) -> Option<HirId> {
    let PatKind::TupleStruct(ref path, [binding], rest) = pat.kind else {
        return None;
    };
    let PatKind::Binding(BindingMode::NONE, id, _, None) = binding.kind else {
        return None;
    };
    let constructor = cx.qpath_res(path, pat.hir_id).opt_def_id()?;
    (rest.as_opt_usize().is_none() && is_lang_item_or_ctor(cx, constructor, LangItem::ResultOk))
        .then_some(id)
}

fn boundary<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    function: &str,
) -> Option<&'tcx Expr<'tcx>> {
    let ExprKind::Call(callee, [input]) = call.kind else {
        return None;
    };
    let ty::FnDef(id, _) = *cx.typeck_results().expr_ty(callee).kind() else {
        return None;
    };
    if cx.tcx.crate_name(id.krate).as_str() != "console_core_number_conversion"
        || cx.tcx.item_name(id).as_str() != function
        || cx.tcx.opt_parent(id)?.index != CRATE_DEF_INDEX
        || clean(cx, callee.span)? != format!("::console_core_number_conversion::{function}")
    {
        return None;
    }
    Some(input)
}

impl<'tcx> Position<'tcx> {
    pub(super) fn new(cx: &LateContext<'tcx>, body: &'tcx Expr<'tcx>) -> Option<Self> {
        let ExprKind::Block(block, _) = body.kind else {
            return None;
        };
        let statement = block.stmts.first()?;
        let StmtKind::Let(local) = statement.kind else {
            return None;
        };
        if local.els.is_some() || statement.span.from_expansion() {
            return None;
        }
        let binding = ok_binding(cx, local.pat)?;
        let input = boundary(cx, local.init?, "fixed_index")?;
        cx.typeck_results()
            .expr_ty(input)
            .is_usize()
            .then_some(Self {
                binding,
                input,
                statement: statement.span,
            })
    }

    pub(super) fn original(
        &self,
        cx: &LateContext<'tcx>,
        expr: &'tcx Expr<'tcx>,
    ) -> Option<&'tcx Expr<'tcx>> {
        let ExprKind::Match(call, [arm], MatchSource::Normal) = expr.kind else {
            return None;
        };
        if expr.span.from_expansion()
            || arm.guard.is_some()
            || arm.body.res_local_id()? != ok_binding(cx, arm.pat)?
            || boundary(cx, call, "index")?.res_local_id()? != self.binding
            || !cx.typeck_results().expr_ty(expr).is_usize()
        {
            return None;
        }
        Some(self.input)
    }

    pub(super) fn standalone_statement(&self, cx: &LateContext<'_>) -> Option<Span> {
        let map = cx.sess().source_map();
        let start = map.lookup_char_pos(self.statement.lo());
        let end = map.lookup_char_pos(self.statement.hi());
        let lo = start.file.line_bounds(start.line.checked_sub(1)?).start;
        let hi = end.file.line_bounds(end.line.checked_sub(1)?).end;
        let line = self.statement.with_lo(lo).with_hi(hi);
        clean(cx, line)?;
        let before = map
            .span_to_snippet(line.with_hi(self.statement.lo()))
            .ok()?;
        let after = map
            .span_to_snippet(line.with_lo(self.statement.hi()))
            .ok()?;
        (before.trim().is_empty() && after.trim().is_empty()).then_some(line)
    }
}
