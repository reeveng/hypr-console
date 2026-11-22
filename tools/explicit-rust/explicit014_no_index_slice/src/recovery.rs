//! Rewrite array and slice accesses while preserving their use as places.

use std::collections::HashMap;

use clippy_utils::{higher::Range, sym};
use rustc_hir::{
    BindingMode, BorrowKind, Expr, ExprKind, LangItem, Node, PatKind, QPath, UnOp,
    def::{CtorKind, DefKind, Res},
    def_id::DefId,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{
    self,
    adjustment::{Adjust, AutoBorrow, AutoBorrowMutability},
};

use super::fixes::{clean, element_pattern, position};

fn name(cx: &LateContext<'_>, definition: DefId) -> String {
    let path = cx.tcx.def_path(definition).to_string_no_crate_verbose();
    format!("{}{path}", cx.tcx.crate_name(definition.krate))
}

pub(super) fn body_name(cx: &LateContext<'_>, expr: &Expr<'_>) -> String {
    name(cx, cx.tcx.hir_enclosing_body_owner(expr.hir_id).to_def_id())
}

fn bounds_error<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'_>,
    configured: &HashMap<String, String>,
) -> Option<String> {
    if configured.is_empty() {
        return None;
    }
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    let body = cx.tcx.hir_body_owned_by(owner);
    let output = cx.typeck_results().expr_ty(body.value);
    let ty::Adt(result, args) = output.kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(sym::Result, result.did()) {
        return None;
    }
    let error = args.type_at(1);
    if error.is_unit() {
        return (configured.get(&name(cx, owner.to_def_id()))? == "()").then(|| "()".to_owned());
    }
    let ty::Adt(adt, error_args) = error.kind() else {
        return None;
    };
    if !adt.is_enum() || !adt.did().is_local() {
        return None;
    }
    let variant = configured.get(&name(cx, adt.did()))?;
    let variant = adt
        .variants()
        .iter()
        .find(|item| item.name.as_str() == variant)?;
    let path = cx.tcx.def_path(adt.did()).to_string_no_crate_verbose();
    // The local type path is valid from the current module, even with aliases
    // or variants of the same name in scope.
    let constructor = format!("crate{path}::{}", variant.name);
    match (variant.ctor_kind(), variant.fields.raw.as_slice()) {
        (Some(CtorKind::Const), []) => Some(constructor),
        (Some(CtorKind::Fn), [field])
            if matches!(
                field.ty(cx.tcx, error_args).skip_norm_wip().kind(),
                ty::Adt(field_type, _) if cx.tcx.is_lang_item(field_type.did(), LangItem::String)
            ) =>
        {
            Some(format!(
                "{constructor}(::core::convert::From::from(\"index or range outside the buffer\"))"
            ))
        }
        _ => None,
    }
}

fn bounds_failure<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'_>,
    configured_errors: &HashMap<String, String>,
    none_on_bounds: &[String],
) -> Option<String> {
    if !none_on_bounds.is_empty() {
        let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
        let body = cx.tcx.hir_body_owned_by(owner);
        let output = cx.typeck_results().expr_ty(body.value);
        if matches!(output.kind(), ty::Adt(adt, _) if cx.tcx.is_diagnostic_item(sym::Option, adt.did()))
            && none_on_bounds.contains(&body_name(cx, expr))
        {
            return Some("::core::option::Option::None".to_owned());
        }
    }
    let error = bounds_error(cx, expr, configured_errors)?;
    Some(format!("::core::result::Result::Err({error})"))
}

pub(super) fn slice_like(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        ty::Array(..) | ty::Slice(..) => true,
        ty::Adt(adt, _) => cx.tcx.is_diagnostic_item(sym::Vec, adt.did()),
        _ => false,
    }
}

