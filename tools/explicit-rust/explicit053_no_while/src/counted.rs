//! Compiler-proven range rewrites for loops with a stable integer bound.

use clippy_utils::higher::While;
use clippy_utils::res::MaybeResPath;
use clippy_utils::usage::mutated_variables;
use clippy_utils::{sym, visitors};
use rustc_ast::LitKind;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::{
    BinOpKind, BindingMode, BlockCheckMode, BorrowKind, Expr, ExprKind, HirId, LangItem, LetStmt,
    MatchSource, Mutability, Pat, PatKind, Stmt, StmtKind, UnOp,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::adjustment::{Adjust, DerefAdjustKind};
use rustc_middle::ty::{self, Ty};
use rustc_span::{BytePos, Span};
use std::ops::ControlFlow;

fn binary_call<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(DefId, &'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
    match expr.kind {
        ExprKind::MethodCall(_, left, [right], _) => {
            let method = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;
            Some((method, left, right))
        }
        ExprKind::Call(callee, [left, right]) => {
            let ExprKind::Path(ref path) = callee.kind else {
                return None;
            };
            let function = cx.qpath_res(path, callee.hir_id).opt_def_id()?;
            Some((function, left, right))
        }
        _ => None,
    }
}

fn integer_method<'tcx>(cx: &LateContext<'tcx>, id: DefId, ty: Ty<'tcx>) -> bool {
    cx.tcx
        .inherent_impl_of_assoc(id)
        .is_some_and(|implementation| {
            cx.tcx
                .type_of(implementation)
                .instantiate_identity()
                .skip_norm_wip()
                == ty
        })
}

fn wrapping_operand<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let ExprKind::Call(callee, [value]) = expr.kind else {
        return None;
    };
    let ExprKind::Path(ref path) = callee.kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(..), constructor) = cx.qpath_res(path, callee.hir_id) else {
        return None;
    };
    cx.tcx
        .is_diagnostic_item(sym::Wrapping, cx.tcx.parent(constructor))
        .then_some(value)
}

fn wrapping_binary<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(BinOpKind, &'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
    let ExprKind::Field(binary, field) = expr.kind else {
        return None;
    };
    let ExprKind::Binary(op, left, right) = binary.kind else {
        return None;
    };
    if field.name.as_str() != "0" {
        return None;
    }
    Some((
        op.node,
        wrapping_operand(cx, left)?,
        wrapping_operand(cx, right)?,
    ))
}

pub(super) fn bound_place(cx: &LateContext<'_>, place: &Expr<'_>) -> Option<HirId> {
    if place.span.from_expansion() {
        return None;
    }
    match place.kind {
        ExprKind::Path(_) => place.res_local_id(),
        ExprKind::Field(base, _) => {
            let types = cx.typeck_results();
            let container = types.expr_ty_adjusted(base).peel_refs();
            if !matches!(container.kind(), ty::Tuple(_))
                && !container.ty_adt_def().is_some_and(|adt| adt.is_struct())
            {
                return None;
            }
            if !types.expr_adjustments(base).iter().all(|adjustment| {
                matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin))
            }) {
                return None;
            }
            bound_place(cx, base)
        }
        _ => None,
    }
}

fn stable_length(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    receiver: &Expr<'_>,
    locals: &mut Vec<HirId>,
) -> Option<()> {
    let types = cx.typeck_results();
    let method = types.type_dependent_def_id(expression.hir_id)?;
    let received = types.expr_ty(receiver).peel_refs();
    let root = bound_place(cx, receiver)?;
    if cx.tcx.lang_items().slice_len_fn() == Some(method) {
        match received.kind() {
            // A fixed array has the same length even if the body replaces
            // the array or writes to its elements.
            ty::Array(..) => return Some(()),
            ty::Slice(..) => {}
            _ => return None,
        }
    } else {
        let implementation = cx.tcx.inherent_impl_of_assoc(method)?;
        let owner = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .skip_norm_wip();
        if cx.tcx.item_name(method).as_str() != "len"
            || !received
                .ty_adt_def()
                .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Vec, adt.did()))
            || owner.ty_adt_def() != received.ty_adt_def()
        {
            return None;
        }
    }
    // A slice reference or Vec can be replaced. Retain the existing
    // mutation proof for their owning local.
    locals.push(root);
    Some(())
}

