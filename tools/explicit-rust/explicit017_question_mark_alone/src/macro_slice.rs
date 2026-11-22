//! Bind checked references at their use or before an audited macro.
use super::{Explicit017QuestionMarkAlone, binding_name, local_binding_name, macro_input};
use clippy_utils::{
    macros::{MacroCall, macro_backtrace},
    source::indent_of,
    sym, visitors,
};
use rustc_hir::{
    BindingMode, BorrowKind, Expr, ExprKind, MatchSource, Node, PatKind, StmtKind, UnOp,
    def::{DefKind, Res},
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use std::ops::ControlFlow;

fn slice_storage(cx: &LateContext<'_>, base: &Expr<'_>, budget: u8) -> bool {
    let Some(next) = budget.checked_sub(1) else {
        return false;
    };
    let types = cx.typeck_results();
    match base.kind {
        ExprKind::AddrOf(BorrowKind::Ref, _, place) => slice_storage(cx, place, next),
        ExprKind::Unary(UnOp::Deref, reference) if types.expr_ty(reference).is_ref() => {
            slice_storage(cx, reference, next)
        }
        // A borrowed view of an existing place has no temporary owner. Keep
        // its borrow and call at the original lookup site.
        ExprKind::Call(_, [receiver]) | ExprKind::MethodCall(_, receiver, [], _)
            if !base.span.from_expansion()
                && matches!(types.expr_ty(base).kind(), ty::Ref(_, view, _) if view.is_slice() || view.is_array())
                && types.expr_ty_adjusted(receiver).is_ref()
                && clippy_utils::fn_def_id(cx, base).is_some() =>
        {
            let place = match receiver.kind {
                ExprKind::AddrOf(BorrowKind::Ref, _, place) => place,
                _ => receiver,
            };
            super::existing_place(cx, place)
        }
        ExprKind::Path(ref path)
            if matches!(
                cx.qpath_res(path, base.hir_id),
                Res::Local(_)
                    | Res::Def(
                        DefKind::Static {
                            mutability: rustc_hir::Mutability::Not,
                            ..
                        },
                        _
                    )
            ) =>
        {
            let storage = types.expr_ty(base).peel_refs();
            storage.is_slice()
                || storage.is_array()
                || storage
                    .ty_adt_def()
                    .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Vec, adt.did()))
        }
        // The outer lookup can borrow a row from another checked lookup.
        ExprKind::Match(branch, _, MatchSource::TryDesugar(_))
            if types.expr_ty(base).is_ref() && !base.span.from_expansion() =>
        {
            matches!(branch.kind, ExprKind::Call(_, [value]) if checked_slice_with_budget(cx, value, next))
        }
        // A block that binds that row and returns it retains the same storage.
        ExprKind::Block(block, None)
            if !base.span.from_expansion() && types.expr_ty(base).is_ref() =>
        {
            let [statement] = block.stmts else {
                return false;
            };
            let StmtKind::Let(local) = statement.kind else {
                return false;
            };
            matches!(local.pat.kind, PatKind::Binding(BindingMode::NONE, binding, _, None)
                if block.expr.is_some_and(|tail| matches!(tail.kind, ExprKind::Path(ref path)
                    if cx.qpath_res(path, tail.hir_id) == Res::Local(binding))))
                && local.els.is_none()
                && cx.tcx.hir_attrs(local.hir_id).is_empty()
                && local.init.is_some_and(|init| slice_storage(cx, init, next))
        }
        _ => false,
    }
}

fn result_method(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    cx.typeck_results()
        .type_dependent_def_id(call.hir_id)
        .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
        .and_then(|imp| {
            cx.tcx
                .type_of(imp)
                .instantiate_identity()
                .skip_norm_wip()
                .ty_adt_def()
        })
        .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Result, adt.did()))
}

