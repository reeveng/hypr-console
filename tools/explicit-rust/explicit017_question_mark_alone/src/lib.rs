#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::res::MaybeQPath;
use clippy_utils::sym;
use clippy_utils::{is_in_const_context, macros::root_macro_call_first_node, visitors};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{
    BindingMode, Expr, ExprKind, HirId, LangItem, MatchSource, Node, Pat, PatKind, UnOp,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::adjustment::{Adjust, AutoBorrow, AutoBorrowMutability, DerefAdjustKind};
use rustc_middle::ty::{self, TypeVisitableExt};
use rustc_span::Span;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

#[path = "../../shared/authored_function.rs"]
mod authored_function;
#[path = "../../shared/macro_input.rs"]
mod macro_input;
mod macro_slice;

dylint_linting::impl_late_lint! {
    /// EXPLICIT017: `?` stands alone. `frame(settle(x)?, y)` hides an early
    /// return in the middle of a line; EXPLICIT013 asks that a block that
    /// decides something be visible in the shape of the function, and a buried
    /// `?` is a decision with no shape at all. `let settled = settle(x)?;`
    /// puts the exit where a reader scanning the left margin sees it.
    ///
    /// Allowed positions are the ones a scanning eye already reads as an exit:
    /// the whole of a statement, the right-hand side of a `let`, a `return`,
    /// the tail expression of a block, and the whole body of a `match` arm.
    pub EXPLICIT017_QUESTION_MARK_ALONE,
    Deny,
    "`?` buried in an expression hides an early return; lift it into a `let` of its own",
    Explicit017QuestionMarkAlone {
        body_macros: dylint_linting::config_or_default("explicit017_body_macros"),
        first_slice_macros: dylint_linting::config_or_default("explicit017_first_slice_macros"),
        ..Default::default()
    }
}

#[derive(Default)]
pub struct Explicit017QuestionMarkAlone {
    first_slice_macros: Vec<String>,
    body_macros: Vec<String>,
    next_name: HashMap<LocalDefId, u32>,
    grouped: HashSet<HirId>,
}

fn returned(cx: &LateContext<'_>, value: &Expr<'_>) -> bool {
    let owner = cx.tcx.hir_enclosing_body_owner(value.hir_id);
    let body = cx.tcx.hir_body_owned_by(owner);
    let mut child = value.hir_id;
    for (id, node) in cx.tcx.hir_parent_iter(child) {
        if child == body.value.hir_id {
            return true;
        }
        match node {
            Node::Expr(parent) => match parent.kind {
                ExprKind::Ret(_) => return true,
                ExprKind::Block(block, _) if block.hir_id == child => {}
                ExprKind::Match(_, arms, MatchSource::Normal)
                    if arms.iter().any(|a| a.hir_id == child) => {}
                ExprKind::If(_, yes, no)
                    if yes.hir_id == child || no.is_some_and(|e| e.hir_id == child) => {}
                _ => return false,
            },
            Node::Arm(arm) if arm.body.hir_id == child => {}
            Node::Block(block) if block.expr.is_some_and(|e| e.hir_id == child) => {}
            _ => return false,
        }
        child = id;
    }
    false
}

fn return_value<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
) -> Option<Vec<(Span, String)>> {
    let Node::Expr(call) = cx.tcx.parent_hir_node(question.hir_id) else {
        return None;
    };
    let ExprKind::Call(constructor, [_]) = call.kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(..), ctor) = constructor.res(cx) else {
        return None;
    };
    let variant = cx.tcx.opt_parent(ctor)?;
    let lang = cx.tcx.lang_items();
    if Some(variant) != lang.result_ok_variant() && Some(variant) != lang.option_some_variant() {
        return None;
    }
    if !returned(cx, call) {
        return None;
    }
    let ExprKind::Call(callee, [value]) = branch.kind else {
        return None;
    };
    let ExprKind::Path(path) = callee.kind else {
        return None;
    };
    let types = cx.typeck_results();
    if !cx.tcx.qpath_is_lang_item(path, LangItem::TryTraitBranch)
        || types.expr_ty(call) != types.expr_ty(value)
        || call.span.from_expansion()
        || value.span.from_expansion()
        || !call.span.contains(value.span)
    {
        return None;
    }
    let leading = call.span.with_hi(value.span.lo());
    let trailing = call.span.with_lo(value.span.hi());
    let map = cx.sess().source_map();
    for span in [leading, trailing] {
        let text = map.span_to_snippet(span).ok()?;
        if text.contains("//") || text.contains("/*") {
            return None;
        }
    }
    Some(vec![(leading, String::new()), (trailing, String::new())])
}

