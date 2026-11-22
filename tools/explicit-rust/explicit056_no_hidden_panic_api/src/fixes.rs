use clippy_utils::eq_expr_value;
use clippy_utils::source::indent_of;
use clippy_utils::sym;
use rustc_ast::LitKind;
use rustc_hir::{ConstArg, ConstArgKind, Expr, ExprKind, Node, TyKind, UnOp, def::Res};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{
    self,
    adjustment::{Adjust, DerefAdjustKind},
};

fn clean(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    if expr.span.from_expansion() {
        return None;
    }
    let text = cx.sess().source_map().span_to_snippet(expr.span).ok()?;
    (!text.contains("//") && !text.contains("/*")).then_some(text)
}

fn function_source(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id);
    let declaration = cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner));
    let body = cx.tcx.hir_body_owned_by(owner).value.span;
    if declaration.from_expansion() || !body.contains(expr.span) {
        return None;
    }
    let map = cx.sess().source_map();
    let declaration = map.span_to_snippet(declaration).ok()?;
    let body = map.span_to_snippet(body).ok()?;
    Some(format!("{declaration}\n{body}"))
}

fn fresh_name(source: &str, base: &str) -> String {
    let mut name = base.to_owned();
    while source.contains(&name) {
        name.push('_');
    }
    name
}

fn slice_method(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    let method = cx.typeck_results().type_dependent_def_id(call.hir_id);
    method
        .filter(|id| cx.tcx.crate_name(id.krate).as_str() == "core")
        .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
        .is_some_and(|id| {
            cx.tcx
                .type_of(id)
                .instantiate_identity()
                .skip_norm_wip()
                .is_slice()
        })
}

fn integer_literal(expr: &Expr<'_>) -> Option<u64> {
    if expr.span.from_expansion() {
        return None;
    }
    let ExprKind::Lit(lit) = expr.kind else {
        return None;
    };
    let LitKind::Int(value, _) = lit.node else {
        return None;
    };
    u64::try_from(value.get()).ok()
}

fn array_length_literal(cx: &LateContext<'_>, length: &ConstArg<'_>) -> Option<u64> {
    if length.span.from_expansion() {
        return None;
    }
    match length.kind {
        ConstArgKind::Literal {
            lit: LitKind::Int(value, _),
            negated: false,
        } => u64::try_from(value.get()).ok(),
        ConstArgKind::Anon(anon) => integer_literal(cx.tcx.hir_body(anon.body).value),
        _ => None,
    }
}

fn array_type_length(cx: &LateContext<'_>, mut ty: &rustc_hir::Ty<'_>) -> Option<u64> {
    while let TyKind::Ref(_, inner) = ty.kind {
        ty = inner.ty;
    }
    let TyKind::Array(_, length) = ty.kind else {
        return None;
    };
    array_length_literal(cx, length)
}

// A length resolved only on the active target cannot justify editing source
// that is also compiled under other configurations.
fn written_array_length(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<u64> {
    let ExprKind::Path(ref path) = receiver.kind else {
        return None;
    };
    let Res::Local(binding) = cx.qpath_res(path, receiver.hir_id) else {
        return None;
    };
    let owner = cx.tcx.hir_enclosing_body_owner(binding);
    if owner != cx.tcx.hir_enclosing_body_owner(receiver.hir_id) {
        return None;
    }
    match cx.tcx.parent_hir_node(binding) {
        Node::LetStmt(local) if local.pat.hir_id == binding => match local.ty {
            Some(ty) => array_type_length(cx, ty),
            None => {
                let init = local.init?;
                if init.span.from_expansion() {
                    return None;
                }
                match init.kind {
                    ExprKind::Array(items) => u64::try_from(items.len()).ok(),
                    ExprKind::Repeat(_, length) => array_length_literal(cx, length),
                    _ => None,
                }
            }
        },
        Node::Param(param) if param.pat.hir_id == binding => {
            let body = cx.tcx.hir_body_owned_by(owner);
            let index = body
                .params
                .iter()
                .position(|input| input.hir_id == param.hir_id)?;
            let declaration = cx
                .tcx
                .hir_fn_decl_by_hir_id(cx.tcx.local_def_id_to_hir_id(owner))?;
            array_type_length(cx, declaration.inputs.get(index)?)
        }
        _ => None,
    }
}

pub(super) fn fixed_array_swap(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    receiver: &Expr<'_>,
    left: &Expr<'_>,
    right: &Expr<'_>,
) -> Option<String> {
    if !slice_method(cx, call) || !matches!(cx.tcx.parent_hir_node(call.hir_id), Node::Stmt(_)) {
        return None;
    }
    let source = function_source(cx, call)?;
    // Configuration attributes could choose another declaration or remove
    // elements in an inferred array literal.
    if source.contains('#') {
        return None;
    }
    let length = written_array_length(cx, receiver)?;
    let mut ty = cx.typeck_results().expr_ty(receiver);
    let mut stars = String::new();
    while let ty::Ref(_, pointee, _) = ty.kind() {
        ty = *pointee;
        stars.push('*');
    }
    let ty::Array(_, actual) = ty.kind() else {
        return None;
    };
    if actual.try_to_target_usize(cx.tcx)? != length {
        return None;
    }
    let left = integer_literal(left)?;
    let right = integer_literal(right)?;
    let first = left.min(right);
    let second = left.max(right);
    if first == second || second >= length || second > 16 {
        return None;
    }
    clean(cx, call)?;
    let base = clean(cx, receiver)?;
    let first_name = fresh_name(&source, "swap_first");
    let second_name = fresh_name(&source, "swap_second");
    let prefix = "_, ".repeat(usize::try_from(first).ok()?);
    let gap = "_, ".repeat(usize::try_from(second.checked_sub(first)?.checked_sub(1)?).ok()?);
    let indent = " ".repeat(indent_of(cx, call.span)?);
    Some(format!(
        "{{\n{indent}  let [{prefix}{first_name}, {gap}{second_name}, ..] = &mut {stars}{base};\n{indent}  ::core::mem::swap({first_name}, {second_name});\n{indent}}}"
    ))
}

// Keep only places that can be read again without a call or overloaded
// dereference. The split result still borrows the original list.
fn repeatable_place(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    if expr.span.from_expansion() {
        return false;
    }
    let types = cx.typeck_results();
    match expr.kind {
        ExprKind::Path(ref path) => matches!(cx.qpath_res(path, expr.hir_id), Res::Local(_)),
        ExprKind::Field(base, _) => {
            types.expr_adjustments(base).iter().all(|adjustment| {
                matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin))
            }) && repeatable_place(cx, base)
        }
        ExprKind::Unary(UnOp::Deref, base) => {
            types.expr_ty(base).is_ref() && repeatable_place(cx, base)
        }
        _ => false,
    }
}

