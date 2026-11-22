//! Convert positional path calls to their named coordinate contracts.

use clippy_utils::fn_def_id;
use rustc_hir::{Expr, ExprKind, HirId, UnOp, def::{DefKind, Res}, def_id::DefId};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, TypeVisitableExt};

fn paths_root(cx: &LateContext<'_>, owner: DefId) -> Option<&'static str> {
    if owner.is_local() {
        return Some("crate");
    }
    let crate_name = "console_core_paths";
    let in_prelude = cx.sess().opts.externs.get(crate_name).is_some_and(|dep| dep.add_prelude);
    let in_module = cx
        .tcx
        .module_children_local(rustc_hir::def_id::CRATE_DEF_ID)
        .iter()
        .any(|child| {
            child.ident.as_str() == crate_name && child.res.opt_def_id() == Some(owner.krate.as_def_id())
        });
    (in_prelude || in_module).then_some("::console_core_paths")
}

pub(super) fn transform_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    if !matches!(expr.kind, ExprKind::Call(..) | ExprKind::MethodCall(..)) {
        return None;
    }
    let types = cx.typeck_results();
    let old = fn_def_id(cx, expr)?;
    let (name, shape) = match cx.tcx.opt_item_name(old)?.as_str() {
        "from_translate" => ("from_translation", "Point"),
        "pre_translate" => ("pre_translate_by", "Point"),
        "post_translate" => ("post_translate_by", "Point"),
        "from_scale" => ("from_scale_factors", "ScaleFactors"),
        "pre_scale" => ("pre_scale_by", "ScaleFactors"),
        "post_scale" => ("post_scale_by", "ScaleFactors"),
        _ => return None,
    };
    let implementation = cx.tcx.inherent_impl_of_assoc(old)?;
    let owner = cx.tcx.type_of(implementation).instantiate_identity().skip_norm_wip().ty_adt_def()?;
    let owner_id = owner.did();
    if cx.tcx.crate_name(owner_id.krate).as_str() != "console_core_paths"
        || cx.tcx.def_path(owner_id).to_string_no_crate_verbose() != "::transform::Transform"
        || expr.span.from_expansion()
        || !cx.sess().edition().at_least_rust_2018()
        || !super::macro_input::outside_macro_input(cx, expr.span)
    {
        return None;
    }
    let root = paths_root(cx, owner_id)?;
    let map = cx.sess().source_map();
    let (call, x, y, has_receiver) = match expr.kind {
        ExprKind::Call(callee, [x, y]) => {
            let ExprKind::Path(ref path) = callee.kind else { return None };
            if callee.span.from_expansion()
                || !matches!(cx.qpath_res(path, callee.hir_id), Res::Def(DefKind::AssocFn, _))
            {
                return None;
            }
            (format!("{root}::Transform::{name}"), x, y, false)
        }
        ExprKind::MethodCall(_, receiver, [x, y], _) => {
            if receiver.span.from_expansion() || types.expr_ty(receiver).peel_refs().ty_adt_def() != Some(owner) {
                return None;
            }
            let value = map.span_to_snippet(receiver.span).ok()?;
            let call = match receiver.kind {
                ExprKind::Path(_) | ExprKind::Field(..) | ExprKind::Call(..) | ExprKind::MethodCall(..) => {
                    format!("{value}.{name}")
                }
                _ if value.starts_with('(') && value.ends_with(')') => format!("{value}.{name}"),
                _ => format!("({value}).{name}"),
            };
            (call, x, y, true)
        }
        _ => return None,
    };
    let next = cx
        .tcx
        .inherent_impls(owner_id)
        .iter()
        .flat_map(|id| cx.tcx.associated_items(*id).in_definition_order())
        .find(|item| item.opt_name().is_some_and(|item_name| item_name.as_str() == name))?
        .def_id;
    let next_signature = cx.tcx.fn_sig(next).skip_binder();
    let arguments = next_signature.inputs().skip_binder();
    let shape_id = arguments.last()?.ty_adt_def()?.did();
    if arguments.len() != 1 + usize::from(has_receiver)
        || shape_id.krate != owner_id.krate
        || cx.tcx.item_name(shape_id).as_str() != shape
        || !cx.tcx.visibility(next).is_public()
        || next_signature.output().skip_binder() != cx.tcx.fn_sig(old).skip_binder().output().skip_binder()
        || [x, y].iter().any(|arg| arg.span.from_expansion() || types.expr_ty_adjusted(arg) != cx.tcx.types.f32)
    {
        return None;
    }
    let source = map.span_to_snippet(expr.span).ok()?;
    if source.contains("//") || source.contains("/*") {
        return None;
    }
    let x = map.span_to_snippet(x.span).ok()?;
    let y = map.span_to_snippet(y.span).ok()?;
    let x_field = if x == "x" { x } else { format!("x: {x}") };
    let y_field = if y == "y" { y } else { format!("y: {y}") };
    Some(format!("{call}({root}::{shape} {{ {x_field}, {y_field} }})"))
}