fn match_infallible_position(
    cx: &LateContext<'_>,
    question: &Expr<'_>,
    branch: &Expr<'_>,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<String> {
    if question.span.from_expansion()
        || !cx.sess().edition().at_least_rust_2018()
        || !cx.tcx.hir_attrs(question.hir_id).is_empty()
    {
        return None;
    }
    let ExprKind::Call(_, [value]) = branch.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let output = types.expr_ty(value);
    let ty::Adt(result, args) = output.kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
        || !matches!(
            args.type_at(0).kind(),
            ty::Uint(ty::UintTy::Usize) | ty::Int(ty::IntTy::Isize)
        )
    {
        return None;
    }
    let empty_error = match args.type_at(1).kind() {
        ty::Never => true,
        ty::Adt(error, _) => {
            error.is_enum()
                && error.variants().is_empty()
                && !error.is_variant_list_non_exhaustive()
        }
        _ => false,
    };
    if !empty_error {
        return None;
    }
    // The signature must fix the error type independently of the propagation
    // context. Constructors and generic error parameters can rely on `?`.
    let function = clippy_utils::fn_def_id(cx, value)?;
    if matches!(cx.tcx.def_kind(function), DefKind::Ctor(..))
        || cx.tcx.fn_sig(function).skip_binder().output().skip_binder() != output
        || value.span.from_expansion()
        || !question.span.contains(value.span)
        || !macro_input::outside_macro_input(cx, question.span)
    {
        return None;
    }
    macro_slice::authored_scope(cx, question, question.span, &pass.body_macros)?;
    let map = cx.sess().source_map();
    if !map
        .span_to_snippet(question.span.with_hi(value.span.lo()))
        .ok()?
        .trim()
        .is_empty()
        || map
            .span_to_snippet(question.span.with_lo(value.span.hi()))
            .ok()?
            .trim()
            != "?"
    {
        return None;
    }
    let source = map.span_to_snippet(value.span).ok()?;
    let name = local_binding_name(cx, question, &source, pass)?;
    let indent = " ".repeat(clippy_utils::source::indent_of(cx, question.span)?);
    // Match the call in place so its temporaries and evaluation order are kept.
    let matched = format!(
        "match {source} {{\n{indent}  ::core::result::Result::Ok({name}) => {name},\n{indent}}}"
    );
    let grouped = matches!(cx.tcx.parent_hir_node(question.hir_id), Node::Expr(parent)
    if match parent.kind {
        ExprKind::Binary(_, left, _) | ExprKind::Cast(left, _) | ExprKind::Index(left, _, _)
        | ExprKind::MethodCall(_, left, _, _) => left.hir_id == question.hir_id,
        _ => false,
    });
    Some(if grouped {
        format!("({matched})")
    } else {
        matched
    })
}

fn copy_before_question<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
) -> Option<Vec<(Span, String)>> {
    let Node::Expr(deref) = cx.tcx.parent_hir_node(question.hir_id) else {
        return None;
    };
    if !matches!(deref.kind, ExprKind::Unary(UnOp::Deref, _))
        || !cx.sess().edition().at_least_rust_2018()
        || deref.span.from_expansion()
        || is_in_const_context(cx)
    {
        return None;
    }
    // A ref binding would borrow the original pointee instead of copying it.
    match cx.tcx.parent_hir_node(deref.hir_id) {
        Node::LetStmt(local)
            if matches!(
                local.pat.kind,
                PatKind::Binding(BindingMode::NONE, _, _, None)
            ) => {}
        Node::Expr(parent) if matches!(parent.kind, ExprKind::Ret(_)) => {}
        Node::Block(_) | Node::Arm(_) => {}
        _ => return None,
    }
    let ExprKind::Call(_, [value]) = branch.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let ty::Adt(adt, _) = types.expr_ty(value).kind() else {
        return None;
    };
    let method = match cx.tcx.get_diagnostic_name(adt.did()) {
        Some(sym::Option) => "::core::option::Option::<&_>::copied",
        Some(sym::Result) => "::core::result::Result::<&_, _>::copied",
        _ => return None,
    };
    let ty::Ref(_, pointee, rustc_hir::Mutability::Not) = types.expr_ty(question).kind() else {
        return None;
    };
    if value.span.from_expansion()
        || !deref.span.contains(value.span)
        || !cx.type_is_copy_modulo_regions(*pointee)
        || types.expr_ty(deref) != *pointee
    {
        return None;
    }
    let leading = deref.span.with_hi(value.span.lo());
    let trailing = deref.span.with_lo(value.span.hi());
    let map = cx.sess().source_map();
    if map.span_to_snippet(leading).ok()?.trim() != "*"
        || map.span_to_snippet(trailing).ok()?.trim() != "?"
    {
        return None;
    }
    Some(vec![
        (leading, format!("{method}(")),
        (trailing, ")?".to_owned()),
    ])
}