// Only the source slice can supply the result's lifetime. Splitting a
// statement cannot extend a reference into a temporary slice or shorten an
// observable guard in the bounds or error expression.
pub(super) fn checked_slice(cx: &LateContext<'_>, value: &Expr<'_>) -> bool {
    checked_slice_with_budget(cx, value, 16)
}

fn checked_slice_with_budget(cx: &LateContext<'_>, value: &Expr<'_>, budget: u8) -> bool {
    let types = cx.typeck_results();
    let operation = match value.kind {
        ExprKind::MethodCall(method, receiver, [mapper], _)
            if matches!(method.ident.name.as_str(), "and_then" | "map_err")
                && matches!(mapper.kind, ExprKind::Closure(_))
                && result_method(cx, value) =>
        {
            receiver
        }
        _ => value,
    };
    let get = match operation.kind {
        ExprKind::MethodCall(method, receiver, [_], _) if method.ident.name.as_str() == "ok_or" => {
            if !types
                .type_dependent_def_id(operation.hir_id)
                .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
                .is_some_and(|id| {
                    cx.tcx
                        .type_of(id)
                        .instantiate_identity()
                        .skip_norm_wip()
                        .ty_adt_def()
                        .is_some_and(|adt| cx.tcx.is_diagnostic_item(sym::Option, adt.did()))
                })
            {
                return false;
            }
            receiver
        }
        _ => operation,
    };
    let (id, base) = match get.kind {
        ExprKind::Call(_, [base, _]) | ExprKind::Call(_, [base]) => {
            (clippy_utils::fn_def_id(cx, get), base)
        }
        ExprKind::MethodCall(_, base, [_], _) => (types.type_dependent_def_id(get.hir_id), base),
        _ => return false,
    };
    let Some(id) = id else { return false };
    let from_slice = matches!(cx.tcx.item_name(id).as_str(), "get" | "get_mut")
        && cx.tcx.inherent_impl_of_assoc(id).is_some_and(|imp| {
            cx.tcx
                .type_of(imp)
                .instantiate_identity()
                .skip_norm_wip()
                .is_slice()
        });
    let to_array = matches!(get.kind, ExprKind::Call(_, [_]))
        && cx
            .tcx
            .trait_of_assoc(id)
            .is_some_and(|tr| cx.tcx.is_diagnostic_item(rustc_span::sym::TryFrom, tr))
        && matches!(types.expr_ty(base).kind(), ty::Ref(_, s, _) if s.is_slice())
        && matches!(types.expr_ty(get).kind(), ty::Adt(result, args) if cx.tcx.is_diagnostic_item(sym::Result, result.did()) && matches!(args.type_at(0).kind(), ty::Ref(_, array, _) if array.is_array()));
    if !(from_slice || to_array) || !slice_storage(cx, base, budget) {
        return false;
    }
    visitors::for_each_expr_without_closures(value, |e| {
        if e.hir_id != value.hir_id
            && types.expr_ty(e).needs_drop(cx.tcx, cx.typing_env())
            && !matches!(e.kind, ExprKind::Path(ref path) if matches!(cx.qpath_res(path, e.hir_id), Res::Local(_)))
            && !matches!(e.kind, ExprKind::Field(..))
        {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .is_none()
}

// Forwarding macros may expand a supplied expression or statement in a different
// scope. Only split a statement that is written directly in the function body.
fn authored_statement(prefix: &str) -> bool {
    let mut groups = Vec::new();
    let mut last = None;
    for token in tokenize(prefix, FrontmatterAllowed::No) {
        use TokenKind::*;
        match token.kind {
            Whitespace | LineComment { .. } | BlockComment { .. } => continue,
            OpenParen | OpenBrace | OpenBracket => groups.push(last == Some(Bang)),
            CloseParen | CloseBrace | CloseBracket if groups.pop().is_none() => return false,
            _ => {}
        }
        last = Some(token.kind);
    }
    matches!(
        last,
        Some(TokenKind::OpenBrace | TokenKind::CloseBrace | TokenKind::Semi)
    ) && !groups.contains(&true)
}

pub(super) fn authored_body(
    cx: &LateContext<'_>,
    question: &Expr<'_>,
    at: Span,
    body_macros: &[String],
) -> Option<Span> {
    let body_span = authored_scope(cx, question, at, body_macros)?;
    let map = cx.sess().source_map();
    authored_statement(&map.span_to_snippet(body_span.with_hi(at.lo())).ok()?).then_some(body_span)
}

pub(super) fn authored_scope(
    cx: &LateContext<'_>,
    question: &Expr<'_>,
    at: Span,
    body_macros: &[String],
) -> Option<Span> {
    let owner = cx.tcx.hir_enclosing_body_owner(question.hir_id);
    let function_span = cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner));
    let body_span = if function_span.from_expansion() {
        if !audited_body(cx, function_span, body_macros) {
            return None;
        }
        super::authored_function::authored_function(cx, function_span, &[at, question.span])?
    } else {
        cx.tcx.hir_body_owned_by(owner).value.span
    };

    (body_span.contains(at) && body_span.contains(question.span)).then_some(body_span)
}

fn audited_call(cx: &LateContext<'_>, call: &MacroCall, names: &[String]) -> bool {
    let id = call.def_id;
    let path = format!(
        "{}{}",
        cx.tcx.crate_name(id.krate),
        cx.tcx.def_path(id).to_string_no_crate_verbose()
    );
    names.contains(&path)
}

fn audited_body(cx: &LateContext<'_>, span: Span, body_macros: &[String]) -> bool {
    let mut expansions = macro_backtrace(span).peekable();
    expansions.peek().is_some() && expansions.all(|call| audited_call(cx, &call, body_macros))
}

// Find the authored first argument. A conversion such as
// <&[u8; 32]>::try_from(&*checked?).unwrap() evaluates the lookup before
// either call. Keep the conversion in place and move only the lookup.
fn first_macro_argument<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    call: Span,
) -> Option<Span> {
    let map = cx.sess().source_map();
    let mut root = question;
    let mut replace = question.span;
    loop {
        if root.span.from_expansion()
            || !call.contains(root.span)
            || !cx.tcx.hir_attrs(root.hir_id).is_empty()
        {
            return None;
        }
        let prefix = map.span_to_snippet(call.with_hi(root.span.lo())).ok()?;
        let (_, arguments) = prefix.split_once('!')?;
        let before = arguments.trim_start().strip_prefix('(')?.trim();
        if before.is_empty()
            && map
                .span_to_snippet(call.with_lo(root.span.hi()))
                .ok()?
                .trim_start()
                .starts_with([',', ')'])
        {
            return Some(replace);
        }
        let Node::Expr(parent) = cx.tcx.parent_hir_node(root.hir_id) else {
            return None;
        };
        match parent.kind {
            ExprKind::Unary(UnOp::Deref, _) if root.hir_id == question.hir_id => {
                let Node::Expr(borrow) = cx.tcx.parent_hir_node(parent.hir_id) else {
                    return None;
                };
                if !matches!(borrow.kind, ExprKind::AddrOf(BorrowKind::Ref, _, _))
                    || cx.typeck_results().expr_ty(borrow) != cx.typeck_results().expr_ty(question)
                    || !cx.tcx.hir_attrs(parent.hir_id).is_empty()
                    || parent.span.from_expansion()
                {
                    return None;
                }
                let source = map.span_to_snippet(borrow.span).ok()?;
                if source.contains("/*") || source.contains("//") || source.contains('#') {
                    return None;
                }
                replace = borrow.span;
                root = borrow;
            }
            ExprKind::Call(_, [first, ..])
                if first.hir_id == root.hir_id && super::first_evaluated(cx, root, parent) =>
            {
                root = parent
            }
            ExprKind::MethodCall(_, receiver, _, _) if receiver.hir_id == root.hir_id => {
                root = parent
            }
            _ => return None,
        }
    }
}