fn standard_index(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> bool {
    if ty.is_usize() {
        return true;
    }
    let ty::Adt(adt, args) = ty.kind() else {
        return false;
    };
    // A local index type can overload `Index` on a standard container.
    cx.tcx.crate_name(adt.did().krate).as_str() == "core"
        && match cx.tcx.item_name(adt.did()).as_str() {
            "RangeFull" => true,
            "Range" | "RangeFrom" | "RangeTo" | "RangeInclusive" | "RangeToInclusive" => {
                args.types().next().is_some_and(ty::Ty::is_usize)
            }
            _ => false,
        }
}

// An index may be part of a larger place. Follow fields and further indexing
// to the use that decides whether to borrow it, write it, or copy its value.
fn mutable(cx: &LateContext<'_>, original: &Expr<'_>) -> Option<bool> {
    let types = cx.typeck_results();
    let mut child = original;
    loop {
        let parent = cx.tcx.parent_hir_node(child.hir_id);
        // A trait assignment can read its own left place while the implicit
        // mutable borrow is reserved. An explicit borrow would activate it.
        if let Node::Expr(parent) = parent
            && let ExprKind::AssignOp(_, lhs, rhs) = parent.kind
            && lhs.hir_id == child.hir_id
            && !(types.expr_ty(lhs).is_primitive() && types.expr_ty(rhs).is_primitive())
        {
            return None;
        }
        for adjustment in types.expr_adjustments(child) {
            if let Adjust::Borrow(AutoBorrow::Ref(mutability)) = adjustment.kind {
                return Some(matches!(mutability, AutoBorrowMutability::Mut { .. }));
            }
        }
        if let Node::Expr(parent) = parent {
            match parent.kind {
                ExprKind::Index(base, _, _) | ExprKind::Field(base, _)
                    if base.hir_id == child.hir_id =>
                {
                    child = parent;
                    continue;
                }
                ExprKind::Unary(UnOp::Deref, base) if base.hir_id == child.hir_id => {
                    match types.expr_ty(child).kind() {
                        ty::Ref(..) => {
                            child = parent;
                            continue;
                        }
                        ty::RawPtr(..) => return Some(false),
                        _ => return None,
                    }
                }
                ExprKind::AddrOf(BorrowKind::Ref, mutability, operand)
                    if operand.hir_id == child.hir_id =>
                {
                    return Some(mutability.is_mut());
                }
                // Preserve the original evaluation order by replacing the
                // whole place in situ, including in a compound assignment.
                ExprKind::Assign(lhs, _, _) | ExprKind::AssignOp(_, lhs, _)
                    if lhs.hir_id == child.hir_id =>
                {
                    return Some(true);
                }
                ExprKind::AddrOf(BorrowKind::Raw, ..) => return None,
                _ => {}
            }
        }
        if let Node::LetStmt(local) = parent
            && let PatKind::Binding(BindingMode(mode, _), _, _, _) = local.pat.kind
            && let rustc_hir::ByRef::Yes(_, mutability) = mode
        {
            return Some(mutability.is_mut());
        }
        return cx
            .type_is_copy_modulo_regions(types.expr_ty(child))
            .then_some(false);
    }
}

pub(super) fn borrow_base(cx: &LateContext<'_>, base: &Expr<'_>, write: bool) -> Option<String> {
    Some(borrow_snippet(cx, base, clean(cx, base.span)?, write))
}

// Replay the selected dereferences. Rust cannot implicitly chain a wrapper's
// Deref-to-array with array-to-slice unsizing at a get/get_mut argument.
fn slice_projection<'tcx>(
    cx: &LateContext<'tcx>,
    base: &Expr<'tcx>,
    write: bool,
) -> Option<(ty::Ty<'tcx>, String)> {
    let mut stars = String::new();
    for adjustment in cx.typeck_results().expr_adjustments(base) {
        if !matches!(adjustment.kind, Adjust::Deref(_)) {
            return None;
        }
        stars.push('*');
        if matches!(adjustment.target.kind(), ty::Array(..) | ty::Slice(..)) {
            let borrow = if write { "&mut " } else { "&" };
            let receiver = operand(base, clean(cx, base.span)?);
            return Some((adjustment.target, format!("{borrow}{stars}{receiver}")));
        }
    }
    None
}

fn operand(base: &Expr<'_>, receiver: String) -> String {
    match base.kind {
        ExprKind::Path(..)
        | ExprKind::Field(..)
        | ExprKind::Index(..)
        | ExprKind::Unary(..)
        | ExprKind::Call(..)
        | ExprKind::MethodCall(..) => receiver,
        _ => format!("({receiver})"),
    }
}

fn borrow_snippet(cx: &LateContext<'_>, base: &Expr<'_>, receiver: String, write: bool) -> String {
    let receiver = operand(base, receiver);
    let mut stars = String::new();
    let mut current = cx.typeck_results().expr_ty(base);
    while let ty::Ref(_, pointee, _) = *current.kind() {
        stars.push('*');
        current = pointee;
    }
    let borrow = if write { "&mut " } else { "&" };
    format!("{borrow}{stars}{receiver}")
}