// An immutable by-value scalar read can be postponed without a change of
// value. A field reached through a reference, union or custom projection does
// not have that guarantee. Neither does evaluating a call or borrowing a value.
fn unchanged_operand(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let types = cx.typeck_results();
    if matches!(expr.kind, ExprKind::Lit(_)) {
        return types.expr_adjustments(expr).is_empty();
    }
    if !matches!(
        types.expr_ty(expr).kind(),
        ty::Bool | ty::Char | ty::Uint(_) | ty::Int(_) | ty::Float(_)
    ) || !types.expr_adjustments(expr).is_empty()
    {
        return false;
    }
    let mut place = expr;
    loop {
        if place.span.from_expansion() {
            return false;
        }
        match place.kind {
            // The later operand may move the entire aggregate before we
            // read its field. Copy aggregates remain available after a move.
            ExprKind::Path(ref path) => {
                let Res::Local(id) = cx.qpath_res(path, place.hir_id) else {
                    return false;
                };
                let Node::Pat(pat) = cx.tcx.hir_node(id) else {
                    return false;
                };
                return cx.type_is_copy_modulo_regions(types.expr_ty(place))
                    && matches!(pat.kind, PatKind::Binding(BindingMode::NONE, ..));
            }
            ExprKind::Field(base, _)
                if types.expr_adjustments(base).is_empty()
                    && (matches!(types.expr_ty(base).kind(), ty::Adt(adt, _) if adt.is_struct())
                        || matches!(types.expr_ty(base).kind(), ty::Tuple(_))) =>
            {
                place = base
            }
            _ => return false,
        }
    }
}

// The success value is an rvalue at the original scrutinee. A by-reference
// binding could borrow that temporary, including taking a mutable borrow.
fn binds_by_value(cx: &LateContext<'_>, pattern: &Pat<'_>) -> bool {
    let mut borrowed = false;
    pattern.walk(|pat| {
        if matches!(pat.kind, PatKind::Binding(..)) {
            let mode = cx
                .typeck_results()
                .extract_binding_mode(cx.sess(), pat.hir_id, pat.span);
            borrowed |= !matches!(mode, BindingMode::NONE | BindingMode::MUT);
        }
        !borrowed
    });
    !borrowed
}

// Follow unconditionally evaluated edges. On the right of a plain assignment
// Rust evaluates the value before the destination, even for a dynamic place.
fn first_evaluated<'tcx>(
    cx: &LateContext<'tcx>,
    child: &Expr<'_>,
    parent: &'tcx Expr<'tcx>,
) -> bool {
    let types = cx.typeck_results();
    let arguments = match parent.kind {
        ExprKind::MethodCall(_, receiver, args, _) => {
            if receiver.hir_id == child.hir_id {
                return true;
            }
            if !unchanged_operand(cx, receiver) {
                return false;
            }
            args
        }
        ExprKind::Binary(op, left, right) => {
            return left.hir_id == child.hir_id
                || right.hir_id == child.hir_id
                    && !matches!(
                        op.node,
                        rustc_hir::BinOpKind::And | rustc_hir::BinOpKind::Or
                    )
                    && unchanged_operand(cx, left);
        }
        ExprKind::Assign(_, right, _) => return right.hir_id == child.hir_id,
        ExprKind::If(condition, _, _) => return condition.hir_id == child.hir_id,
        ExprKind::Match(scrutinee, arms, MatchSource::Normal) => {
            return scrutinee.hir_id == child.hir_id
                && arms.iter().all(|arm| binds_by_value(cx, arm.pat));
        }
        ExprKind::Unary(_, operand)
        | ExprKind::Cast(operand, _)
        | ExprKind::Field(operand, _)
        | ExprKind::Ret(Some(operand)) => {
            return operand.hir_id == child.hir_id;
        }
        ExprKind::Match(branch, _, MatchSource::TryDesugar(_)) => {
            return branch.hir_id == child.hir_id;
        }
        ExprKind::Tup(arguments) | ExprKind::Array(arguments) => arguments,
        ExprKind::Call(callee, arguments)
            if matches!(callee.kind, ExprKind::Path(..))
                && matches!(types.expr_ty(callee).kind(), ty::FnDef(..)) =>
        {
            arguments
        }
        _ => return false,
    };
    arguments
        .iter()
        .take_while(|arg| arg.hir_id != child.hir_id)
        .all(|before| unchanged_operand(cx, before))
        && arguments.iter().any(|arg| arg.hir_id == child.hir_id)
}

