//! Keep machine-width conversions at the existing index/offset boundary.

use super::CONVERSION_CRATE;
use super::operands::{
    earlier_operand_types_local, enumerated_position, literal_arithmetic, source_or_vec_argument,
};
use super::boundary_conversion;
use rustc_ast::ast::{IntTy, LitIntType, LitKind, UintTy};
use clippy_utils::higher::{ForLoop, Range};
use rustc_hir::{
    BinOpKind, BindingMode, Expr, ExprKind, HirId, ItemKind, Node, PatKind, QPath, StmtKind, Ty,
    TyKind, UnOp, UseKind,
    def::{DefKind, Res},
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

pub(super) fn clean(cx: &LateContext<'_>, span: Span) -> Option<String> {
    if span.from_expansion() {
        return None;
    }
    let text = cx.sess().source_map().span_to_snippet(span).ok()?;
    (!text.contains("//") && !text.contains("/*")).then_some(text)
}

fn from_primitive(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    input: &Expr<'_>,
    function: rustc_hir::def_id::DefId,
    width: &str,
) -> Option<Vec<(Span, String)>> {
    let types = cx.typeck_results();
    let conversion_trait = cx.tcx.trait_of_assoc(function)?;
    if !cx.tcx.is_diagnostic_item(rustc_span::sym::From, conversion_trait)
        || !cx.sess().edition().at_least_rust_2018()
        || clippy_utils::is_in_const_context(cx)
    {
        return None;
    }

    let source = types.expr_ty_adjusted(input);
    let (helper, input_ty) = match (width, source.kind(), types.expr_ty(call).kind()) {
        ("usize", ty::Bool | ty::Uint(ty::UintTy::U8), ty::Uint(ty::UintTy::Usize)) => ("index", "u8"),
        ("usize", ty::Uint(ty::UintTy::U16), ty::Uint(ty::UintTy::Usize)) => ("index", "u16"),
        ("isize", ty::Bool | ty::Uint(ty::UintTy::U8), ty::Int(ty::IntTy::Isize)) => ("wrapped_offset", "u8"),
        ("isize", ty::Int(ty::IntTy::I8), ty::Int(ty::IntTy::Isize)) => ("wrapped_offset", "i8"),
        ("isize", ty::Int(ty::IntTy::I16), ty::Int(ty::IntTy::Isize)) => ("wrapped_offset", "i16"),
        _ => return None,
    };
    // Each source fits even on a 16-bit target. The standard conversions
    // from bool have the same 0/1 representation as u8::from.
    if !boundary_conversion::available(cx, helper, 1) {
        return None;
    }
    clean(cx, call.span)?;
    clean(cx, input.span)?;
    let mut opening = format!("::{CONVERSION_CRATE}::{helper}::<::core::primitive::{input_ty}>(");
    let mut closing = ")".to_owned();
    if source.is_bool() {
        opening.push_str("<::core::primitive::u8 as ::core::convert::From<::core::primitive::bool>>::from(");
        closing.push(')');
    }

    let mut edits = Vec::new();
    if let Node::LetStmt(local) = cx.tcx.parent_hir_node(call.hir_id)
        && local.ty.is_none()
        && local.els.is_none()
        && matches!(local.pat.kind, PatKind::Binding(BindingMode::NONE | BindingMode::MUT, _, _, None))
    {
        let binding = clean(cx, local.pat.span)?;
        edits.push((local.pat.span, format!("::core::result::Result::Ok({binding})")));
    } else {
        let (before, after) = boundary_conversion::extraction(cx, call);
        opening.insert_str(0, before);
        closing.push_str(&after);
    }
    edits.push((call.span.with_hi(input.span.lo()), opening));
    edits.push((call.span.with_lo(input.span.hi()), closing));
    Some(edits)
}

// Method syntax must not remove the only constraint on an unsuffixed literal
// or generic return type. Parameters and concrete function returns already
// have a type independently of the operation that consumes them.
fn independently_typed(cx: &LateContext<'_>, expr: &rustc_hir::Expr<'_>) -> bool {
    has_type_anchor(cx, expr, 16)
}

pub(super) fn concrete_annotation(ty: &Ty<'_>) -> bool {
    matches!(
        ty.kind,
        TyKind::Path(QPath::Resolved(None, path))
            if path.segments.iter().all(|segment| segment.args.is_none())
                && matches!(path.res, Res::PrimTy(_) | Res::Def(DefKind::TyAlias, _))
    )
}

fn receiver_delimiters(expression: &Expr<'_>) -> (&'static str, &'static str) {
    match expression.kind {
        ExprKind::Path(..) | ExprKind::Call(..) | ExprKind::MethodCall(..)
        | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Lit(..) => ("", ""),
        _ => ("(", ")"),
    }
}

pub(super) fn has_block_glob(cx: &LateContext<'_>, site: HirId) -> bool {
    cx.tcx.hir_parent_iter(site).any(|(_, node)| {
        matches!(node, Node::Block(block) if block.stmts.iter().any(|stmt| {
            matches!(stmt.kind, StmtKind::Item(item)
                if matches!(cx.tcx.hir_item(item).kind, ItemKind::Use(_, UseKind::Glob)))
        }))
    })
}

fn literal_shift_receiver(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    receiver: &Expr<'_>,
    distance: &Expr<'_>,
    method: &str,
) -> Option<Vec<(Span, String)>> {
    if !matches!(method, "wrapping_shl" | "wrapping_shr")
        || !cx.sess().edition().at_least_rust_2018()
        || clippy_utils::is_in_const_context(cx)
        || !source_or_vec_argument(cx, call)
    {
        return None;
    }
    let ExprKind::Lit(literal) = receiver.kind else {
        return None;
    };
    let LitKind::Int(value, LitIntType::Unsuffixed) = literal.node else {
        return None;
    };
    let (helper, fixed) = match cx.typeck_results().expr_ty(receiver).kind() {
        ty::Uint(ty::UintTy::Usize) if value.get() <= u128::from(u16::MAX) => ("index", "u16"),
        ty::Int(ty::IntTy::Isize) if value.get() <= i16::MAX as u128 => ("wrapped_offset", "i16"),
        _ => return None,
    };
    if !boundary_conversion::available(cx, helper, 1) || has_block_glob(cx, call.hir_id) {
        return None;
    }
    let file = cx.sess().source_map().lookup_source_file(call.span.lo());
    let source = file.src.as_ref()?;
    let module = cx.tcx.parent_module(call.hir_id).to_local_def_id();
    let children = cx.tcx.module_children_local(module);
    let mut binding = "native_shift_input".to_owned();
    while source.contains(&binding)
        || children.iter().any(|child| child.ident.as_str() == binding)
    {
        binding.push('_');
    }
    // Convert the literal before shifting. A native wrapping shift reduces
    // the distance modulo the target's bit width.
    let opening = format!("(match ::{CONVERSION_CRATE}::{helper}::<::core::primitive::{fixed}>(");
    let between = format!(") {{ ::core::result::Result::Ok({binding}) => {binding} }}).{method}(");
    Some(vec![
        (call.span.with_hi(receiver.span.lo()), opening),
        (receiver.span.between(distance.span), between),
        (call.span.with_lo(distance.span.hi()), ")".to_owned()),
    ])
}

fn has_type_anchor(cx: &LateContext<'_>, expr: &rustc_hir::Expr<'_>, budget: u32) -> bool {
    let Some(next) = budget.checked_sub(1) else {
        return false;
    };
    let types = cx.typeck_results();
    let id = match expr.kind {
        ExprKind::Path(ref path) => {
            return match cx.qpath_res(path, expr.hir_id) {
                Res::Local(binding) => {
                    let parent = cx
                        .tcx
                        .hir_parent_iter(binding)
                        .map(|(_, node)| node)
                        .find(|node| !matches!(node, Node::Pat(_) | Node::PatField(_)));
                    match parent {
                        Some(Node::Param(param)) => {
                            let owner = cx.tcx.hir_enclosing_body_owner(binding);
                            if matches!(cx.tcx.def_kind(owner), DefKind::Fn | DefKind::AssocFn) {
                                return true;
                            }
                            let Node::Expr(owner) = cx.tcx.hir_node_by_def_id(owner) else {
                                return false;
                            };
                            let ExprKind::Closure(closure) = owner.kind else {
                                return false;
                            };
                            cx.tcx.hir_body(closure.body).params.iter()
                                .zip(closure.fn_decl.inputs)
                                .any(|(input, ty)| input.hir_id == param.hir_id && concrete_annotation(ty))
                        }
                        Some(Node::LetStmt(local)) if binding.owner == types.hir_owner => {
                            // An annotation such as `Alias<_>` can still depend on the old call.
                            if local.ty.is_some_and(concrete_annotation) {
                                return true;
                            }
                            local.init.is_some_and(|init| {
                                if let PatKind::TupleStruct(ref path, [field], _) = local.pat.kind
                                    && field.hir_id == binding
                                    && let Some(ctor) = cx.qpath_res(path, local.pat.hir_id).opt_def_id()
                                    && [rustc_hir::LangItem::ResultOk, rustc_hir::LangItem::ResultErr,
                                        rustc_hir::LangItem::OptionSome].into_iter()
                                        .any(|item| clippy_utils::is_lang_item_or_ctor(cx, ctor, item))
                                {
                                    return has_type_anchor(cx, init, next);
                                }
                                if let (PatKind::Tuple(fields, rest), ExprKind::Tup(values)) =
                                    (local.pat.kind, init.kind)
                                    && rest.as_opt_usize().is_none()
                                    && fields.len() == values.len()
                                    && let Some((_, value)) = fields.iter().zip(values)
                                        .find(|(field, _)| field.hir_id == binding)
                                    && types.expr_ty(value) == types.expr_ty(expr)
                                    && types.expr_adjustments(value).is_empty()
                                {
                                    return has_type_anchor(cx, value, next);
                                }
                                types.expr_ty(init) == types.node_type(local.pat.hir_id)
                                    && types.expr_adjustments(init).is_empty()
                                    && has_type_anchor(cx, init, next)
                            })
                        }
                        Some(Node::Arm(_)) => cx.tcx.hir_parent_iter(binding).any(|(_, node)| {
                            let Node::Expr(parent) = node else { return false };
                            let Some(walk) = ForLoop::hir(parent) else { return false };
                            if enumerated_position(cx, &walk, binding) {
                                return source_or_vec_argument(cx, expr);
                            }
                            if walk.pat.hir_id != binding {
                                return false;
                            }
                            let Some(range) = Range::hir(cx, walk.arg) else { return false };
                            [range.start, range.end].into_iter().flatten().any(|end| {
                                types.expr_ty(end) == types.expr_ty(expr)
                                    && has_type_anchor(cx, end, next / 2)
                            })
                        }),
                        _ => false,
                    }
                }
                Res::Def(
                    DefKind::Const { .. } | DefKind::Static { .. } | DefKind::ConstParam,
                    _,
                ) => true,
                _ => false,
            };
        }
        ExprKind::Call(func, _) => match types.expr_ty(func).kind() {
            ty::FnDef(id, _) => *id,
            _ => return false,
        },
        ExprKind::MethodCall(..) => match types.type_dependent_def_id(expr.hir_id) {
            Some(id) => id,
            None => return false,
        },
        ExprKind::Block(block, None) => {
            return block.expr.is_some_and(|tail| {
                types.expr_ty(tail) == types.expr_ty(expr)
                    && types.expr_adjustments(tail).is_empty()
                    && has_type_anchor(cx, tail, next)
            });
        }
        ExprKind::Lit(literal) => return matches!(
            literal.node,
            LitKind::Int(_, LitIntType::Unsigned(UintTy::Usize) | LitIntType::Signed(IntTy::Isize))
        ),
        ExprKind::Match(_, arms, rustc_hir::MatchSource::Normal)
            if clean(cx, expr.span).is_some_and(|source| !source.contains('#')) =>
        {
            let output = types.expr_ty(expr);
            return arms.iter().any(|arm| {
                types.expr_ty(arm.body) == output
                    && types.expr_adjustments(arm.body).is_empty()
                    && has_type_anchor(cx, arm.body, next / 2)
            });
        }
        ExprKind::Index(base, _, _) if matches!(
            types.expr_ty(base).peel_refs().kind(), ty::Array(..) | ty::Slice(..)
        ) => return has_type_anchor(cx, base, next),
        ExprKind::Cast(_, written) => return super::written_pointer_sized(written).is_some(),
        ExprKind::Unary(UnOp::Neg | UnOp::Not, operand)
            if types.expr_ty(operand) == types.expr_ty(expr) && types.expr_ty(expr).is_integral() =>
        {
            return has_type_anchor(cx, operand, next);
        }
        ExprKind::Unary(UnOp::Deref, operand) | ExprKind::AddrOf(_, _, operand) => {
            let (reference, pointee) = match expr.kind {
                ExprKind::Unary(..) => (types.expr_ty(operand), types.expr_ty(expr)),
                _ => (types.expr_ty(expr), types.expr_ty(operand)),
            };
            return matches!(reference.kind(), ty::Ref(_, held, _) if *held == pointee)
                && types.expr_adjustments(operand).is_empty()
                && has_type_anchor(cx, operand, next);
        }
        ExprKind::Field(base, _) => {
            let ty::Adt(adt, _) = types.expr_ty_adjusted(base).peel_refs().kind() else {
                return false;
            };
            let Some(index) = types.opt_field_index(expr.hir_id) else {
                return false;
            };
            return adt.is_struct() && adt.non_enum_variant().fields.get(index).is_some_and(|field| {
                cx.tcx.type_of(field.did).instantiate_identity().skip_norm_wip() == types.expr_ty(expr)
            });
        }
        ExprKind::Binary(op, left, right) if types.expr_ty(left).is_integral() && types.expr_ty(right).is_integral() => {
            let output = types.expr_ty(expr);
            return (types.expr_ty(left) == output && has_type_anchor(cx, left, next / 2))
                || (!matches!(op.node, BinOpKind::Shl | BinOpKind::Shr)
                    && types.expr_ty(right) == output && has_type_anchor(cx, right, next / 2));
        }
        _ => return false,
    };
    cx.tcx.fn_sig(id).skip_binder().output().skip_binder() == types.expr_ty(expr)
}

pub(super) fn replacement(
    cx: &LateContext<'_>,
    target: &Ty<'_>,
    width: &str,
) -> Option<Vec<(Span, String)>> {
    let wrapped = match width {
        "usize" => "wrapped_index",
        "isize" => "wrapped_offset",
        _ => return None,
    };
    let expr = cx
        .tcx
        .hir_parent_iter(target.hir_id)
        .find_map(|(_, node)| match node {
            Node::Expr(e) => Some(e),
            _ => None,
        })?;
    match expr.kind {
        ExprKind::Path(QPath::TypeRelative(receiver_ty, segment))
            if receiver_ty.hir_id == target.hir_id =>
        {
            let Node::Expr(call) = cx.tcx.parent_hir_node(expr.hir_id) else {
                return None;
            };
            let ExprKind::Call(func, [receiver, args @ ..]) = call.kind else {
                return None;
            };
            if func.hir_id != expr.hir_id {
                return None;
            }
            let name = segment.ident.name.as_str();
            let types = cx.typeck_results();
            let ty::FnDef(id, _) = *types.expr_ty(func).kind() else {
                return None;
            };
            if name == "try_from" && args.is_empty() {
                let helper = match width {
                    "usize" => "checked_index",
                    "isize" => "checked_offset",
                    _ => return None,
                };
                let trait_id = cx.tcx.trait_of_assoc(id)?;
                if !types.expr_ty_adjusted(receiver).is_integral()
                    || !cx.tcx.is_diagnostic_item(rustc_span::sym::TryFrom, trait_id)
                    || !cx.sess().edition().at_least_rust_2018()
                    || clippy_utils::is_in_const_context(cx)
                    || !source_or_vec_argument(cx, call)
                    || !boundary_conversion::available(cx, helper, 1)
                {
                    return None;
                }
                clean(cx, func.span)?;
                return Some(vec![(func.span, format!("::{CONVERSION_CRATE}::{helper}"))]);
            }
            if name == "from" && args.is_empty() {
                return from_primitive(cx, call, receiver, id, width);
            }
            match (name, args) {
                ("cast_signed" | "cast_unsigned" | "wrapping_neg", []) => {}
                ("wrapping_add" | "wrapping_sub" | "wrapping_mul" | "wrapping_div"
                    | "wrapping_rem" | "wrapping_shl" | "wrapping_shr", [rhs]) => {
                    clean(cx, rhs.span)?;
                }
                _ => return None,
            };
            let implementation = cx.tcx.inherent_impl_of_assoc(id)?;
            if cx.tcx.crate_name(id.krate).as_str() != "core"
                || cx
                    .tcx
                    .type_of(implementation)
                    .instantiate_identity()
                    .skip_norm_wip()
                    != types.expr_ty(receiver)
            {
                return None;
            }
            clean(cx, call.span)?;
            clean(cx, receiver.span)?;
            let prefix = call.span.with_hi(receiver.span.lo());
            if !independently_typed(cx, receiver) && !earlier_operand_types_local(cx, call, receiver, id) {
                if let Some(edits) = super::bounded_ranges::replacement(cx, call, receiver) {
                    return Some(edits);
                }
                let [rhs] = args else { return None };
                if let Some(edit) = literal_arithmetic(cx, call, receiver, rhs, name) {
                    return Some(vec![edit]);
                }
                if let Some(edits) = literal_shift_receiver(cx, call, receiver, rhs, name) {
                    return Some(edits);
                }
                let operator = match name {
                    "wrapping_add" => "+",
                    "wrapping_sub" => "-",
                    "wrapping_mul" => "*",
                    _ => return None,
                };
                if clippy_utils::is_in_const_context(cx)
                    || !cx.sess().edition().at_least_rust_2018()
                    || types.expr_ty(rhs) != types.expr_ty(receiver)
                    || !independently_typed(cx, rhs)
                {
                    return None;
                }
                if operator != "-" && matches!(receiver.kind, ExprKind::Lit(_)) {
                    // Reading a literal has no effects, so the operand with
                    // a known type can be the receiver of the commutative call.
                    let literal = clean(cx, receiver.span)?;
                    let (opening, closing) = receiver_delimiters(rhs);
                    return Some(vec![
                        (call.span.with_hi(rhs.span.lo()), opening.to_owned()),
                        (call.span.with_lo(rhs.span.hi()), format!("{closing}.{name}({literal})")),
                    ]);
                }
                // The right operand fixes the width. Wrapping preserves
                // the original order and works for noncommutative subtraction.
                return Some(vec![
                    (prefix, "(::core::num::Wrapping(".to_owned()),
                    (receiver.span.between(rhs.span), format!(") {operator} ::core::num::Wrapping(")),
                    (call.span.with_lo(rhs.span.hi()), ")).0".to_owned()),
                ]);
            }
            let (opening, closing) = receiver_delimiters(receiver);
            // Leave the operands' spans available for other compiler fixes.
            let mut edits = vec![(prefix, opening.to_owned())];
            match args {
                [rhs] => {
                    edits.push((receiver.span.between(rhs.span), format!("{closing}.{name}(")));
                    edits.push((call.span.with_lo(rhs.span.hi()), ")".to_owned()));
                }
                _ => edits.push((call.span.with_lo(receiver.span.hi()), format!("{closing}.{name}()"))),
            }
            Some(edits)
        }
        ExprKind::Cast(value, written) if written.hir_id == target.hir_id => {
            if cx.typeck_results().expr_ty(value).is_floating_point() {
                return super::float_conversion::edits(cx, expr, value, written);
            }
            if clippy_utils::is_in_const_context(cx) || !cx.sess().edition().at_least_rust_2018() {
                return None;
            }
            let source = cx.typeck_results().expr_ty(value);
            if !matches!(source.kind(), ty::Int(t) if t.bit_width().is_some())
                && !matches!(source.kind(), ty::Uint(t) if t.bit_width().is_some())
            {
                return None;
            }
            if !boundary_conversion::available(cx, wrapped, 1) {
                return None;
            }
            clean(cx, expr.span)?;
            clean(cx, value.span)?;
            let (opening, closing) = boundary_conversion::extraction(cx, expr);
            Some(vec![
                (expr.span.with_hi(value.span.lo()), format!("{opening}::{CONVERSION_CRATE}::{wrapped}(")),
                (expr.span.with_lo(value.span.hi()), format!("){closing}")),
            ])
        }
        ExprKind::Path(QPath::Resolved(Some(destination), path))
            if destination.hir_id == target.hir_id && path.segments.last()?.ident.as_str() == "from" =>
        {
            let Node::Expr(call) = cx.tcx.parent_hir_node(expr.hir_id) else { return None };
            let ExprKind::Call(callee, [input]) = call.kind else { return None };
            if callee.hir_id != expr.hir_id || !source_or_vec_argument(cx, call) {
                return None;
            }
            from_primitive(cx, call, input, path.res.opt_def_id()?, width)
        }
        ExprKind::Path(QPath::Resolved(_, path)) => {
            let [.., root, function] = path.segments else {
                return None;
            };
            let Res::Def(DefKind::Mod, root) = root.res else {
                return None;
            };
            let Res::Def(DefKind::Fn, id) = path.res else {
                return None;
            };
            if id.krate != root.krate {
                return None;
            }
            let name = match (cx.tcx.item_name(id).as_str(), width) {
                ("fitted", "usize") => "index",
                ("wrapped", _) => wrapped,
                _ => return None,
            };
            let args = function.args?;
            let [source, destination] = args.args else {
                return None;
            };
            if destination.hir_id() != target.hir_id || !boundary_conversion::exported(cx, root, name, 1) {
                return None;
            }
            let span = function.ident.span.to(args.span_ext);
            clean(cx, span)?;
            let source = clean(cx, source.span())?;
            let edit = if source == "_" {
                name.to_owned()
            } else {
                format!("{name}::<{source}>")
            };
            Some(vec![(span, edit)])
        }
        _ => None,
    }
}