fn carried_binding(cx: &LateContext<'_>, pattern: &Pat<'_>) -> Option<HirId> {
    let PatKind::TupleStruct(ref path, [field], rest) = pattern.kind else {
        return None;
    };
    let constructor = cx.qpath_res(path, pattern.hir_id).opt_def_id()?;
    if rest.as_opt_usize().is_some()
        || !clippy_utils::is_lang_item_or_ctor(cx, constructor, LangItem::ResultOk)
    {
        return None;
    }
    match field.kind {
        PatKind::Binding(BindingMode::NONE, id, _, None) => Some(id),
        _ => None,
    }
}

fn conversion_argument<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let ExprKind::Call(callee, [argument]) = expression.kind else {
        return None;
    };
    let ExprKind::Path(ref path) = callee.kind else {
        return None;
    };
    let function = cx.qpath_res(path, callee.hir_id).opt_def_id()?;
    if expression.span.from_expansion()
        || function.is_local()
        || cx.tcx.crate_name(function.krate).as_str() != "console_core_number_conversion"
    {
        return None;
    }
    let module = cx.tcx.parent(function);
    let stable = match cx.tcx.item_name(function).as_str() {
        "index" | "fixed_index" | "fixed_offset" => module.is_crate_root(),
        "wrapped_index" | "wrapped_offset" => cx.tcx.item_name(module).as_str() == "integer_bits",
        _ => false,
    };
    stable.then_some(argument)
}

pub(super) fn successful_operand<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let (pattern, result, tail) = match expression.kind {
        ExprKind::Match(result, [arm], MatchSource::Normal) if arm.guard.is_none() => {
            (arm.pat, result, arm.body)
        }
        ExprKind::Block(block, None) if matches!(block.rules, BlockCheckMode::DefaultBlock) => {
            let [statement] = block.stmts else {
                return None;
            };
            let StmtKind::Let(binding) = statement.kind else {
                return None;
            };
            if binding.els.is_some() {
                return None;
            }
            (binding.pat, binding.init?, block.expr?)
        }
        _ => return None,
    };
    if pattern.span.from_expansion() || tail.res_local_id() != Some(carried_binding(cx, pattern)?)
    {
        return None;
    }
    Some(result)
}