fn existing_place(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Path(ref path) => matches!(
            cx.qpath_res(path, expr.hir_id),
            Res::Local(_) | Res::Def(DefKind::Static { .. }, _)
        ),
        ExprKind::Field(base, _) => existing_place(cx, base),
        ExprKind::Unary(UnOp::Deref, base) if cx.typeck_results().expr_ty(base).is_ref() => {
            existing_place(cx, base)
        }
        _ => false,
    }
}

fn borrows_temporary(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let autoref = cx
        .typeck_results()
        .expr_adjustments(expr)
        .iter()
        .any(|adjustment| matches!(adjustment.kind, Adjust::Borrow(_)));
    if autoref && !existing_place(cx, expr) {
        return true;
    }
    match expr.kind {
        ExprKind::AddrOf(_, _, place) => !existing_place(cx, place),
        ExprKind::Match(scrutinee, arms, MatchSource::Normal) => {
            !existing_place(cx, scrutinee) && arms.iter().any(|arm| !binds_by_value(cx, arm.pat))
        }
        ExprKind::Let(binding) => {
            !existing_place(cx, binding.init) && !binds_by_value(cx, binding.pat)
        }
        _ => false,
    }
}

fn reads_slice_element(cx: &LateContext<'_>, question: &Expr<'_>, value: &Expr<'_>) -> bool {
    let types = cx.typeck_results();
    matches!(types.expr_ty(question).kind(), ty::Ref(_, element, rustc_hir::Mutability::Not)
        if element.is_primitive())
        && matches!(cx.tcx.parent_hir_node(question.hir_id), Node::Expr(deref)
            if matches!(deref.kind, ExprKind::Unary(UnOp::Deref, _))
            && types.expr_adjustments(deref).is_empty())
        && macro_slice::checked_slice(cx, value)
}