// A fully qualified call borrows the builder before reading its arguments.
// Only copy coordinates from independent, aligned places or literals.
fn independent(cx: &LateContext<'_>, expr: &Expr<'_>, builder: HirId) -> bool {
    let types = cx.typeck_results();
    if expr.span.from_expansion() || !types.expr_adjustments(expr).is_empty() {
        return false;
    }
    match expr.kind {
        ExprKind::Lit(_) => true,
        ExprKind::Path(ref path) => {
            matches!(cx.qpath_res(path, expr.hir_id), Res::Local(id) if id != builder)
        }
        ExprKind::Unary(UnOp::Neg, value) => {
            types.expr_ty(value).is_floating_point() && independent(cx, value, builder)
        }
        ExprKind::Field(base, _) => {
            types
                .expr_ty(base)
                .ty_adt_def()
                .is_some_and(|adt| !adt.repr().packed())
                && independent(cx, base, builder)
        }
        _ => false,
    }
}

pub(super) fn drawing_call(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let ExprKind::MethodCall(segment, receiver, args, _) = expr.kind else {
        return None;
    };
    let points = match segment.ident.as_str() {
        "move_to" | "line_to" => 1,
        "quad_to" => 2,
        "cubic_to" => 3,
        _ => return None,
    };
    let types = cx.typeck_results();
    let old = types.type_dependent_def_id(expr.hir_id)?;
    let implementation = cx.tcx.inherent_impl_of_assoc(old)?;
    let builder = cx
        .tcx
        .type_of(implementation)
        .instantiate_identity()
        .skip_norm_wip()
        .ty_adt_def()?;
    let krate = builder.did().krate;
    if cx.tcx.crate_name(krate).as_str() != "console_core_paths"
        || cx.tcx.def_path(builder.did()).to_string_no_crate_verbose()
            != "::path_builder::PathBuilder"
        || args.len() != 2 * points
        || expr.span.from_expansion()
        || !cx.sess().edition().at_least_rust_2018()
    {
        return None;
    }
    let ExprKind::Path(path) = receiver.kind else {
        return None;
    };
    let Res::Local(binding) = cx.qpath_res(&path, receiver.hir_id) else {
        return None;
    };
    if !args
        .iter()
        .all(|arg| types.expr_ty(arg) == cx.tcx.types.f32 && independent(cx, arg, binding))
        || !super::macro_input::outside_macro_input(cx, expr.span)
    {
        return None;
    }
    let borrow = match types.expr_ty(receiver).kind() {
        ty::Adt(adt, _) if *adt == builder => "&mut ",
        ty::Ref(_, inner, ty::Mutability::Mut) if inner.ty_adt_def() == Some(builder) => "&mut *",
        _ => return None,
    };
    let root = paths_root(cx, builder.did())?;
    let exports = if builder.did().is_local() {
        cx.tcx
            .module_children_local(rustc_hir::def_id::CRATE_DEF_ID)
    } else {
        cx.tcx.module_children(krate.as_def_id())
    };
    let point = exports
        .iter()
        .find(|item| item.ident.as_str() == "Point" && item.vis.is_public())?
        .res
        .opt_def_id()?;
    let method = format!("{}_pt", segment.ident);
    let next = cx
        .tcx
        .inherent_impls(builder.did())
        .iter()
        .flat_map(|id| cx.tcx.associated_items(*id).in_definition_order())
        .find(|item| item.opt_name().is_some_and(|name| name.as_str() == method))?
        .def_id;
    let signature = cx.tcx.fn_sig(next).skip_binder();
    let arguments = signature.inputs().skip_binder();
    if !cx.tcx.visibility(next).is_public()
        || arguments.len() != points + 1
        || signature.output().skip_binder().has_escaping_bound_vars()
        || signature.output().skip_binder()
            != cx.tcx.fn_sig(old).skip_binder().output().skip_binder()
        || !arguments
            .iter()
            .skip(1)
            .all(|arg| arg.ty_adt_def().is_some_and(|adt| adt.did() == point))
    {
        return None;
    }
    let map = cx.sess().source_map();
    let source = map.span_to_snippet(expr.span).ok()?;
    if source.contains("//") || source.contains("/*") {
        return None;
    }
    let receiver = map.span_to_snippet(receiver.span).ok()?;
    let mut call = format!("{root}::PathBuilder::{method}({borrow}{receiver}");
    for pair in args.chunks_exact(2) {
        let [x, y] = pair else { return None };
        if let (ExprKind::Field(first, horizontal), ExprKind::Field(second, vertical)) =
            (x.kind, y.kind)
            && horizontal.as_str() == "x"
            && vertical.as_str() == "y"
            && let (ExprKind::Path(first_path), ExprKind::Path(second_path)) =
                (first.kind, second.kind)
            && cx.qpath_res(&first_path, first.hir_id) == cx.qpath_res(&second_path, second.hir_id)
            && types
                .expr_ty(first)
                .ty_adt_def()
                .is_some_and(|adt| adt.did() == point)
            && cx.type_is_copy_modulo_regions(types.expr_ty(first))
        {
            call.push_str(", ");
            call.push_str(&map.span_to_snippet(first.span).ok()?);
            continue;
        }
        let x = map.span_to_snippet(x.span).ok()?;
        let y = map.span_to_snippet(y.span).ok()?;
        call.push_str(&format!(", {root}::Point {{ x: {x}, y: {y} }}"));
    }
    call.push(')');
    Some(call)
}
