//! Keep an immutable position at a fixed width between native indexing operations.

use super::{CONVERSION_CRATE, boundary_conversion, fixes::{clean, has_block_glob}};
use clippy_utils::{higher::Range, visitors::for_each_expr_without_closures};
use rustc_hir::{BindingMode, Expr, ExprKind, LangItem, LetStmt, Node, PatKind, def::Res};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Span};
use std::{collections::BTreeSet, ops::ControlFlow};

// A source token must correspond to exactly one resolved expression. This also
// excludes shadowing, inactive cfg uses and macros that stringify an argument.
fn written_reads(text: &str, start: BytePos, name: &str) -> Option<BTreeSet<BytePos>> {
    use TokenKind::*;
    let mut at = 0;
    let mut delimiter = Vec::new();
    let mut reads = BTreeSet::new();
    let mut ident = false;
    let mut bang = false;
    for token in tokenize(text, FrontmatterAllowed::No) {
        let end = at + token.len as usize;
        let word = &text[at..end];
        match token.kind {
            Whitespace | BlockComment { .. } | LineComment { .. } => {
                at = end;
                continue;
            }
            Pound => return None,
            OpenParen | OpenBrace | OpenBracket => delimiter.push(bang),
            CloseParen | CloseBrace | CloseBracket => {
                delimiter.pop();
            }
            Ident | RawIdent if word.trim_start_matches("r#") == name => {
                if delimiter.contains(&true) {
                    return None;
                }
                reads.insert(start + BytePos(u32::try_from(at).ok()?));
            }
            Literal { .. } if word.contains('{') && word.contains(name) => return None,
            _ => {}
        }
        bang = ident && token.kind == Bang;
        ident = matches!(token.kind, Ident | RawIdent);
        at = end;
    }
    Some(reads)
}

fn native_use<'tcx>(
    cx: &LateContext<'tcx>,
    reference: &'tcx Expr<'tcx>,
    native: ty::Ty<'tcx>,
) -> bool {
    let types = cx.typeck_results();
    let mut child = reference.hir_id;
    for (_, node) in cx.tcx.hir_parent_iter(child) {
        match node {
            Node::ExprField(_) => continue,
            Node::Expr(parent) => match parent.kind {
                _ if Range::hir(cx, parent).is_some() => {
                    child = parent.hir_id;
                }
                ExprKind::Index(_, index, _) => return index.hir_id == child,
                // A primitive arithmetic operand is copied. Neither the
                // address of its storage nor a borrow can escape the expression.
                ExprKind::Binary(_, left, right) => {
                    return types.expr_ty(left).is_integral()
                        && types.expr_ty(right).is_integral()
                        && types.type_dependent_def_id(parent.hir_id).is_none();
                }
                ExprKind::Cast(value, _)
                | ExprKind::Unary(rustc_hir::UnOp::Not | rustc_hir::UnOp::Neg, value) => {
                    return value.hir_id == child
                        && types.expr_ty(value).is_integral()
                        && types.type_dependent_def_id(parent.hir_id).is_none();
                }
                ExprKind::Tup(values) | ExprKind::Array(values) => {
                    return values.iter().any(|item| item.hir_id == child);
                }
                ExprKind::Ret(Some(value)) => return value.hir_id == child,
                ExprKind::MethodCall(method, receiver, args, _) => {
                    let Some(id) = types.type_dependent_def_id(parent.hir_id) else {
                        return false;
                    };
                    if id.is_local() || cx.tcx.crate_name(id.krate).as_str() != "core" {
                        return false;
                    }
                    if matches!(
                        method.ident.as_str(),
                        "get"
                            | "get_mut"
                            | "split_at"
                            | "split_at_mut"
                            | "split_at_checked"
                            | "split_at_mut_checked"
                            | "chunks"
                            | "chunks_mut"
                            | "chunks_exact"
                            | "chunks_exact_mut"
                            | "windows"
                    ) && args.iter().any(|arg| arg.hir_id == child)
                        && types.expr_ty_adjusted(receiver).peel_refs().is_slice()
                    {
                        return true;
                    }
                    return types.expr_ty(receiver) == native
                        && cx.tcx.inherent_impl_of_assoc(id).is_some()
                        && cx
                            .tcx
                            .fn_sig(id)
                            .skip_binder()
                            .inputs()
                            .skip_binder()
                            .first()
                            == Some(&native)
                        && (receiver.hir_id == child
                            || args.iter().any(|arg| arg.hir_id == child));
                }
                ExprKind::Call(callee, args) => {
                    let ty::FnDef(id, arguments) = *types.expr_ty(callee).kind() else {
                        return false;
                    };
                    let Some(index) = args.iter().position(|arg| arg.hir_id == child) else {
                        return false;
                    };
                    // Calls, including generic helpers and tuple constructors,
                    // consume the original scalar type after instantiation.
                    return cx
                        .tcx
                        .fn_sig(id)
                        .instantiate(cx.tcx, arguments)
                        .skip_norm_wip()
                        .inputs()
                        .skip_binder()
                        .get(index)
                        .is_some_and(|ty| ty.is_integral() && *ty == native);
                }
                _ => return false,
            },
            _ => return false,
        }
    }
    false
}