fn value_with_scoped_temporaries<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
    stays_in_operand: bool,
) -> bool {
    let types = cx.typeck_results();
    let ExprKind::Call(_, [value]) = branch.kind else {
        return false;
    };
    let result = types.expr_ty(question);
    let shared_reference = matches!(result.kind(), ty::Ref(_, _, rustc_hir::Mutability::Not));
    // Moving a value out of an inline block keeps its drop scope at the
    // original consumer. A statement outside the operand can extend it.
    let aggregate = matches!(result.kind(), ty::Adt(..) | ty::Array(..) | ty::Tuple(..))
        && !result.has_regions()
        && (stays_in_operand || cx.type_is_copy_modulo_regions(result));
    let adjustments = types.expr_adjustments(question);
    let shared_reborrow = shared_reference
        && adjustments.iter().all(|adjustment| {
            matches!(
                adjustment.kind,
                Adjust::Deref(DerefAdjustKind::Builtin)
                    | Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not))
            )
        });
    if (!result.is_primitive() && !shared_reference && !aggregate)
        || matches!(
            result.kind(),
            ty::Uint(ty::UintTy::Usize) | ty::Int(ty::IntTy::Isize)
        )
        || (!adjustments.is_empty() && !shared_reborrow)
    {
        return false;
    }

    // Inserting a statement ends the lifetime of its operand temporaries.
    // A local/field read by the operand does not create such a temporary.
    let check_borrow = !reads_slice_element(cx, question, value);
    types.expr_ty(value).ty_adt_def().is_some_and(|adt| {
        [sym::Result, sym::Option]
            .into_iter()
            .any(|name| cx.tcx.is_diagnostic_item(name, adt.did()))
    }) && !value.span.from_expansion()
        && visitors::for_each_expr_without_closures(value, |subexpression| {
            let is_place = matches!(subexpression.kind, ExprKind::Path(ref path)
            if matches!(cx.qpath_res(path, subexpression.hir_id), Res::Local(_)))
                || matches!(subexpression.kind, ExprKind::Field(..));
            let drops_temporary = subexpression.hir_id != value.hir_id
                && !is_place
                && types
                    .expr_ty(subexpression)
                    .needs_drop(cx.tcx, cx.typing_env());

            // A returned reference, pointer, or exposed address can depend on
            // the storage of a temporary even if it has no destructor.
            let borrows_temporary = check_borrow && borrows_temporary(cx, subexpression);

            if drops_temporary || borrows_temporary {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .is_none()
}

fn lift_site<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
) -> Option<(HirId, Span)> {
    if !value_with_scoped_temporaries(cx, question, branch, false) {
        return None;
    }

    let mut root = question;
    let (site, span) = loop {
        if !cx.tcx.hir_attrs(root.hir_id).is_empty() {
            return None;
        }
        match cx.tcx.parent_hir_node(root.hir_id) {
            Node::Expr(parent) if first_evaluated(cx, root, parent) => {
                // The call and match inserted for another question mark have
                // expansion spans. All other expressions must be authored.
                if parent.span.from_expansion()
                    && !matches!(parent.kind,
                    ExprKind::Call(callee, _) if matches!(callee.kind, ExprKind::Path(path)
                        if cx.tcx.qpath_is_lang_item(path, LangItem::TryTraitBranch)))
                {
                    return None;
                }
                root = parent;
            }
            Node::LetStmt(local)
                if local.els.is_none() && cx.tcx.hir_attrs(local.hir_id).is_empty() =>
            {
                let Node::Stmt(stmt) = cx.tcx.parent_hir_node(local.hir_id) else {
                    return None;
                };
                break (stmt.hir_id, stmt.span);
            }
            Node::Stmt(stmt) => break (stmt.hir_id, stmt.span),
            Node::Block(block) if block.expr.is_some_and(|tail| tail.hir_id == root.hir_id) => {
                // The HIR for a while condition contains a synthetic block.
                // Place the statement only inside braces in the source.
                if block.span.from_expansion()
                    || root.span.from_expansion()
                    || !block.span.contains(root.span)
                    || block.span.lo() == root.span.lo()
                    || block.span.hi() == root.span.hi()
                    || matches!(cx.tcx.parent_hir_node(block.hir_id), Node::Expr(parent)
                        if matches!(parent.kind, ExprKind::Loop(_, _, rustc_hir::LoopSource::While, _)))
                {
                    return None;
                }
                let opening = block.span.with_hi(block.span.lo() + rustc_span::BytePos(1));
                if cx.sess().source_map().span_to_snippet(opening).ok()? != "{" {
                    return None;
                }
                break (root.hir_id, root.span);
            }
            _ => return None,
        }
    };
    if !cx.tcx.hir_attrs(site).is_empty()
        || !span.contains(question.span)
        || span.from_expansion()
        || question.span.from_expansion()
    {
        return None;
    }
    Some((site, span))
}

fn keep_operand_evaluation<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<String> {
    let parent = cx.tcx.parent_hir_node(question.hir_id);
    if question.span.from_expansion() || !cx.tcx.hir_attrs(question.hir_id).is_empty() {
        return None;
    }
    let checked_place = checked_dereference(cx, parent, question, branch);
    // These parents use this operand as a value. A block in the same slot
    // keeps the operand after any preceding effects or short-circuit test.
    let operand = match parent {
        Node::Expr(parent) if !parent.span.from_expansion() => match parent.kind {
            ExprKind::Binary(_, left, right) => {
                left.hir_id == question.hir_id || right.hir_id == question.hir_id
            }
            ExprKind::MethodCall(_, receiver, args, _) => {
                receiver.hir_id == question.hir_id
                    || args.iter().any(|arg| arg.hir_id == question.hir_id)
            }
            ExprKind::Call(_, args) | ExprKind::Tup(args) | ExprKind::Array(args) => {
                args.iter().any(|arg| arg.hir_id == question.hir_id)
            }
            // A let condition can sit behind a short-circuit or in a loop.
            // Keep the new statement in its initializer, where the condition
            // was evaluated, and reject patterns that borrow its temporary.
            ExprKind::Let(binding) => {
                binding.init.hir_id == question.hir_id && binds_by_value(cx, binding.pat)
            }
            ExprKind::Match(scrutinee, arms, MatchSource::Normal) => {
                scrutinee.hir_id == question.hir_id
                    && arms.iter().all(|arm| binds_by_value(cx, arm.pat))
            }
            // Built-in and overloaded compound assignments can evaluate the
            // place and value in different orders. Keep the binding on the RHS.
            ExprKind::Assign(_, right, _) | ExprKind::AssignOp(_, _, right) => {
                right.hir_id == question.hir_id
            }
            _ => false,
        },
        Node::ExprField(field) => {
            !field.span.from_expansion() && field.expr.hir_id == question.hir_id
        }
        _ => false,
    };
    let in_macro = matches_argument(cx, parent, question);
    if !(in_macro || operand || checked_place.is_some()) {
        return None;
    }
    if (!in_macro && !macro_input::outside_macro_input(cx, question.span))
        || (checked_place.is_none() && !value_with_scoped_temporaries(cx, question, branch, true))
    {
        return None;
    }
    let map = cx.sess().source_map();
    let source = map.span_to_snippet(question.span).ok()?;
    if source.contains("//") || source.contains("/*") {
        return None;
    }
    let source = if let Some(value) = checked_place {
        // HIR can include the parentheses around a dereferenced `?`.
        // Preserve them on the operand only when they precede the `?`.
        let prefix = map
            .span_to_snippet(question.span.with_hi(value.span.lo()))
            .ok()?;
        let suffix = map
            .span_to_snippet(question.span.with_lo(value.span.hi()))
            .ok()?;
        let open = prefix.chars().filter(|c| !c.is_whitespace());
        let mut close = suffix.chars().filter(|c| !c.is_whitespace());
        if !open.clone().all(|c| c == '(')
            || close.next() != Some('?')
            || !close.clone().all(|c| c == ')')
            || open.count() != close.count()
        {
            return None;
        }
        format!("{}?", map.span_to_snippet(value.span).ok()?)
    } else {
        source.ends_with('?').then_some(source)?
    };
    let owner = cx.tcx.hir_enclosing_body_owner(question.hir_id);
    if checked_place.is_some() {
        macro_slice::authored_scope(cx, question, question.span, &pass.body_macros)?;
    } else if cx
        .tcx
        .hir_span(cx.tcx.local_def_id_to_hir_id(owner))
        .from_expansion()
    {
        return None;
    }
    let binding = local_binding_name(cx, question, &source, pass)?;
    let block = format!("{{ let {binding} = {source}; {binding} }}");
    // At the start of a statement or match arm, a bare block can end the
    // expression before the binary operator.
    if let Node::Expr(parent) = parent
        && let ExprKind::Binary(_, left, _) = parent.kind
        && left.hir_id == question.hir_id
    {
        Some(format!("({block})"))
    } else {
        Some(block)
    }
}

fn checked_dereference<'tcx>(
    cx: &LateContext<'tcx>,
    parent: Node<'tcx>,
    question: &Expr<'_>,
    branch: &Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let Node::Expr(parent) = parent else {
        return None;
    };
    let ExprKind::Call(_, [value]) = branch.kind else {
        return None;
    };
    let types = cx.typeck_results();
    // A fresh reference from get/get_mut can be bound just before the
    // original dereference. In particular, a write still evaluates the
    // lookup after its RHS and borrows the same element.
    (matches!(parent.kind, ExprKind::Unary(UnOp::Deref, operand) if operand.hir_id == question.hir_id)
    && !parent.span.from_expansion()
    && question.span.contains(value.span)
    && types.expr_ty(question).is_ref()
    && types.expr_adjustments(question).is_empty()
    && macro_slice::checked_slice(cx, value))
    .then_some(value)
}