// A range reads its end once. Only accept expressions made of integer values
// and known integer operations, and track each local whose value must stay put.
fn stable_bound<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    locals: &mut Vec<HirId>,
) -> Option<()> {
    let types = cx.typeck_results();
    let ty = types.expr_ty(expr);
    if !ty.is_integral() || expr.span.from_expansion() {
        return None;
    }
    let check_both = |left, right, locals: &mut Vec<HirId>| {
        stable_bound(cx, left, locals)?;
        stable_bound(cx, right, locals)
    };
    match expr.kind {
        ExprKind::Lit(_) => Some(()),
        ExprKind::Path(ref path) => match cx.qpath_res(path, expr.hir_id) {
            Res::Local(local) => {
                locals.push(local);
                Some(())
            }
            Res::Def(
                DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::ConstParam,
                _,
            ) => Some(()),
            _ => None,
        },
        ExprKind::Unary(UnOp::Neg | UnOp::Not, value) => stable_bound(cx, value, locals),
        ExprKind::Binary(op, left, right) if integer_operator(op.node) => {
            check_both(left, right, locals)
        }
        ExprKind::Field(..) => {
            if let Some((op, left, right)) = wrapping_binary(cx, expr) {
                return integer_operator(op).then(|| check_both(left, right, locals))?;
            }
            locals.push(bound_place(cx, expr)?);
            Some(())
        }
        ExprKind::MethodCall(_, receiver, [], _) => stable_length(cx, expr, receiver, locals),
        ExprKind::Match(..) | ExprKind::Block(..) => {
            let result = successful_operand(cx, expr)?;
            stable_bound(cx, conversion_argument(cx, result)?, locals)
        }
        ExprKind::MethodCall(..) | ExprKind::Call(..) => {
            let (id, left, right) = binary_call(cx, expr)?;
            if cx.tcx.is_diagnostic_item(sym::cmp_min, id)
                || cx.tcx.is_diagnostic_item(sym::cmp_max, id)
                || (integer_method(cx, id, ty)
                    && matches!(
                        cx.tcx.item_name(id).as_str(),
                        "wrapping_add"
                            | "wrapping_sub"
                            | "wrapping_mul"
                            | "wrapping_shl"
                            | "wrapping_shr"
                            | "saturating_add"
                            | "saturating_sub"
                            | "saturating_mul"
                            | "min"
                            | "max"
                    ))
            {
                check_both(left, right, locals)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn integer_operator(op: BinOpKind) -> bool {
    matches!(
        op,
        BinOpKind::Add
            | BinOpKind::Sub
            | BinOpKind::Mul
            | BinOpKind::Div
            | BinOpKind::Rem
            | BinOpKind::BitXor
            | BinOpKind::BitAnd
            | BinOpKind::BitOr
            | BinOpKind::Shl
            | BinOpKind::Shr
    )
}

fn literal_one(expr: &Expr<'_>) -> bool {
    !expr.span.from_expansion()
        && matches!(expr.kind, ExprKind::Lit(literal) if matches!(literal.node, LitKind::Int(one, _) if one.get() == 1))
}

fn integer_update<'tcx>(
    cx: &LateContext<'tcx>,
    id: DefId,
    ty: Ty<'tcx>,
    direction: BinOpKind,
) -> bool {
    let name = cx.tcx.item_name(id);
    let matches_direction = match direction {
        BinOpKind::Add => matches!(name.as_str(), "wrapping_add" | "saturating_add"),
        BinOpKind::Sub => matches!(name.as_str(), "wrapping_sub" | "saturating_sub"),
        _ => false,
    };
    matches_direction && integer_method(cx, id, ty)
}

fn immutable_binding<'tcx>(statement: &Stmt<'tcx>) -> Option<(HirId, &'tcx Expr<'tcx>)> {
    let StmtKind::Let(binding) = statement.kind else {
        return None;
    };
    let PatKind::Binding(BindingMode::NONE, id, _, None) = binding.pat.kind else {
        return None;
    };
    if binding.els.is_some() {
        return None;
    }
    Some((id, binding.init?))
}

fn dereferences(expr: &Expr<'_>, binding: HirId) -> bool {
    matches!(expr.kind, ExprKind::Unary(UnOp::Deref, operand) if operand.res_local_id() == Some(binding))
}

// EXPLICIT015 can spell a compound update as two operands followed by an
// assignment to the saved place. Match the operands by binding identity.
fn borrowed_step<'tcx>(
    cx: &LateContext<'tcx>,
    update: &'tcx Expr<'tcx>,
    counter: HirId,
    ty: Ty<'tcx>,
    direction: BinOpKind,
) -> Option<()> {
    let ExprKind::Block(block, None) = update.kind else {
        return None;
    };
    let [amount, destination, assignment] = block.stmts else {
        return None;
    };
    if block.stmts.iter().any(|stmt| stmt.span.from_expansion()) {
        return None;
    }
    let StmtKind::Semi(assignment) = assignment.kind else {
        return None;
    };
    let (amount, step) = immutable_binding(amount)?;
    let (destination, saved_place) = immutable_binding(destination)?;
    let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, place) = saved_place.kind else {
        return None;
    };
    let ExprKind::Assign(target, value, _) = assignment.kind else {
        return None;
    };
    let (id, base, argument) = binary_call(cx, value)?;
    (block.expr.is_none()
        && matches!(block.rules, BlockCheckMode::DefaultBlock)
        && [
            saved_place,
            place,
            assignment,
            target,
            value,
            base,
            argument,
        ]
        .iter()
        .all(|expr| !expr.span.from_expansion())
        && literal_one(step)
        && place.res_local_id() == Some(counter)
        && dereferences(target, destination)
        && dereferences(base, destination)
        && argument.res_local_id() == Some(amount)
        && integer_update(cx, id, ty, direction))
    .then_some(())
}