pub(super) fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    local: &'tcx LetStmt<'tcx>,
) -> Option<Vec<(Span, String)>> {
    if !cx.sess().edition().at_least_rust_2018()
        || clippy_utils::is_in_const_context(cx)
        || local.els.is_some()
    {
        return None;
    }
    let (pattern, extract) = match local.pat.kind {
        PatKind::TupleStruct(ref path, [field], rest)
            if rest.as_opt_usize().is_none()
                && cx.qpath_res(path, local.pat.hir_id).opt_def_id().is_some_and(|id| {
                    clippy_utils::is_lang_item_or_ctor(cx, id, LangItem::ResultOk)
                }) => (field, true),
        _ => (local.pat, false),
    };
    let PatKind::Binding(BindingMode::NONE, binding, ident, None) = pattern.kind else {
        return None;
    };
    let init = local.init?;
    let types = cx.typeck_results();
    let native = types.node_type(binding);
    let (store, project) = match native.kind() {
        ty::Uint(ty::UintTy::Usize) => ("fixed_index", "index"),
        ty::Int(ty::IntTy::Isize) => ("fixed_offset", "wrapped_offset"),
        _ => return None,
    };
    // Other suggestions already convert these initializers directly at the
    // indexing boundary. Applying both would widen and immediately narrow.
    if matches!(init.kind, ExprKind::Lit(_))
        || matches!(init.kind, ExprKind::Call(callee, [_])
            if matches!(cx.typeck_results().expr_ty(callee).kind(), ty::FnDef(id, _)
                if cx.tcx.trait_of_assoc(*id).is_some_and(|owner|
                    cx.tcx.is_diagnostic_item(rustc_span::sym::From, owner)
                        || cx.tcx.is_diagnostic_item(rustc_span::sym::TryFrom, owner))))
        || matches!(
            init.kind,
            ExprKind::Cast(..) | ExprKind::Match(_, _, rustc_hir::MatchSource::TryDesugar(_))
        )
        || init.span.from_expansion()
        || !super::bounded_ranges::permits_edit(cx, init)
        || !boundary_conversion::available(cx, store, 0)
        || !boundary_conversion::available(cx, project, 1)
    {
        return None;
    }
    let initializer = types.expr_ty(init);
    if extract {
        let (value, _) = super::immediate_position::infallible_result(cx, initializer)?;
        if value != native || !types.expr_adjustments(init).is_empty() {
            return None;
        }
    } else if initializer != native {
        return None;
    }
    let Node::Stmt(statement) = cx.tcx.parent_hir_node(local.hir_id) else {
        return None;
    };
    let Node::Block(block) = cx.tcx.parent_hir_node(statement.hir_id) else {
        return None;
    };
    if let [_, borrow, _] = block.stmts
        && let rustc_hir::StmtKind::Let(place) = borrow.kind
        && place.hir_id != local.hir_id
        && super::assignment::replacement(cx, place).is_some()
    {
        return None;
    }
    if !block.span.contains(statement.span) || !statement.span.contains(init.span) {
        return None;
    }
    let written = clean(cx, ident.span)?;
    let rest = block.span.with_lo(statement.span.hi());
    let tail = cx.sess().source_map().span_to_snippet(rest).ok()?;
    let tokens = written_reads(&tail, rest.lo(), ident.as_str())?;
    if tokens.is_empty() {
        return None;
    }
    let mut refs = Vec::new();
    let mut positions = BTreeSet::new();
    let failed = for_each_expr_without_closures(block, |expr| {
        if let ExprKind::Path(ref path) = expr.kind
            && cx.qpath_res(path, expr.hir_id) == Res::Local(binding)
        {
            if expr.span.from_expansion()
                || !tokens.contains(&expr.span.lo())
                || !native_use(cx, expr, native)
                || has_block_glob(cx, expr.hir_id)
            {
                return ControlFlow::Break(());
            }
            positions.insert(expr.span.lo());
            refs.push(expr);
        }
        ControlFlow::Continue(())
    });
    if failed.is_some() || positions != tokens {
        return None;
    }
    let body = cx
        .tcx
        .hir_body_owned_by(cx.tcx.hir_enclosing_body_owner(binding))
        .value
        .span;
    let source = cx.sess().source_map().span_to_snippet(body).ok()?;
    let module = cx.tcx.parent_module(local.hir_id).to_local_def_id();
    let children = cx.tcx.module_children_local(module);
    let mut projection = "native_position".to_owned();
    while source.contains(&projection)
        || children.iter().any(|child| child.ident.as_str() == projection)
    {
        projection.push('_');
    }
    let ok = "::core::result::Result::Ok";
    let initializer = format!("::{CONVERSION_CRATE}::{store}(");
    let mut edits = Vec::new();
    let (prefix, suffix) = if extract {
        // A match keeps temporaries in the existing initializer scope.
        (
            format!("{initializer}match "),
            format!(" {{ {ok}({written}) => {written} }})"),
        )
    } else {
        edits.push((ident.span, format!("{ok}({written})")));
        (initializer, ")".to_owned())
    };
    edits.push((init.span.shrink_to_lo(), prefix));
    edits.push((init.span.shrink_to_hi(), suffix));
    for reference in refs {
        if let Node::Expr(parent) = cx.tcx.parent_hir_node(reference.hir_id)
            && let ExprKind::Cast(value, _) = parent.kind
            && value.hir_id == reference.hir_id
            && matches!(
                (native.kind(), cx.typeck_results().expr_ty(parent).kind()),
                (ty::Uint(ty::UintTy::Usize), ty::Uint(ty::UintTy::U64))
                    | (ty::Int(ty::IntTy::Isize), ty::Int(ty::IntTy::I64))
            )
            && clean(cx, parent.span).is_some()
        {
            edits.push((parent.span, written.clone()));
            continue;
        }
        // An unnamed result extraction cannot shadow a binding outside its arm.
        let cast = format!("::{CONVERSION_CRATE}::{project}({written})");
        let conversion =
            format!("match {cast} {{ ::core::result::Result::Ok({projection}) => {projection} }}");
        let needs_parentheses = matches!(cx.tcx.parent_hir_node(reference.hir_id), Node::Expr(parent)
            if matches!(parent.kind, ExprKind::MethodCall(_, receiver, _, _) | ExprKind::Binary(_, receiver, _) | ExprKind::Cast(receiver, _) if receiver.hir_id == reference.hir_id));
        edits.push((
            reference.span,
            if needs_parentheses {
                format!("({conversion})")
            } else {
                conversion
            },
        ));
    }
    Some(edits)
}