pub(super) fn lift<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    branch: &'tcx Expr<'tcx>,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<Vec<(Span, String)>> {
    if question.span.from_expansion() {
        return None;
    }
    let ExprKind::Call(_, [value]) = branch.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let ty::Ref(_, inner, _) = *types.expr_ty(question).kind() else {
        return None;
    };
    if !(inner.is_slice() || inner.is_array())
        || !checked_slice(cx, value)
        || value.span.from_expansion()
    {
        return None;
    }
    let map = cx.sess().source_map();
    // A fresh slice reference has no second user whose borrow could be shortened.
    if let Node::Expr(deref) = cx.tcx.parent_hir_node(question.hir_id)
        && matches!(deref.kind, ExprKind::Unary(UnOp::Deref, _))
        && let Node::Expr(borrow) = cx.tcx.parent_hir_node(deref.hir_id)
        && matches!(borrow.kind, ExprKind::AddrOf(_, _, _))
        && types.expr_ty(borrow) == types.expr_ty(question)
        && let Node::LetStmt(local) = cx.tcx.parent_hir_node(borrow.hir_id)
        && !local.span.from_expansion()
        && local.span.contains(borrow.span)
        && authored_body(cx, question, local.span, &pass.body_macros).is_some()
        && matches!(
            local.pat.kind,
            rustc_hir::PatKind::Binding(rustc_hir::BindingMode::NONE, ..)
        )
        && !borrow.span.from_expansion()
        && borrow.span.contains(question.span)
        && borrow.span.contains(value.span)
        && [question.hir_id, deref.hir_id, borrow.hir_id]
            .into_iter()
            .all(|id| cx.tcx.hir_attrs(id).is_empty())
    {
        let original = map.span_to_snippet(borrow.span).ok()?;
        if !original.contains("//") && !original.contains("/*") && !original.contains('#') {
            // Edit the wrapper only so a nested lookup can be fixed separately.
            return Some(vec![
                (borrow.span.with_hi(value.span.lo()), String::new()),
                (borrow.span.with_lo(value.span.hi()), "?".to_owned()),
            ]);
        }
    }
    if pass.first_slice_macros.is_empty() {
        return None;
    }
    lift_statement(cx, question, value, pass)
        .or_else(|| bind_macro_input(cx, question, value, pass))
}