fn native_width_step<'tcx>(
    cx: &LateContext<'tcx>,
    binding: &LetStmt<'tcx>,
    counter: HirId,
    ty: Ty<'tcx>,
    direction: BinOpKind,
) -> Option<()> {
    let PatKind::TupleStruct(ref pattern, [value], _) = binding.pat.kind else {
        return None;
    };
    let constructor = cx.qpath_res(pattern, binding.pat.hir_id).opt_def_id()?;
    if binding.els.is_some()
        || !clippy_utils::is_lang_item_or_ctor(cx, constructor, LangItem::ResultOk)
        || !matches!(value.kind, PatKind::Tuple([], _))
    {
        return None;
    }
    let call = binding.init?;
    let ExprKind::Call(..) = call.kind else {
        return None;
    };
    let (id, amount, place) = binary_call(cx, call)?;
    if call.span.from_expansion() || place.span.from_expansion() {
        return None;
    }
    let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, place) = place.kind else {
        return None;
    };
    if id.is_local()
        || cx.tcx.crate_name(id.krate).as_str() != "console_core_number_conversion"
        || cx.tcx.item_name(cx.tcx.parent(id)).as_str() != "assignments"
        || !literal_one(amount)
        || place.res_local_id() != Some(counter)
    {
        return None;
    }
    let name = match (direction, *ty.kind()) {
        (BinOpKind::Add, rustc_middle::ty::Uint(rustc_middle::ty::UintTy::Usize)) => {
            "wrapping_add_index"
        }
        (BinOpKind::Sub, rustc_middle::ty::Uint(rustc_middle::ty::UintTy::Usize)) => {
            "wrapping_sub_index"
        }
        (BinOpKind::Add, rustc_middle::ty::Int(rustc_middle::ty::IntTy::Isize)) => {
            "wrapping_add_offset"
        }
        (BinOpKind::Sub, rustc_middle::ty::Int(rustc_middle::ty::IntTy::Isize)) => {
            "wrapping_sub_offset"
        }
        _ => return None,
    };
    (cx.tcx.item_name(id).as_str() == name).then_some(())
}

// Recognize the same one-step update in both operator and named arithmetic
// forms. A strict comparison proves that this step cannot cross a type limit.
fn unit_step<'tcx>(
    cx: &LateContext<'tcx>,
    update: &'tcx Expr<'tcx>,
    counter: HirId,
    ty: Ty<'tcx>,
    direction: BinOpKind,
) -> Option<bool> {
    let (step, bare) = match update.kind {
        ExprKind::Block(..) => {
            borrowed_step(cx, update, counter, ty, direction)?;
            return Some(false);
        }
        ExprKind::AssignOp(op, place, step)
            if BinOpKind::from(op.node) == direction && place.res_local_id() == Some(counter) =>
        {
            (step, true)
        }
        ExprKind::Assign(place, value, _) if place.res_local_id() == Some(counter) => {
            let (base, step, bare) = match value.kind {
                ExprKind::Binary(op, base, step) if op.node == direction => (base, step, true),
                ExprKind::Field(..) => {
                    let (op, base, step) = wrapping_binary(cx, value)?;
                    if op != direction {
                        return None;
                    }
                    (base, step, false)
                }
                _ => {
                    let (id, base, step) = binary_call(cx, value)?;
                    if !integer_update(cx, id, ty, direction) {
                        return None;
                    }
                    (base, step, false)
                }
            };
            if base.res_local_id() != Some(counter) {
                return None;
            }
            (step, bare)
        }
        _ => return None,
    };
    literal_one(step).then_some(bare)
}