fn stable_place(expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(_, path)) => match path.res {
            Res::Local(_) => true,
            Res::Def(DefKind::Static { mutability, .. }, _) => !mutability.is_mut(),
            _ => false,
        },
        ExprKind::Field(base, _) | ExprKind::Index(base, _, _) => stable_place(base),
        ExprKind::Lit(literal) => matches!(
            literal.node,
            rustc_ast::LitKind::ByteStr(..) | rustc_ast::LitKind::Str(..)
        ),
        _ => false,
    }
}

fn stable_storage(cx: &LateContext<'_>, base: &Expr<'_>) -> bool {
    // A function that returns a reference already bounds the lifetime by the
    // call. The borrow of an owned temporary can extend past a let statement
    // through indexing, but not through get or a new pattern match.
    stable_place(base)
        || matches!(base.kind, ExprKind::Call(..) | ExprKind::MethodCall(..))
            && cx.typeck_results().expr_ty(base).is_ref()
}

pub(super) fn whole_slice<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(rustc_span::Span, String)> {
    let ExprKind::Index(base, index, _) = expr.kind else {
        return None;
    };
    let range = Range::hir(cx, index)?;
    if range.start.is_some() || range.end.is_some() {
        return None;
    }
    let Node::Expr(borrow) = cx.tcx.parent_hir_node(expr.hir_id) else {
        return None;
    };
    let ExprKind::AddrOf(BorrowKind::Ref, mutability, operand) = borrow.kind else {
        return None;
    };
    // Keep this to existing borrows of stable places. A call returning a new
    // reference would otherwise shorten the lifetime of a borrowed temporary.
    if operand.hir_id != expr.hir_id || !stable_place(base) {
        return None;
    }
    let types = cx.typeck_results();
    if types
        .type_dependent_def_id(expr.hir_id)
        .is_some_and(|id| !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "alloc"))
    {
        return None;
    }
    let value_type = types.expr_ty(base).peel_refs();
    if !slice_like(cx, value_type)
        && !value_type.is_str()
        && !matches!(value_type.kind(), ty::Adt(adt, _) if cx.tcx.is_lang_item(adt.did(), LangItem::String))
    {
        return None;
    }
    clean(cx, borrow.span)?;
    // A primitive byte string produced by `include_bytes!` is static.
    // Restrict expansion spans here to this builtin's input call site.
    let receiver = if base.span.from_expansion() {
        let data = base.span.ctxt().outer_expn_data();
        let definition = data.macro_def_id?;
        if cx.tcx.crate_name(definition.krate).as_str() != "core"
            || cx.tcx.item_name(definition).as_str() != "include_bytes"
        {
            return None;
        }
        clean(cx, data.call_site)?
    } else {
        clean(cx, base.span)?
    };
    let write = mutability.is_mut();
    let receiver = borrow_snippet(cx, base, receiver, write);
    let target = match types.expr_ty(expr).kind() {
        ty::Slice(_) if value_type.is_slice() => return Some((borrow.span, receiver)),
        ty::Slice(_) => "[_]",
        ty::Str if value_type.is_str() => return Some((borrow.span, receiver)),
        ty::Str => "str",
        _ => return None,
    };
    let trait_method = if write { "AsMut" } else { "AsRef" };
    let call = if write { "as_mut" } else { "as_ref" };
    Some((
        borrow.span,
        format!("::core::convert::{trait_method}::<{target}>::{call}({receiver})"),
    ))
}

pub(super) fn dereference(cx: &LateContext<'_>, expr: &Expr<'_>, matched: String) -> String {
    let postfix = matches!(cx.tcx.parent_hir_node(expr.hir_id), Node::Expr(parent) if match parent.kind {
        ExprKind::Field(base, _) | ExprKind::Index(base, _, _) | ExprKind::MethodCall(_, base, _, _)
        | ExprKind::Call(base, _) => base.hir_id == expr.hir_id,
        _ => false,
    });
    if postfix {
        format!("(*{matched})")
    } else {
        format!("*{matched}")
    }
}

pub(super) fn binding_name(cx: &LateContext<'_>, expr: &Expr<'_>, preferred: &str) -> Option<String> {
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    let map = cx.sess().source_map();
    let declaration = map
        .span_to_snippet(cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner)))
        .ok()?;
    let body = map
        .span_to_snippet(cx.tcx.hir_body_owned_by(owner).value.span)
        .ok()?;
    let available = |name: &str| !declaration.contains(name) && !body.contains(name);
    if available(preferred) {
        return Some(preferred.to_owned());
    }
    (2..=64)
        .map(|suffix| format!("{preferred}_{suffix}"))
        .find(|name| available(name))
}