fn lift_statement<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    value: &'tcx Expr<'tcx>,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<Vec<(Span, String)>> {
    let map = cx.sess().source_map();
    for (_, node) in cx.tcx.hir_parent_iter(question.hir_id) {
        let Node::Stmt(stmt) = node else { continue };
        let expr = match stmt.kind {
            StmtKind::Let(local)
                if local.els.is_none() && cx.tcx.hir_attrs(local.hir_id).is_empty() =>
            {
                local.init?
            }
            StmtKind::Expr(expr) | StmtKind::Semi(expr) => expr,
            _ => continue,
        };
        if !cx.tcx.hir_attrs(stmt.hir_id).is_empty() {
            continue;
        }
        // The macro may supply the first argument to another intrinsic.
        // Evaluating the path of a resolved function item has no effects.
        // Stop at any other enclosing operation or unreviewed expansion.
        let mut first = expr;
        let call = loop {
            if let Some(call) = macro_backtrace(first.span).next() {
                break Some(call);
            }
            if first.span.from_expansion()
                || !cx.tcx.hir_attrs(first.hir_id).is_empty()
                || !first.span.contains(question.span)
            {
                break None;
            }
            match first.kind {
                ExprKind::Call(_, [argument, ..])
                    if super::first_evaluated(cx, argument, first) =>
                {
                    first = argument
                }
                ExprKind::MethodCall(_, receiver, _, _) => first = receiver,
                _ => break None,
            }
        };
        let Some(call) = call else { continue };
        if !audited_call(cx, &call, &pass.first_slice_macros)
            || call.span.from_expansion()
            || !call.span.contains(question.span)
            || !call.span.contains(value.span)
        {
            continue;
        }
        let at = match stmt.kind {
            StmtKind::Let(_) if !stmt.span.from_expansion() && stmt.span.contains(call.span) => {
                stmt.span
            }
            StmtKind::Expr(_) | StmtKind::Semi(_)
                if !stmt.span.from_expansion() && stmt.span.contains(call.span) =>
            {
                stmt.span
            }
            StmtKind::Expr(_) | StmtKind::Semi(_) if first.hir_id == expr.hir_id => call.span,
            _ => continue,
        };
        let argument = first_macro_argument(cx, question, call.span)?;
        let source = map.span_to_snippet(question.span).ok()?;
        if source.contains("//") || source.contains("/*") {
            return None;
        }
        let source = map.span_to_snippet(value.span).ok()?;
        let owner = cx.tcx.hir_enclosing_body_owner(question.hir_id);
        let function = map
            .span_to_snippet(cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner)))
            .ok()?;
        let body_span = authored_body(cx, question, at, &pass.body_macros)?;
        let body = map.span_to_snippet(body_span).ok()?;
        let module = cx.tcx.parent_module(question.hir_id).to_local_def_id();
        let name = loop {
            let candidate = binding_name(&mut pass.next_name, owner, &function, &body)?;
            if !cx
                .tcx
                .module_children_local(module)
                .iter()
                .any(|child| child.ident.as_str() == candidate)
            {
                break candidate;
            }
        };
        let indent = clippy_utils::source::indent_of(cx, at)?;
        return Some(vec![
            (
                at.shrink_to_lo(),
                format!("let {name} = {source}?;\n{}", " ".repeat(indent)),
            ),
            (argument, name),
        ]);
    }
    None
}