fn literal_guard<'tcx>(
    cx: &LateContext<'tcx>,
    guard: &'tcx Expr<'tcx>,
    block: &'tcx rustc_hir::Block<'tcx>,
    locals: &[HirId],
) -> Option<(Span, String)> {
    let ExprKind::Binary(operator, left, right) = guard.kind else {
        return None;
    };
    let (value, literal) = match (left.kind, right.kind) {
        (_, ExprKind::Lit(_)) => (left, right),
        (ExprKind::Lit(_), _) => (right, left),
        _ => return None,
    };
    let ty = cx.typeck_results().expr_ty(value);
    if (!ty.is_integral() && !ty.is_char())
        || ty != cx.typeck_results().expr_ty(literal)
        || [guard.span, value.span, literal.span]
            .iter()
            .any(|span| span.from_expansion())
    {
        return None;
    }
    let (on_literal, otherwise) = match operator.node {
        BinOpKind::Eq => ("{}", "break"),
        BinOpKind::Ne => ("break", "{}"),
        _ => return None,
    };
    let mutated = mutated_variables(guard, cx)?;
    if locals.iter().any(|id| mutated.contains(id)) {
        return None;
    }
    if visitors::for_each_expr(cx, guard, |child| match child.kind {
        ExprKind::Break(..)
        | ExprKind::Continue(..)
        | ExprKind::Block(
            rustc_hir::Block {
                rules: BlockCheckMode::UnsafeBlock(_),
                ..
            },
            _,
        ) => ControlFlow::Break(()),
        _ => ControlFlow::Continue(()),
    })
    .is_some()
    {
        return None;
    }

    let map = cx.sess().source_map();
    let start = block.span.with_hi(block.span.lo() + BytePos(1));
    if map.span_to_snippet(start).ok()? != "{" {
        return None;
    }
    let first = block.stmts.first()?;
    let prefix = start.with_lo(start.hi()).with_hi(first.span.lo());
    if !map.span_to_snippet(prefix).ok()?.trim().is_empty() {
        return None;
    }
    let indent = map.indentation_before(first.span)?;
    let arm_indent = format!("{indent}    ");
    let value = map.span_to_snippet(value.span).ok()?;
    let literal = map.span_to_snippet(literal.span).ok()?;
    let text = format!(
        "\n{indent}match {value} {{\n{arm_indent}{literal} => {on_literal},\n{arm_indent}_ => {otherwise},\n{indent}}};"
    );
    Some((start.shrink_to_hi(), text))
}