fn min_operands<'hir>(
    cx: &LateContext<'_>,
    expr: &Expr<'hir>,
) -> Option<(&'hir Expr<'hir>, &'hir Expr<'hir>)> {
    let types = cx.typeck_results();
    if types.expr_ty(expr) != cx.tcx.types.usize {
        return None;
    }
    match expr.kind {
        ExprKind::MethodCall(_, left, [right], _) => {
            let method = types.type_dependent_def_id(expr.hir_id)?;
            let ord = cx.tcx.trait_of_assoc(method)?;
            (cx.tcx.item_name(method) == sym::min && cx.tcx.is_diagnostic_item(sym::Ord, ord))
                .then_some((left, right))
        }
        ExprKind::Call(callee, [left, right]) => match types.expr_ty(callee).kind() {
            ty::FnDef(method, _) if cx.tcx.is_diagnostic_item(sym::cmp_min, *method) => {
                Some((left, right))
            }
            _ => None,
        },
        _ => None,
    }
}

fn length_of(cx: &LateContext<'_>, expr: &Expr<'_>, original: &Expr<'_>) -> bool {
    let ExprKind::MethodCall(method, receiver, [], _) = expr.kind else {
        return false;
    };
    if method.ident.name != sym::len
        || !repeatable_place(cx, receiver)
        || !eq_expr_value(cx, expr.span.ctxt(), receiver, original)
    {
        return false;
    }
    if slice_method(cx, expr) {
        return true;
    }
    cx.typeck_results()
        .type_dependent_def_id(expr.hir_id)
        .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
        .is_some_and(|id| {
            matches!(cx.tcx.type_of(id).instantiate_identity().skip_norm_wip().kind(),
                ty::Adt(adt, _) if cx.tcx.is_diagnostic_item(sym::Vec, adt.did()))
        })
}

fn shared_slice(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<String> {
    let text = clean(cx, receiver)?;
    if cx.typeck_results().expr_ty(receiver).is_ref() {
        Some(text)
    } else {
        Some(format!("&{text}"))
    }
}

pub(super) fn clamped_slice_split(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    receiver: &Expr<'_>,
    clamped: &Expr<'_>,
) -> Option<String> {
    if !slice_method(cx, call)
        || !repeatable_place(cx, receiver)
        || !matches!(cx.tcx.parent_hir_node(call.hir_id), Node::LetStmt(local) if local.els.is_none())
    {
        return None;
    }
    clean(cx, call)?;
    let (left, right) = min_operands(cx, clamped)?;
    let index = if length_of(cx, right, receiver) {
        left
    } else if length_of(cx, left, receiver) {
        right
    } else {
        return None;
    };
    let index = clean(cx, index)?;
    let slice = shared_slice(cx, receiver)?;
    let source = function_source(cx, call)?;
    let halves = fresh_name(&source, "split_halves");
    let whole = fresh_name(&source, "split_whole");
    let empty = fresh_name(&source, "split_empty");
    let indent = " ".repeat(indent_of(cx, call.span)?);
    // Chunks of one leave an empty remainder at the end of the same
    // allocation. A literal empty slice could have a different address.
    Some(format!(
        "match <[_]>::split_at_checked({slice}, {index}) {{\n{indent}  \
         ::core::option::Option::Some({halves}) => {halves},\n{indent}  \
         ::core::option::Option::None => {{\n{indent}    \
         let ({whole}, {empty}) = <[_]>::as_chunks::<1>({slice});\n{indent}    \
         ({whole}.as_flattened(), {empty})\n{indent}  \
         }}\n{indent}}}"
    ))
}
