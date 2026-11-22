use clippy_utils::is_hir_ty_cfg_dependant;
use clippy_utils::visitors::for_each_expr_without_closures;
use rustc_hir::attrs::AttributeKind;
use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{
    Attribute, Expr, ExprKind, FnRetTy, HirId, ItemKind, Node, PatKind, Path, QPath, Ty, TyKind,
    UseKind,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use std::ops::ControlFlow;

pub(super) fn conditional_definition(cx: &LateContext<'_>, definition: LocalDefId) -> bool {
    let id = cx.tcx.local_def_id_to_hir_id(definition);
    std::iter::once(id)
        .chain(cx.tcx.hir_parent_id_iter(id))
        .flat_map(|id| cx.tcx.hir_attrs(id))
        .any(|attr| {
            matches!(
                attr,
                Attribute::Parsed(AttributeKind::CfgTrace(..) | AttributeKind::CfgAttrTrace)
            )
        })
}

pub(super) fn conditional_type(cx: &LateContext<'_>, ty: &Ty<'_>) -> bool {
    if is_hir_ty_cfg_dependant(cx, ty) {
        return true;
    }
    let path = match &ty.kind {
        TyKind::Ref(_, reference) | TyKind::Ptr(reference) => {
            return conditional_type(cx, reference.ty);
        }
        TyKind::Path(path) => path,
        _ => return false,
    };
    if let QPath::Resolved(None, path) = path
        && conditional_import(cx, ty.hir_id, path)
    {
        return true;
    }
    let Some(definition) = cx
        .qpath_res(path, ty.hir_id)
        .opt_def_id()
        .and_then(|id| id.as_local())
    else {
        return false;
    };
    conditional_definition(cx, definition)
        || matches!(cx.tcx.hir_node_by_def_id(definition), Node::Item(item)
            if matches!(item.kind, ItemKind::TyAlias(_, _, alias) if conditional_type(cx, alias)))
}

pub(super) fn conditional_import(cx: &LateContext<'_>, use_site: HirId, path: &Path<'_>) -> bool {
    let Some(first) = path.segments.first() else {
        return false;
    };
    let module = cx.tcx.parent_module_from_def_id(use_site.owner.def_id);
    cx.tcx.hir_module_free_items(module).any(|id| {
        let item = cx.tcx.hir_item(id);
        match item.kind {
            ItemKind::Use(_, UseKind::Single(name)) if name.name == first.ident.name => {
                conditional_definition(cx, item.owner_id.def_id)
            }
            ItemKind::Use(_, UseKind::Glob) => conditional_definition(cx, item.owner_id.def_id),
            _ => false,
        }
    })
}

fn binding_annotation<'tcx>(cx: &LateContext<'tcx>, id: HirId) -> Option<&'tcx Ty<'tcx>> {
    match cx.tcx.parent_hir_node(id) {
        Node::LetStmt(binding) => binding.ty,
        Node::Param(param) => {
            let owner = cx.tcx.hir_enclosing_body_owner(param.hir_id);
            let declaration = cx
                .tcx
                .hir_fn_decl_by_hir_id(cx.tcx.local_def_id_to_hir_id(owner))?;
            let body = cx.tcx.hir_body_owned_by(owner);
            body.params
                .iter()
                .zip(declaration.inputs)
                .find_map(|(parameter, ty)| (parameter.hir_id == param.hir_id).then_some(ty))
        }
        _ => None,
    }
}