pub(super) fn counted_loop<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<Vec<(Span, String)>> {
    let walk = While::hir(expr)?;
    let (bounded, guard) = match walk.condition.kind {
        ExprKind::Binary(operator, bound, guard) if operator.node == BinOpKind::And => {
            (bound, Some(guard))
        }
        _ => (walk.condition, None),
    };
    let ExprKind::Binary(operator, counter, limit) = bounded.kind else {
        return None;
    };
    let (direction, symbol) = match operator.node {
        BinOpKind::Lt => (BinOpKind::Add, "+"),
        BinOpKind::Gt => (BinOpKind::Sub, "-"),
        _ => return None,
    };
    let counter_id = counter.res_local_id()?;
    let types = cx.typeck_results();
    let counter_ty = types.expr_ty(counter);
    if !counter_ty.is_integral() || types.expr_ty(limit) != counter_ty {
        return None;
    }
    let mut bound_locals = Vec::new();
    stable_bound(cx, limit, &mut bound_locals)?;
    if bound_locals.contains(&counter_id) {
        return None;
    }
    bound_locals.push(counter_id);
    let ExprKind::Block(block, _) = walk.body.kind else {
        return None;
    };
    let (last, earlier) = block.stmts.split_last()?;
    if block.expr.is_some() {
        return None;
    }
    let (increment, bare) = match last.kind {
        StmtKind::Semi(increment) => (
            increment.span,
            unit_step(cx, increment, counter_id, counter_ty, direction)?,
        ),
        StmtKind::Let(binding) => {
            native_width_step(cx, binding, counter_id, counter_ty, direction)?;
            (last.span, false)
        }
        _ => return None,
    };

    // A continue to this loop skips the original increment. The bound and
    // counter must otherwise retain their value throughout each iteration.
    if visitors::for_each_expr(cx, walk.body, |child| {
        let changes_control = matches!(child.kind,
            ExprKind::Continue(destination) if destination.target_id == Ok(expr.hir_id)
        );
        let is_unsafe = matches!(child.kind,
            ExprKind::Block(block, _) if matches!(block.rules, BlockCheckMode::UnsafeBlock(_))
        );
        if changes_control || is_unsafe {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .is_some()
    {
        return None;
    }
    for statement in earlier {
        let value = match statement.kind {
            StmtKind::Expr(value) | StmtKind::Semi(value) => Some(value),
            StmtKind::Let(binding) if binding.els.is_none() => binding.init,
            _ => return None,
        };
        if let Some(value) = value {
            let mutated = mutated_variables(value, cx)?;
            if bound_locals.iter().any(|id| mutated.contains(id)) {
                return None;
            }
            // A pattern can borrow its initializer mutably outside that
            // expression's own use visitor.
            if let StmtKind::Let(binding) = statement.kind {
                let mut mutable_pattern = false;
                binding.pat.each_binding(|mode, _, _, _| {
                    mutable_pattern |=
                        matches!(mode.0, rustc_hir::ByRef::Yes(_, rustc_hir::Mutability::Mut));
                });
                if mutable_pattern
                    && bound_locals
                        .iter()
                        .any(|id| visitors::is_local_used(cx, value, *id))
                {
                    return None;
                }
            }
        }
    }

    let map = cx.sess().source_map();
    let header = expr.span.with_hi(walk.condition.span.hi());
    let original = map.span_to_snippet(header).ok()?;
    let body = map.span_to_snippet(walk.body.span).ok()?;
    let update = map.span_to_snippet(increment).ok()?;
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    let signature = map
        .span_to_snippet(cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner)))
        .ok()?;
    if header.from_expansion()
        || increment.from_expansion()
        || original.contains('#')
        || [original.as_str(), update.as_str()]
            .iter()
            .any(|text| text.contains("//") || text.contains("/*"))
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
    let current = map.span_to_snippet(counter.span).ok()?;
    let limit = map.span_to_snippet(limit.span).ok()?;
    let (start, end, order) = if direction == BinOpKind::Add {
        (current.as_str(), limit.as_str(), "")
    } else {
        (limit.as_str(), current.as_str(), ".rev()")
    };
    let start_field = if start == "start" {
        start.to_owned()
    } else {
        format!("start: {start}")
    };
    let end_field = if end == "end" {
        end.to_owned()
    } else {
        format!("end: {end}")
    };
    let mut edits = vec![(
        header,
        format!("{label}for _ in (::core::ops::Range {{ {start_field}, {end_field} }}){order}"),
    )];
    if let Some(guard) = guard {
        edits.push(literal_guard(cx, guard, block, &bound_locals)?);
    }
    if bare {
        edits.push((
            increment,
            format!(
                "{current} = (::core::num::Wrapping({current}) {symbol} ::core::num::Wrapping(1)).0"
            ),
        ));
    }
    Some(edits)
}