// An audited macro can be nested in a larger expression that evaluates other
// arguments first. Bind the reference inside the written argument in that case.
fn bind_macro_input<'tcx>(
    cx: &LateContext<'tcx>,
    question: &'tcx Expr<'tcx>,
    value: &'tcx Expr<'tcx>,
    pass: &mut Explicit017QuestionMarkAlone,
) -> Option<Vec<(Span, String)>> {
    if !question.span.contains(value.span) || !cx.tcx.hir_attrs(question.hir_id).is_empty() {
        return None;
    }
    let owner = cx.tcx.hir_enclosing_body_owner(question.hir_id);
    let enclosing = cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner));
    if enclosing.from_expansion() && !audited_body(cx, enclosing, &pass.body_macros) {
        return None;
    }
    let call = cx
        .tcx
        .hir_parent_iter(question.hir_id)
        .find_map(|(_, node)| {
            let Node::Expr(parent) = node else {
                return None;
            };
            macro_backtrace(parent.span).find(|call| {
                !call.span.from_expansion()
                    && call.span.contains(question.span)
                    && audited_call(cx, call, &pass.first_slice_macros)
            })
        })?;
    if !macro_input::outside_macro_input(cx, call.span) {
        return None;
    }
    let map = cx.sess().source_map();
    let before = map
        .span_to_snippet(call.span.with_hi(question.span.lo()))
        .ok()?;
    let (_, opening) = before.split_once('!')?;
    let close = match opening.trim() {
        "(" => ')',
        "[" => ']',
        "{" => '}',
        _ => return None,
    };
    let after = map
        .span_to_snippet(call.span.with_lo(question.span.hi()))
        .ok()?;
    if !after.trim_start().starts_with([',', close]) {
        return None;
    }
    let source = map.span_to_snippet(question.span).ok()?;
    if !source.ends_with('?') || source.contains("//") || source.contains("/*") {
        return None;
    }
    let name = local_binding_name(cx, question, &source, pass)?;
    let indent = " ".repeat(indent_of(cx, question.span)?);
    let inner = format!("{indent}  ");
    Some(vec![(
        question.span,
        format!("{{\n{inner}let {name} = {source};\n{inner}{name}\n{indent}}}"),
    )])
}
