#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::higher::WhileLet;
use clippy_utils::res::{MaybeDef, MaybeResPath, MaybeTypeckRes};
use clippy_utils::{as_some_pattern, is_in_const_context, is_refutable, sym, visitors};
use rustc_errors::Applicability;
use rustc_hir::{BlockCheckMode, Expr, ExprKind, LoopSource, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::adjustment::{Adjust, AutoBorrow, AutoBorrowMutability, DerefAdjustKind};
use rustc_middle::ty::{self, TypeVisitableExt};
use rustc_span::Span;
use std::ops::ControlFlow;

mod counted;
#[path = "../../shared/macro_input.rs"]
mod macro_input;

dylint_linting::declare_late_lint! {
    /// EXPLICIT053: `while` is forbidden. A `while` is an `if` at the top of a
    /// loop, and its false answer is the loop ending, which is written nowhere:
    /// the reader learns where the walk stops by working out when the
    /// condition fails. EXPLICIT019 took the `if` and EXPLICIT023 the `else`
    /// that names nothing; this is the same outcome without a name, moved to
    /// the head of a loop.
    ///
    /// What the loop walks is usually a list or an iterator, and then it is a
    /// `for`, or an iterator word -- `take_while`, `successors`, `from_fn` --
    /// that says what is walked and where it stops. A `loop` is not the
    /// answer either: EXPLICIT054 takes it, for the same reason. `while let`
    /// is the same question about a pattern, and the same answer: a `for`
    /// over what the pattern was taking apart.
    pub EXPLICIT053_NO_WHILE,
    Deny,
    "`while` is forbidden; a loop names where it stops"
}

fn iterator_loop<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> Option<(Span, String)> {
    let walk = WhileLet::hir(expr)?;
    let [item] = as_some_pattern(cx, walk.let_pat)? else {
        return None;
    };
    let returned = walk.let_expr;
    let call = counted::successful_operand(cx, returned).unwrap_or(returned);
    let ExprKind::MethodCall(name, receiver, arguments, _) = call.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let direct_iterator = call.hir_id == returned.hir_id
        && arguments.is_empty()
        && name.ident.name == sym::next
        && cx
            .ty_based_def(call)
            .opt_parent(cx)
            .is_diag_item(cx, sym::Iterator);
    if is_refutable(cx, item) {
        return None;
    }
    let mut argument_roots = Vec::new();
    if !direct_iterator {
        let ty::Adt(_, item_type) = types.expr_ty(returned).kind() else {
            return None;
        };
        // An item borrowed from the receiver may leave the while condition
        // yet cannot escape through a closure that retains that receiver.
        if !cx.sess().edition().at_least_rust_2021()
            || item_type.type_at(0).has_regions()
        {
            return None;
        }
        for argument in arguments {
            if !cx.type_is_copy_modulo_regions(types.expr_ty(argument))
                || argument.span.from_expansion()
                || !types.expr_adjustments(argument).is_empty()
            {
                return None;
            }
            if !matches!(argument.kind, ExprKind::Lit(_)) {
                argument_roots.push(counted::bound_place(cx, argument)?);
            }
        }
    }
    let (binding, iterator_field) = if let ExprKind::Field(base, field) = receiver.kind {
        let base_type = types.expr_ty_adjusted(base).peel_refs();
        // A field borrow can coexist with uses of the other fields. Older
        // editions may capture the entire enclosing local in a closure.
        if !cx.sess().edition().at_least_rust_2021()
            || !matches!(base_type.kind(), ty::Tuple(_) | ty::Adt(..))
            || base_type.ty_adt_def().is_some_and(|adt| !adt.is_struct())
            || !types.expr_adjustments(base).iter().all(|adjustment| {
                matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin))
            })
        {
            return None;
        }
        (base.res_local_id()?, Some(field.name))
    } else {
        (receiver.res_local_id()?, None)
    };
    if visitors::for_each_expr(cx, walk.if_then, |expr| {
        let aliases_iterator = expr.res_local_id() == Some(binding)
            && !matches!(
                (iterator_field, cx.tcx.parent_hir_node(expr.hir_id)),
                (Some(iter), Node::Expr(parent))
                    if matches!(parent.kind, ExprKind::Field(base, field)
                        if base.hir_id == expr.hir_id && field.name != iter)
            );
        let is_unsafe = matches!(
            expr.kind,
            ExprKind::Block(block, _) if matches!(block.rules, BlockCheckMode::UnsafeBlock(_))
        );
        let uses_argument = expr
            .res_local_id()
            .is_some_and(|id| argument_roots.contains(&id));
        if aliases_iterator || uses_argument || is_unsafe {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .is_some()
    {
        return None;
    }

    let map = cx.sess().source_map();
    let header = expr.span.with_hi(walk.let_expr.span.hi());
    let original = map.span_to_snippet(header).ok()?;
    let body = map.span_to_snippet(walk.if_then.span).ok()?;
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    let signature = map
        .span_to_snippet(cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner)))
        .ok()?;
    if header.from_expansion()
        || call.span.from_expansion()
        || item.span.from_expansion()
        || receiver.span.from_expansion()
        || original.contains("//")
        || original.contains("/*")
        || original.contains('#')
        || body.contains('#')
        || signature.contains("unsafe")
    {
        return None;
    }
    let label = if let Some(label) = walk.label {
        let prefix = map
            .span_to_snippet(header.with_hi(label.ident.span.hi()))
            .ok()?;
        format!("{prefix}: ")
    } else {
        String::new()
    };
    let item = map.span_to_snippet(item.span).ok()?;
    let mut dereferences = String::new();
    let (borrow, deref) = types.expr_adjustments(receiver).split_last()?;
    if !matches!(borrow.kind, Adjust::Borrow(AutoBorrow::Ref(_))) {
        return None;
    }
    for adjustment in deref {
        if !matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin)) {
            return None;
        }
        dereferences.push('*');
    }
    let iterator = if direct_iterator {
        if !matches!(
            borrow.kind,
            Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut { .. }))
        ) {
            return None;
        }
        let received = map.span_to_snippet(receiver.span).ok()?;
        format!("&mut {dereferences}{received}")
    } else {
        let next = map.span_to_snippet(returned.span).ok()?;
        format!("::core::iter::from_fn(|| {next})")
    };
    Some((header, format!("{label}for {item} in {iterator}")))
}

impl<'tcx> LateLintPass<'tcx> for Explicit053NoWhile {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Loop(_, _, LoopSource::While, _) = expr.kind else {
            return;
        };

        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        span_lint_and_then(
            cx,
            EXPLICIT053_NO_WHILE,
            expr.span,
            "`while` ends the loop on a false answer that has no name here",
            |diagnostic| {
                let can_fix = cx.sess().edition().at_least_rust_2018()
                    && !is_in_const_context(cx)
                    && macro_input::outside_macro_input(cx, expr.span);
                if can_fix && let Some((header, replacement)) = iterator_loop(cx, expr) {
                    diagnostic.span_suggestion(
                        header,
                        "iterate until this iterator returns no more items",
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else if can_fix && let Some(edits) = counted::counted_loop(cx, expr) {
                    diagnostic.multipart_suggestion(
                        "iterate over the remaining positions and preserve the counter on exit",
                        edits,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("walk it with a `for` over the list or the source of events, or an iterator word that says where it stops");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}

#[test]
fn fixes() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