fn shape_known_at_match<'tcx>(cx: &LateContext<'tcx>, mut base: &'tcx Expr<'tcx>) -> bool {
    let types = cx.typeck_results();
    // An unannotated array can acquire its length from a later use. Array
    // patterns need a length during type checking, earlier than `Index` does.
    while let ExprKind::Path(QPath::Resolved(None, path)) = base.kind
        && let Res::Local(binding) = path.res
        && let Node::LetStmt(local) = cx.tcx.parent_hir_node(binding)
        && local.ty.is_none()
    {
        let Some(initializer) = local.init else {
            return false;
        };
        base = initializer;
    }
    let function = match base.kind {
        ExprKind::Call(callee, _) => match types.expr_ty(callee).kind() {
            ty::FnDef(id, _) => Some(*id),
            _ => None,
        },
        ExprKind::MethodCall(..) => types.type_dependent_def_id(base.hir_id),
        _ => None,
    };
    function.is_none_or(|id| {
        let output = cx.tcx.fn_sig(id).skip_binder().output().skip_binder();
        matches!(output.peel_refs().kind(), ty::Array(_, length) if length.try_to_target_usize(cx.tcx).is_some())
    })
}

pub(super) fn fixed_projection<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<String> {
    let ExprKind::Index(base, index, _) = expr.kind else {
        return None;
    };
    let types = cx.typeck_results();
    if !stable_storage(cx, base)
        || types.type_dependent_def_id(expr.hir_id).is_some()
        || !types.expr_ty(index).is_usize()
        || !shape_known_at_match(cx, base)
    {
        return None;
    }
    let write = mutable(cx, expr)?;
    let original = types.expr_ty(base).peel_refs();
    let (projected, reference) = if matches!(original.kind(), ty::Array(..)) {
        (original, borrow_base(cx, base, write)?)
    } else {
        slice_projection(cx, base, write)?
    };
    let ty::Array(_, length) = projected.kind() else { return None; };
    let at = position(index, 8)?;
    let element = binding_name(cx, expr, "element")?;
    let pattern = element_pattern(cx, base, at, length.try_to_target_usize(cx.tcx)?, &element)?;
    clean(cx, expr.span)?;
    Some(dereference(
        cx,
        expr,
        format!("match {reference} {{ {pattern} => {element} }}"),
    ))
}

pub(super) fn replacement<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    configured_errors: &HashMap<String, String>,
    none_on_bounds: &[String],
) -> Option<String> {
    if configured_errors.is_empty() && none_on_bounds.is_empty() {
        return None;
    }
    let ExprKind::Index(base, index, _) = expr.kind else {
        return None;
    };
    if !stable_storage(cx, base) {
        return None;
    }
    let types = cx.typeck_results();
    let base_type = types.expr_ty(base).peel_refs();
    let index_type = types.expr_ty(index);
    // For a wrapper with Deref, use the same dereference chain selected by
    // indexing. A wrapper's own Index implementation retains the wrapper
    // as its adjusted receiver and cannot take this path.
    let adjusted = types.expr_ty_adjusted(base).peel_refs();
    let target = if slice_like(cx, base_type)
        || matches!(adjusted.kind(), ty::Array(..) | ty::Slice(..))
    {
        "<[_]>"
    } else if !index_type.is_usize()
        && (base_type.is_str()
            || matches!(base_type.kind(), ty::Adt(adt, _)
            if cx.tcx.is_lang_item(adt.did(), LangItem::String)))
    {
        if !stable_place(base) {
            return None;
        }
        "::core::primitive::str"
    } else {
        return None;
    };
    if !standard_index(cx, index_type) {
        return None;
    }
    if types
        .type_dependent_def_id(expr.hir_id)
        .is_some_and(|id| !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "alloc"))
    {
        return None;
    }
    let failure = bounds_failure(cx, expr, configured_errors, none_on_bounds)?;
    clean(cx, expr.span)?;
    let write = mutable(cx, expr)?;
    let reference = if target == "<[_]>" && !slice_like(cx, base_type) {
        slice_projection(cx, base, write)?.1
    } else {
        borrow_base(cx, base, write)?
    };
    // HIR gives range constructors desugaring spans. Recover the authored text.
    let index = clean(cx, index.range_span().unwrap_or(index.span))?;
    let method = if write { "get_mut" } else { "get" };
    let element = binding_name(cx, expr, "indexed_value")?;
    let checked = format!(
        "match {target}::{method}({reference}, {index}) {{ ::core::option::Option::Some({element}) => {element}, ::core::option::Option::None => return {failure} }}"
    );
    Some(dereference(cx, expr, checked))
}