fn matches_argument<'tcx>(cx: &LateContext<'tcx>, parent: Node<'tcx>, question: &Expr<'_>) -> bool {
    let Node::Expr(parent) = parent else {
        return false;
    };
    let ExprKind::Match(scrutinee, [matched, _], MatchSource::Normal) = parent.kind else {
        return false;
    };
    let Some(call) = root_macro_call_first_node(cx, parent) else {
        return false;
    };
    // The standard macro evaluates this input once, as the scrutinee. Keep
    // both the pattern and any guard in place. Do not edit a forwarded input.
    if scrutinee.hir_id != question.hir_id
        || !cx.tcx.is_diagnostic_item(sym::matches_macro, call.def_id)
        || !call.span.contains(question.span)
        || !macro_input::outside_macro_input(cx, call.span)
        || !binds_by_value(cx, matched.pat)
        || !cx.typeck_results().expr_adjustments(question).is_empty()
    {
        return false;
    }
    let map = cx.sess().source_map();
    map.span_to_snippet(call.span.with_hi(question.span.lo()))
        .is_ok_and(|prefix| {
            prefix
                .split_once('!')
                .is_some_and(|(_, opening)| matches!(opening.trim(), "(" | "[" | "{"))
        })
        && map
            .span_to_snippet(call.span.with_lo(question.span.hi()))
            .is_ok_and(|suffix| suffix.trim_start().starts_with(','))
}

