//! Reuse a checked fixed prefix for subsequent reads of the same slice.

use super::fixes::{clean, element_pattern, function_source, literal_length, position};
use super::recovery::{binding_name, dereference};
use clippy_utils::is_lang_item_or_ctor;
use clippy_utils::res::MaybeResPath;
use rustc_hir::{
    BindingMode, Expr, ExprKind, GenericArg, HirId, LangItem, LetStmt, MatchSource, Node, Pat,
    PatExpr, PatExprKind, PatKind, StmtKind,
    intravisit::{self, Visitor},
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{BytePos, Symbol};
use std::ops::Range;

fn immutable_local(cx: &LateContext<'_>, binding: HirId) -> bool {
    matches!(cx.tcx.hir_node(binding), Node::Pat(pat)
        if matches!(pat.kind, PatKind::Binding(BindingMode::NONE, ..)))
}

fn option_case(cx: &LateContext<'_>, pat: &Pat<'_>) -> Option<LangItem> {
    let (path, id, variant) = match pat.kind {
        PatKind::TupleStruct(ref path, [_], _) => (path, pat.hir_id, LangItem::OptionSome),
        PatKind::Expr(PatExpr {
            kind: PatExprKind::Path(path),
            hir_id,
            ..
        }) => (path, *hir_id, LangItem::OptionNone),
        _ => return None,
    };
    let constructor = cx.qpath_res(path, id).opt_def_id()?;
    is_lang_item_or_ctor(cx, constructor, variant).then_some(variant)
}

fn checked_chunk(cx: &LateContext<'_>, statement: &LetStmt<'_>, slice: HirId) -> Option<u64> {
    let ExprKind::Match(check, arms, MatchSource::Normal) = statement.init?.kind else {
        return None;
    };
    let ExprKind::MethodCall(segment, receiver, [], _) = check.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let method = types.type_dependent_def_id(check.hir_id)?;
    if check.span.from_expansion()
        || receiver.res_local_id() != Some(slice)
        || cx.tcx.crate_name(method.krate).as_str() != "core"
        || cx.tcx.item_name(method).as_str() != "first_chunk"
        || !cx
            .tcx
            .inherent_impl_of_assoc(method)
            .is_some_and(|implementation| {
                cx.tcx
                    .type_of(implementation)
                    .instantiate_identity()
                    .skip_norm_wip()
                    .is_slice()
            })
    {
        return None;
    }
    let [GenericArg::Const(length)] = segment.args?.args else {
        return None;
    };
    let [first, second] = arms else { return None };
    let (present, absent) = match (option_case(cx, first.pat)?, option_case(cx, second.pat)?) {
        (LangItem::OptionSome, LangItem::OptionNone) => (first, second),
        (LangItem::OptionNone, LangItem::OptionSome) => (second, first),
        _ => return None,
    };
    if present.guard.is_some() || absent.guard.is_some() || !types.expr_ty(absent.body).is_never() {
        return None;
    }
    let PatKind::TupleStruct(_, [field], _) = present.pat.kind else {
        return None;
    };
    let PatKind::Binding(BindingMode::NONE, binding, _, None) = field.kind else {
        return None;
    };
    if present.body.res_local_id() != Some(binding) {
        return None;
    }
    literal_length(cx, length.as_unambig_ct())
}

struct Shadow {
    name: Symbol,
    between: Range<BytePos>,
    found: bool,
}

impl<'tcx> Visitor<'tcx> for Shadow {
    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        if let PatKind::Binding(_, _, name, _) = pat.kind
            && name.name == self.name
            && self.between.contains(&pat.span.lo())
        {
            self.found = true;
        }
        if !self.found {
            intravisit::walk_pat(self, pat);
        }
    }
}

pub(super) fn replacement<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> Option<String> {
    let ExprKind::Index(base, offset, _) = expr.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let slice = base.res_local_id()?;
    let at = position(offset, 8)?;
    let ty::Ref(_, slice_ty, ty::Mutability::Not) = types.expr_ty(base).kind() else {
        return None;
    };
    let ty::Slice(element) = slice_ty.kind() else {
        return None;
    };
    if types.type_dependent_def_id(expr.hir_id).is_some() || !immutable_local(cx, slice) {
        return None;
    }
    clean(cx, expr.span)?;
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    let (checked, name, length) =
        cx.tcx.hir_parent_iter(expr.hir_id).find_map(|(_, node)| {
            let Node::Block(block) = node else {
                return None;
            };
            block.stmts.iter().rev().find_map(|stmt| {
                let StmtKind::Let(local) = stmt.kind else {
                    return None;
                };
                if stmt.span.hi() > expr.span.lo()
                    || stmt.span.from_expansion()
                    || !expr.span.eq_ctxt(local.pat.span)
                    || cx.tcx.hir_enclosing_body_owner(local.hir_id) != owner
                {
                    return None;
                }
                let PatKind::Binding(BindingMode::NONE, binding, name, None) = local.pat.kind
                else {
                    return None;
                };
                let length = checked_chunk(cx, local, slice)?;
                let ty::Ref(_, prefix, ty::Mutability::Not) = types.node_type(binding).kind()
                else {
                    return None;
                };
                let ty::Array(item, size) = prefix.kind() else {
                    return None;
                };
                if item != element
                    || size.try_to_target_usize(cx.tcx)? != length
                    || u64::from(at) >= length
                {
                    return None;
                }
                Some((stmt.span, name, length))
            })
        })?;
    // A different conditional binding or a macro that introduces a local
    // between the check and the access can change what the written name means.
    let source = function_source(cx, expr)?;
    if tokenize(&source, FrontmatterAllowed::No).any(|token| token.kind == TokenKind::Pound) {
        return None;
    }
    let map = cx.sess().source_map();
    let between = map.span_to_snippet(checked.with_hi(expr.span.lo())).ok()?;
    if tokenize(&between, FrontmatterAllowed::No).any(|token| token.kind == TokenKind::Bang) {
        return None;
    }
    let mut shadow = Shadow {
        name: name.name,
        between: checked.hi()..expr.span.lo(),
        found: false,
    };
    shadow.visit_body(cx.tcx.hir_body_owned_by(owner));
    if shadow.found {
        return None;
    }
    let prefix = clean(cx, name.span)?;
    let element = binding_name(cx, expr, "_explicit_prefix_element")?;
    let pattern = element_pattern(cx, base, at, length, &element)?;
    Some(dereference(
        cx,
        expr,
        format!("match {prefix} {{ {pattern} => {element} }}"),
    ))
}
