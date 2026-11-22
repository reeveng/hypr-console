//! Fixed-size replacements for vectors whose uses work on a slice or array.

use clippy_utils::consts::{ConstEvalCtxt, Constant};
use clippy_utils::get_parent_expr;
use clippy_utils::macros::{macro_backtrace, root_macro_call_first_node};
use clippy_utils::span_contains_comment;
use clippy_utils::sym;
use clippy_utils::visitors::for_each_local_use_after_expr;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{
    BorrowKind, Expr, ExprKind, GenericArg, LetStmt, Node, Pat, PatKind, QPath, TyKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, layout::LayoutOf};
use rustc_span::{DesugaringKind, Span};
use std::ops::ControlFlow;

// Match the default limit of Clippy's useless_vec lint when moving values
// onto the stack. Larger arrays need storage chosen for the calling code.
const STACK_BYTES: u64 = 200;

fn borrowed_as_slice(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let Some(parent) = get_parent_expr(cx, expr) else {
        return false;
    };
    if parent.span.from_expansion() {
        return false;
    }

    match parent.kind {
        ExprKind::AddrOf(BorrowKind::Ref, _, inner) if inner.hir_id == expr.hir_id => {
            matches!(cx.typeck_results().expr_ty_adjusted(parent).kind(), ty::Ref(_, inner, _) if inner.is_slice())
        }
        ExprKind::MethodCall(_, receiver, [], _) if receiver.hir_id == expr.hir_id => {
            let method = cx.typeck_results().type_dependent_def_id(parent.hir_id);
            matches!(
                method.and_then(|id| cx.tcx.get_diagnostic_name(id)),
                Some(sym::vec_as_slice | sym::vec_as_mut_slice)
            )
        }
        _ => false,
    }
}

fn array_works<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    match cx.tcx.parent_hir_node(expr.hir_id) {
        Node::LetStmt(LetStmt {
            ty: None,
            pat:
                Pat {
                    kind: PatKind::Binding(_, local, _, None),
                    ..
                },
            ..
        }) => for_each_local_use_after_expr(cx, *local, expr.hir_id, |reference| {
            if borrowed_as_slice(cx, reference) {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        })
        .is_continue(),
        Node::Expr(parent) if parent.span.is_desugaring(DesugaringKind::ForLoop) => {
            matches!(parent.kind, ExprKind::Call(_, [input]) if input.hir_id == expr.hir_id)
        }
        _ => borrowed_as_slice(cx, expr),
    }
}

fn empty_array(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let ExprKind::Call(callee, []) = expr.kind else {
        return None;
    };
    let ExprKind::Path(QPath::TypeRelative(receiver, _)) = &callee.kind else {
        return None;
    };
    let TyKind::Path(QPath::Resolved(None, path)) = receiver.kind else {
        return None;
    };
    let Res::Def(_, definition) = path.res else {
        return None;
    };
    // A type alias may supply the element type even without a written argument.
    if !cx.tcx.is_diagnostic_item(sym::Vec, definition) {
        return None;
    }

    match path.segments.last()?.args().args {
        [] | [GenericArg::Infer(_)] => Some("[]".to_owned()),
        [GenericArg::Type(written)] => {
            let element = cx.sess().source_map().span_to_snippet(written.span).ok()?;
            Some(format!("::core::convert::identity::<[{element}; 0]>([])"))
        }
        _ => None,
    }
}

fn repeat_count(cx: &LateContext<'_>, count: &Expr<'_>) -> Option<u64> {
    // A run-time value is valid for vec! but cannot be an array length.
    match count.kind {
        ExprKind::Lit(_) => {}
        ExprKind::Path(ref path)
            if matches!(
                cx.qpath_res(path, count.hir_id),
                Res::Def(DefKind::Const { .. }, _)
            ) => {}
        _ => return None,
    }
    match ConstEvalCtxt::new(cx).eval(count)? {
        Constant::Int(value) => u64::try_from(value).ok(),
        _ => None,
    }
}

fn literal_array(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    called: DefId,
    span: Span,
) -> Option<String> {
    let ExprKind::Call(_, args) = expr.kind else {
        return None;
    };
    let ty::Adt(_, generics) = cx.typeck_results().expr_ty(expr).kind() else {
        return None;
    };
    let element = generics.type_at(0);
    let count = match (cx.tcx.get_diagnostic_name(called)?, args) {
        (sym::vec_new, []) => 0,
        (sym::box_assume_init_into_vec_unsafe, [initializer])
            if let ExprKind::Call(_, [_, elements]) = initializer.kind
                && let ExprKind::Array(elements) = elements.kind =>
        {
            u64::try_from(elements.len()).ok()?
        }
        // An array repeat copies its operand. Restrict this rewrite to
        // primitives, where Vec's Clone cannot run user code.
        (sym::vec_from_elem, [_, count]) if element.is_primitive() => repeat_count(cx, count)?,
        _ => return None,
    };
    if count != 0
        && cx
            .layout_of(element)
            .ok()?
            .size
            .bytes()
            .checked_mul(count)?
            > STACK_BYTES
    {
        return None;
    }

    let source = cx.sess().source_map().span_to_snippet(span).ok()?;
    let (_, arguments) = source.split_once('!')?;
    let args = arguments.trim();
    let contents = match args.as_bytes().first()? {
        b'[' => args.strip_prefix('[')?.strip_suffix(']')?,
        b'(' => args.strip_prefix('(')?.strip_suffix(')')?,
        b'{' => args.strip_prefix('{')?.strip_suffix('}')?,
        _ => return None,
    };
    Some(format!("[{contents}]"))
}

pub fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    called: DefId,
) -> Option<(Span, String)> {
    // An enclosing macro can expand the same written expression into contexts
    // with different type requirements.
    if cx
        .tcx
        .hir_parent_iter(expr.hir_id)
        .any(|(id, _)| macro_backtrace(cx.tcx.hir_span(id)).next().is_some())
    {
        return None;
    }
    let (span, array) = if expr.span.from_expansion() {
        let invocation = root_macro_call_first_node(cx, expr)?;
        if !cx.tcx.is_diagnostic_item(sym::vec_macro, invocation.def_id) {
            return None;
        }
        (
            invocation.span,
            literal_array(cx, expr, called, invocation.span)?,
        )
    } else if cx.tcx.is_diagnostic_item(sym::vec_new, called) {
        (expr.span, empty_array(cx, expr)?)
    } else {
        return None;
    };

    (array_works(cx, expr) && !span_contains_comment(cx, span)).then_some((span, array))
}