fn binding_name(
    next_name: &mut HashMap<LocalDefId, u32>,
    owner: LocalDefId,
    function: &str,
    body: &str,
) -> Option<String> {
    let next = next_name.entry(owner).or_insert(1);
    loop {
        let suffix = *next;
        *next = next.checked_add(1)?;
        let name = if suffix == 1 {
            "checked_value".to_owned()
        } else {
            format!("checked_value_{suffix}")
        };
        if !function.contains(&name) && !body.contains(&name) {
            return Some(name);
        }
    }
}

fn local_binding_name(
    cx: &LateContext<'_>,
    question: &Expr<'_>,
    source: &str,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<String> {
    let owner = cx.tcx.hir_enclosing_body_owner(question.hir_id);
    let file = cx
        .sess()
        .source_map()
        .lookup_source_file(question.span.lo());
    let contents = file.src.as_ref()?;
    let module = cx.tcx.parent_module(question.hir_id).to_local_def_id();
    loop {
        let name = binding_name(&mut pass.next_name, owner, contents, source)?;
        if !cx
            .tcx
            .module_children_local(module)
            .iter()
            .any(|child| child.ident.as_str() == name)
        {
            return Some(name);
        }
    }
}

fn lift_argument<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<Vec<(Span, String)>> {
    let (site, span) = lift_site(cx, question, branch)?;
    let map = cx.sess().source_map();
    let owner = cx.tcx.hir_enclosing_body_owner(question.hir_id);
    let function_span = cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner));
    let body_span = if function_span.from_expansion() {
        macro_slice::authored_body(cx, question, span, &pass.body_macros)?
    } else {
        cx.tcx.hir_body_owned_by(owner).value.span
    };
    if !body_span.contains(span) {
        return None;
    }
    let function = map.span_to_snippet(function_span).ok()?;
    let body = map.span_to_snippet(body_span).ok()?;
    let module = cx.tcx.parent_module(question.hir_id).to_local_def_id();
    let mut chain = vec![question];
    visitors::for_each_expr_without_closures(branch, |inner| {
        if let ExprKind::Match(branch, [residual, ..], MatchSource::TryDesugar(_)) = inner.kind
            && matches!(residual.body.kind, ExprKind::Ret(_))
            && lift_site(cx, inner, branch).is_some_and(|(inner_site, _)| inner_site == site)
        {
            chain.push(inner);
        }
        ControlFlow::<(), ()>::Continue(())
    });
    let indent = clippy_utils::source::indent_of(cx, span).map_or_else(
        || " ".to_owned(),
        |indent| format!("\n{}", " ".repeat(indent)),
    );
    let mut statements = String::new();
    let mut previous: Option<(Span, String)> = None;
    for step in chain.iter().rev() {
        let mut source = map.span_to_snippet(step.span).ok()?;
        if source.contains("//") || source.contains("/*") {
            return None;
        }
        if chain.len() == 1
            && let ExprKind::Call(_, [value]) = branch.kind
            && reads_slice_element(cx, step, value)
        {
            source = format!("{}?", map.span_to_snippet(value.span).ok()?);
        }
        if let Some((span, name)) = previous {
            if !step.span.contains(span) {
                return None;
            }
            let start = (span.lo() - step.span.lo()).0 as usize;
            let end = (span.hi() - step.span.lo()).0 as usize;
            source.get(start..end)?;
            source.replace_range(start..end, &name);
        }
        let binding = loop {
            let name = binding_name(&mut pass.next_name, owner, &function, &body)?;
            if !cx
                .tcx
                .module_children_local(module)
                .iter()
                .any(|child| child.ident.as_str() == name)
            {
                break name;
            }
        };
        statements.push_str(&format!("let {binding} = {source};{indent}"));
        previous = Some((step.span, binding));
    }
    let (_, binding) = previous?;
    pass.grouped
        .extend(chain.iter().skip(1).map(|step| step.hir_id));
    if span.lo() == question.span.lo() {
        statements.push_str(&binding);
        return Some(vec![(question.span, statements)]);
    }
    Some(vec![
        (span.shrink_to_lo(), statements),
        (question.span, binding),
    ])
}