pub(super) fn uses_conditional_binding<'tcx>(
    cx: &LateContext<'tcx>,
    operand: &'tcx Expr<'tcx>,
) -> bool {
    for_each_expr_without_closures(operand, |expression| {
        if let ExprKind::Path(ref path) = expression.kind
            && let Res::Local(binding) = cx.qpath_res(path, expression.hir_id)
            && binding_annotation(cx, binding).is_some_and(|ty| conditional_type(cx, ty))
        {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    })
    .is_some()
}

fn primitive_annotation(cx: &LateContext<'_>, ty: &Ty<'_>) -> bool {
    matches!(ty.kind, TyKind::Path(ref path)
        if matches!(cx.qpath_res(path, ty.hir_id), Res::PrimTy(_)))
}

fn primitive_output(cx: &LateContext<'_>, definition: DefId) -> bool {
    let Some(definition) = definition.as_local() else {
        return false;
    };
    !conditional_definition(cx, definition)
        && matches!(cx.tcx.hir_node_by_def_id(definition).fn_decl(),
            Some(declaration) if matches!(declaration.output, FnRetTy::Return(ty)
                if primitive_annotation(cx, ty)))
}

fn fixed_method_receiver(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    match receiver.kind {
        ExprKind::Path(ref path) => {
            matches!(cx.qpath_res(path, receiver.hir_id), Res::Local(binding)
            if binding_annotation(cx, binding).is_some_and(|ty| !conditional_type(cx, ty)))
        }
        ExprKind::Call(callee, _) => matches!(callee.kind, ExprKind::Path(ref path)
            if !matches!(path, QPath::Resolved(None, path) if conditional_import(cx, callee.hir_id, path))
                && matches!(cx.qpath_res(path, callee.hir_id),
                    Res::Def(DefKind::Ctor(CtorOf::Struct, _), constructor)
                        if constructor.as_local().is_some_and(|id| !conditional_definition(cx, id)))),
        _ => false,
    }
}

// Inactive local declarations are absent from HIR. Scan the prefixes of the
// enclosing blocks for attributes that could introduce a different binding.
pub(super) fn cfg_can_shadow_locals(cx: &LateContext<'_>, operand: &Expr<'_>) -> bool {
    let map = cx.sess().source_map();
    cx.tcx.hir_parent_iter(operand.hir_id).any(|(_, node)| {
        let Node::Block(block) = node else {
            return false;
        };
        if !block.span.contains(operand.span) || block.span.from_expansion() {
            return false;
        }
        let Ok(source) = map.span_to_snippet(block.span.with_hi(operand.span.lo())) else {
            return true;
        };
        let mut depth = 0usize;
        let mut offset = 0;
        for token in tokenize(&source, FrontmatterAllowed::No) {
            offset += token.len as usize;
            match token.kind {
                TokenKind::OpenBrace | TokenKind::OpenBracket | TokenKind::OpenParen => {
                    depth += 1
                }
                TokenKind::CloseBrace | TokenKind::CloseBracket | TokenKind::CloseParen => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Pound if depth == 1 => {
                    if let Some(attribute) = source[offset..].trim_start().strip_prefix('[')
                        && attribute.trim_start().starts_with("cfg")
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    })
}

pub(super) fn fixed_primitive_source<'tcx>(
    cx: &LateContext<'tcx>,
    operand: &'tcx Expr<'tcx>,
) -> bool {
    match operand.kind {
        ExprKind::Lit(_) => true,
        ExprKind::Cast(_, ty) => primitive_annotation(cx, ty),
        ExprKind::Block(block, None) => block
            .expr
            .is_some_and(|tail| fixed_primitive_source(cx, tail)),
        ExprKind::Path(ref path) => {
            let Res::Local(binding) = cx.qpath_res(path, operand.hir_id) else {
                return false;
            };
            if let Some(ty) = binding_annotation(cx, binding) {
                return primitive_annotation(cx, ty);
            }
            matches!(cx.tcx.parent_hir_node(binding), Node::LetStmt(binding)
                if matches!(binding.pat.kind, PatKind::Binding(_, _, _, None))
                    && binding.init.is_some_and(|init| fixed_primitive_source(cx, init)))
        }
        ExprKind::Call(function, _) => matches!(function.kind, ExprKind::Path(ref path)
            if !matches!(path, QPath::Resolved(None, path) if conditional_import(cx, function.hir_id, path))
                && cx.qpath_res(path, function.hir_id).opt_def_id()
                    .is_some_and(|definition| primitive_output(cx, definition))),
        ExprKind::MethodCall(_, receiver, _, _) => {
            fixed_method_receiver(cx, receiver)
                && cx
                    .typeck_results()
                    .type_dependent_def_id(operand.hir_id)
                    .is_some_and(|definition| primitive_output(cx, definition))
        }
        ExprKind::Field(base, _) => {
            let Some(adt) = cx
                .typeck_results()
                .expr_ty_adjusted(base)
                .peel_refs()
                .ty_adt_def()
            else {
                return false;
            };
            if adt.is_enum() || !adt.did().is_local() {
                return false;
            }
            let Some(field) = cx.typeck_results().opt_field_index(operand.hir_id) else {
                return false;
            };
            let Some(definition) = adt.non_enum_variant().fields[field].did.as_local() else {
                return false;
            };
            !conditional_definition(cx, definition)
                && matches!(cx.tcx.hir_node_by_def_id(definition),
                    Node::Field(field) if primitive_annotation(cx, field.ty))
        }
        _ => false,
    }
}