impl<'tcx> LateLintPass<'tcx> for Explicit017QuestionMarkAlone {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // `?` lowers to a match the compiler marks as its own.
        let ExprKind::Match(branch, arms, MatchSource::TryDesugar(_)) = expr.kind else {
            return;
        };

        let stands_alone = match cx.tcx.parent_hir_node(expr.hir_id) {
            Node::Stmt(_) => true,
            Node::LetStmt(_) => true,
            Node::Block(_) => true,
            Node::Arm(_) => true,
            Node::Expr(parent) => matches!(parent.kind, ExprKind::Ret(_)),
            _ => false,
        };

        if stands_alone {
            return;
        }

        span_lint_and_then(
            cx,
            EXPLICIT017_QUESTION_MARK_ALONE,
            expr.span,
            "this `?` is buried in a larger expression: an early return with no shape on the screen",
            |diagnostic| {
                if self.grouped.remove(&expr.hir_id) {
                    diagnostic
                        .help("lift the inner and outer operations together in evaluation order");
                } else if matches!(arms, [residual, ..] if matches!(residual.body.kind, ExprKind::Ret(_)))
                    && let Some(edits) = return_value(cx, expr, branch)
                {
                    diagnostic.multipart_suggestion(
                        "return the value directly with the same success and failure types",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some(replacement) = match_infallible_position(cx, expr, branch, self)
                {
                    diagnostic.span_suggestion(
                        expr.span,
                        "match the only possible result variant at this operand",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some(edits) = copy_before_question(cx, expr, branch) {
                    diagnostic.multipart_suggestion(
                        "copy the value before applying the question mark",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if matches!(arms, [residual, ..] if matches!(residual.body.kind, ExprKind::Ret(_)))
                    && let Some(edits) = lift_argument(cx, expr, branch, self)
                {
                    diagnostic.multipart_suggestion(
                        "evaluate the fallible value in its own statement",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if matches!(arms, [residual, ..] if matches!(residual.body.kind, ExprKind::Ret(_)))
                    && let Some(edits) = macro_slice::lift(cx, expr, branch, self)
                {
                    diagnostic.multipart_suggestion(
                        "bind the checked slice directly",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if matches!(arms, [residual, ..] if matches!(residual.body.kind, ExprKind::Ret(_)))
                    && let Some(replacement) = keep_operand_evaluation(cx, expr, branch, self)
                {
                    diagnostic.span_suggestion(
                        expr.span,
                        "bind the fallible operand where it is evaluated",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("lift it out -- `let name = …?;` -- so the exit sits on the left margin where it can be seen");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn let_inputs() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui_let_inputs");
}

#[test]
fn matches_and_fields() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui_matches_and_fields");
}

#[test]
fn infallible_positions() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui_infallible_positions");
}

#[test]
fn first_slice_macros() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_macros")
        .dylint_toml(r#"explicit017_first_slice_macros = ["slice_macro::load", "slice_macro::store", "slice_macro::load_array", "slice_macro::store_array", "slice_review::load"]"#)
        .run();
}
