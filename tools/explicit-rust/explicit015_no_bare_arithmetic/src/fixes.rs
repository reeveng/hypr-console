//! An arithmetic fix is automatic only when its operands prove the operator cannot fail.
//! An explicit wrapping policy also permits rewrites without a range proof.

use super::boundary_conversion;
use clippy_utils::consts::{ConstEvalCtxt, Constant};
use clippy_utils::higher::{ForLoop, Range};
use clippy_utils::is_lang_item_or_ctor;
use clippy_utils::sym::{cmp_max, cmp_min};
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::{BinOpKind, BindingMode, Expr, ExprKind, LangItem, Node, PatKind, UnOp};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;

struct Integer {
    name: String,
    bits: u32,
    unsigned: bool,
    maximum: u128,
}

impl Integer {
    fn portable_bits(&self) -> u32 {
        match self.name.as_str() {
            "usize" | "isize" => 16,
            _ => self.bits,
        }
    }

    fn portable_maximum(&self) -> u128 {
        match self.name.as_str() {
            "usize" => u128::from(u16::MAX),
            "isize" => u128::from(i16::MAX.unsigned_abs()),
            _ => self.maximum,
        }
    }
}

fn integer(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Integer> {
    let kind = cx.typeck_results().expr_ty(expression).peel_refs();
    let pointer = cx.tcx.data_layout.pointer_size().bits();
    let (bits, unsigned) = match kind.kind() {
        ty::Int(kind) => (kind.bit_width().unwrap_or(pointer), false),
        ty::Uint(kind) => (kind.bit_width().unwrap_or(pointer), true),
        _ => return None,
    };
    let bits = u32::try_from(bits).ok()?;
    let shift = if unsigned { 128_u32 } else { 129_u32 }.checked_sub(bits)?;
    let maximum = u128::MAX.checked_shr(shift)?;

    Some(Integer {
        name: kind.to_string(),
        bits,
        unsigned,
        maximum,
    })
}

fn constant(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<u128> {
    // Values read from other items or macros can change with the build configuration.
    let Constant::Int(value) =
        ConstEvalCtxt::new(cx).eval_local(expression, rustc_span::SyntaxContext::root())?
    else {
        return None;
    };

    (value <= integer(cx, expression)?.maximum).then_some(value)
}

fn pure_constant(expression: &Expr<'_>) -> bool {
    matches!(expression.kind, ExprKind::Lit(_))
}

fn result_bounds(op: BinOpKind, left: (u128, u128), right: (u128, u128)) -> Option<(u128, u128)> {
    match op {
        BinOpKind::Add => Some((left.0.checked_add(right.0)?, left.1.checked_add(right.1)?)),
        BinOpKind::Sub => Some((left.0.checked_sub(right.1)?, left.1.checked_sub(right.0)?)),
        BinOpKind::Mul => Some((left.0.checked_mul(right.0)?, left.1.checked_mul(right.1)?)),
        BinOpKind::Div => Some((left.0.checked_div(right.1)?, left.1.checked_div(right.0)?)),
        BinOpKind::Rem if right.0 > 0 => Some((0, left.1.min(right.1 - 1))),
        _ => None,
    }
}

fn primitive_impl<'tcx>(cx: &LateContext<'tcx>, id: DefId) -> Option<ty::Ty<'tcx>> {
    if cx.tcx.crate_name(id.krate).as_str() != "core" {
        return None;
    }
    let implementation = cx.tcx.inherent_impl_of_assoc(id)?;
    let kind = cx.tcx.type_of(implementation).instantiate_identity().skip_norm_wip();
    kind.is_integral().then_some(kind)
}

fn fixed_width(kind: ty::Ty<'_>) -> Option<u64> {
    match kind.kind() {
        ty::Int(number) => number.bit_width(),
        ty::Uint(number) => number.bit_width(),
        _ => None,
    }
}

fn named_arithmetic(cx: &LateContext<'_>, id: DefId) -> Option<BinOpKind> {
    primitive_impl(cx, id)?;
    match cx.tcx.item_name(id).as_str() {
        "wrapping_add" => Some(BinOpKind::Add),
        "wrapping_sub" => Some(BinOpKind::Sub),
        "wrapping_mul" => Some(BinOpKind::Mul),
        "wrapping_div" => Some(BinOpKind::Div),
        "wrapping_rem" => Some(BinOpKind::Rem),
        _ => None,
    }
}

fn call_bounds(
    cx: &LateContext<'_>,
    id: DefId,
    args: &[&Expr<'_>],
    kind: &Integer,
    budget: u32,
) -> Option<(u128, u128)> {
    if cx.tcx.crate_name(id.krate).as_str() != "core" {
        return None;
    }
    let name = cx.tcx.item_name(id);
    if let Some(primitive) = primitive_impl(cx, id) {
        match (name.as_str(), args) {
            ("leading_zeros" | "trailing_zeros" | "leading_ones" | "trailing_ones"
                | "count_zeros" | "count_ones", [_]) => {
                return Some((0, u128::from(fixed_width(primitive)?)));
            }
            ("wrapping_shl" | "wrapping_shr", [left, right]) if kind.unsigned => {
                let left_shift = name.as_str() == "wrapping_shl";
                if let Some(shift) = constant(cx, right).and_then(|n| u32::try_from(n).ok())
                    && shift < kind.portable_bits()
                    && let Some((low, high)) = bounds(cx, left, budget)
                {
                    if left_shift && high <= kind.portable_maximum() >> shift {
                        return Some((low << shift, high << shift));
                    }
                    if !left_shift && high <= kind.portable_maximum() {
                        return Some((low >> shift, high >> shift));
                    }
                }
                // A wrapping left shift masks its count to 0..BITS. If bit
                // zero was set, it survives at the masked position, even if
                // higher bits are lost or the count exceeds the word width.
                return (left_shift && constant(cx, left).is_some_and(|n| n & 1 == 1))
                    .then_some((1, kind.maximum));
            }
            _ => {}
        }
    }
    if let [receiver] = args
        && name.as_str() == "get"
        && kind.unsigned
        && cx.tcx.inherent_impl_of_assoc(id).is_some()
        && super::is_non_zero(cx, receiver)
    {
        return Some((1, kind.maximum));
    }
    let order_method = cx
        .tcx
        .trait_of_assoc(id)
        .is_some_and(|trait_id| cx.tcx.is_diagnostic_item(rustc_span::sym::Ord, trait_id));
    if order_method
        && let [_, minimum, maximum] = args
        && name.as_str() == "clamp"
    {
        // A successful clamp returns between its limits. Its input and any
        // failure on reversed limits are still evaluated by the original call.
        let bound = budget / 2;
        return Some((bounds(cx, minimum, bound)?.0, bounds(cx, maximum, bound)?.1));
    }
    let [left, right] = args else { return None };
    let minimum =
        cx.tcx.is_diagnostic_item(cmp_min, id) || (order_method && name.as_str() == "min");
    let maximum =
        cx.tcx.is_diagnostic_item(cmp_max, id) || (order_method && name.as_str() == "max");
    let arithmetic = named_arithmetic(cx, id);
    if !(minimum || maximum || arithmetic.is_some()) {
        return None;
    }
    let bound = budget / 2;
    let (left, right) = bounds(cx, left, bound).zip(bounds(cx, right, bound))?;
    if minimum {
        return Some((left.0.min(right.0), left.1.min(right.1)));
    }
    if maximum {
        return Some((left.0.max(right.0), left.1.max(right.1)));
    }
    result_bounds(arithmetic?, left, right).filter(|range| range.1 <= kind.maximum)
}

fn wrapped_input<'tcx>(cx: &LateContext<'tcx>, binding: rustc_hir::HirId) -> Option<&'tcx Expr<'tcx>> {
    let Node::Pat(field) = cx.tcx.hir_node(binding) else {
        return None;
    };
    if !matches!(field.kind, PatKind::Binding(BindingMode::NONE, _, _, None)) {
        return None;
    }
    let Node::Pat(pattern) = cx.tcx.parent_hir_node(binding) else {
        return None;
    };
    let PatKind::TupleStruct(ref variant, [only], _) = pattern.kind else {
        return None;
    };
    let ctor = cx.qpath_res(variant, pattern.hir_id).opt_def_id()?;
    if only.hir_id != binding || !is_lang_item_or_ctor(cx, ctor, LangItem::ResultOk) {
        return None;
    }
    let Node::LetStmt(local) = cx.tcx.parent_hir_node(pattern.hir_id) else {
        return None;
    };
    let Node::Stmt(statement) = cx.tcx.parent_hir_node(local.hir_id) else {
        return None;
    };
    if local.els.is_some()
        || !cx.tcx.hir_attrs(local.hir_id).is_empty()
        || !cx.tcx.hir_attrs(statement.hir_id).is_empty()
    {
        return None;
    }
    let init = local.init?;
    let ExprKind::Call(function, [value]) = init.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let ty::FnDef(id, _) = *types.expr_ty(function).kind() else {
        return None;
    };
    if id.is_local()
        || init.span.from_expansion()
        || cx.tcx.crate_name(id.krate).as_str() != boundary_conversion::CRATE
        || cx.tcx.item_name(id).as_str() != "wrapped"
        || cx.tcx.item_name(cx.tcx.parent(id)).as_str() != "integer_bits"
        || !types.expr_adjustments(value).is_empty()
    {
        return None;
    }
    fixed_width(types.expr_ty(value))?;
    Some(value)
}

fn local_bounds(cx: &LateContext<'_>, expr: &Expr<'_>, budget: u32) -> Option<(u128, u128)> {
    let ExprKind::Path(ref path) = expr.kind else {
        return None;
    };
    let id = match cx.qpath_res(path, expr.hir_id) {
        rustc_hir::def::Res::Local(id) => id,
        rustc_hir::def::Res::Def(rustc_hir::def::DefKind::AssocConst { .. }, id)
            if cx.tcx.item_name(id).as_str() == "BITS" => {
                let bits = u128::from(fixed_width(primitive_impl(cx, id)?)?);
                return Some((bits, bits));
            }
        _ => return None,
    };
    if id.owner != cx.typeck_results().hir_owner {
        return None;
    }
    if let Some(input) = wrapped_input(cx, id) {
        let maximum = integer(cx, expr)?.portable_maximum();
        return bounds(cx, input, budget).filter(|range| range.1 <= maximum);
    }
    if let Some(init) = clippy_utils::find_binding_init(cx, id) {
        let types = cx.typeck_results();
        if types.expr_ty(init).is_integral()
            && types.expr_ty(init) == types.expr_ty(expr)
            && types.expr_adjustments(init).is_empty()
        {
            return bounds(cx, init, budget);
        }
        return None;
    }
    // The for-loop desugaring binds the item inside Some(item). Match that
    // exact immutable binding, even in shadowed or nested loops.
    if !matches!(cx.tcx.parent_hir_node(id), rustc_hir::Node::PatField(_)) {
        return None;
    }
    let walked = cx.tcx.hir_parent_iter(id).find_map(|(_, node)| {
        let rustc_hir::Node::Expr(parent) = node else {
            return None;
        };
        ForLoop::hir(parent).filter(|walked| walked.pat.hir_id == id)
    })?;
    if !matches!(
        walked.pat.kind,
        rustc_hir::PatKind::Binding(rustc_hir::BindingMode::NONE, _, _, None)
    ) {
        return None;
    }
    let range = Range::hir(cx, walked.arg)?;
    let (start, end) = (range.start?, range.end?);
    let kind = integer(cx, expr)?;
    let low = if pure_constant(start) {
        constant(cx, start)?
    } else if kind.unsigned {
        0
    } else {
        return None;
    };
    // Even a runtime upper endpoint cannot exceed its integer type. A
    // half-open range therefore excludes MAX for every possible endpoint.
    let end = if pure_constant(end) {
        constant(cx, end)?
    } else {
        kind.maximum
    };
    let high = match range.limits {
        rustc_ast::RangeLimits::HalfOpen => end.checked_sub(1)?,
        rustc_ast::RangeLimits::Closed => end,
    };
    (low <= high).then_some((low, high))
}

// Split the work budget across operands. Following immutable locals can visit
// the same initializer many times in expressions such as x + x.
const BOUNDS_BUDGET: u32 = 128;

fn bounds(cx: &LateContext<'_>, expression: &Expr<'_>, budget: u32) -> Option<(u128, u128)> {
    let next = budget.checked_sub(1)?;
    let kind = integer(cx, expression)?;

    if let Some(value) = constant(cx, expression) {
        return Some((value, value));
    }

    let bounded = match expression.kind {
        ExprKind::Path(_) => local_bounds(cx, expression, next),
        ExprKind::Call(callee, [source])
            if let ty::FnDef(id, _) = *cx.typeck_results().expr_ty(callee).kind()
                && cx.typeck_results().expr_ty(expression).is_integral()
                && let Some(from) = cx.tcx.get_diagnostic_item(rustc_span::sym::From)
                && cx.tcx.trait_of_assoc(id) == Some(from) =>
        {
            let source_ty = cx.typeck_results().expr_ty(source);
            if source_ty.is_bool() {
                Some((0, 1))
            } else if source_ty.is_integral() {
                bounds(cx, source, next).filter(|range| range.1 <= kind.maximum)
            } else {
                None
            }
        }
        ExprKind::Cast(value, _) if cx.typeck_results().expr_ty(value).is_bool() => Some((0, 1)),
        ExprKind::Cast(value, _) => bounds(cx, value, next).filter(|range| range.1 <= kind.maximum),
        ExprKind::DropTemps(value) => bounds(cx, value, next),
        ExprKind::Block(block, None) => block.expr.and_then(|tail| bounds(cx, tail, next)),
        ExprKind::Call(callee, args)
            if let ty::FnDef(id, _) = *cx.typeck_results().expr_ty(callee).kind() =>
        {
            match args {
                [only] => call_bounds(cx, id, &[only], &kind, next),
                [left, right] => call_bounds(cx, id, &[left, right], &kind, next),
                [value, lower, upper] => call_bounds(cx, id, &[value, lower, upper], &kind, next),
                _ => None,
            }
        }
        ExprKind::MethodCall(_, receiver, args, _) => cx
            .typeck_results()
            .type_dependent_def_id(expression.hir_id)
            .and_then(|id| match args {
                [] => call_bounds(cx, id, &[receiver], &kind, next),
                [right] => call_bounds(cx, id, &[receiver, right], &kind, next),
                [lower, upper] => call_bounds(cx, id, &[receiver, lower, upper], &kind, next),
                _ => None,
            }),
        ExprKind::Binary(op, left, right)
            if integer(cx, left).is_some() && integer(cx, right).is_some() =>
        {
            match op.node {
                BinOpKind::BitAnd => constant(cx, left)
                    .or_else(|| constant(cx, right))
                    .map(|mask| (0, mask)),
                BinOpKind::BitOr if kind.unsigned => constant(cx, left)
                    .or_else(|| constant(cx, right))
                    .map(|mask| (mask, kind.maximum)),
                BinOpKind::Add
                | BinOpKind::Sub
                | BinOpKind::Mul
                | BinOpKind::Div
                | BinOpKind::Rem => bounds(cx, left, next / 2)
                    .zip(bounds(cx, right, next / 2))
                    .and_then(|(left, right)| result_bounds(op.node, left, right))
                    .filter(|range| range.1 <= kind.maximum),
                BinOpKind::Shr | BinOpKind::Shl => {
                    let shift = u32::try_from(constant(cx, right)?).ok()?;

                    if shift >= kind.bits {
                        return None;
                    }

                    let (low, high) = bounds(cx, left, next)?;

                    if op.node == BinOpKind::Shr {
                        Some((low.checked_shr(shift)?, high.checked_shr(shift)?))
                    } else if high <= kind.maximum.checked_shr(shift)? {
                        Some((low.checked_shl(shift)?, high.checked_shl(shift)?))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    };

    bounded.or_else(|| kind.unsigned.then_some((0, kind.maximum)))
}

fn value(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<String> {
    // An expanded operand can point into a macro definition even when the
    // surrounding arithmetic was written at the call site.
    if expression.span.from_expansion() {
        return None;
    }
    let mut source = cx
        .sess()
        .source_map()
        .span_to_snippet(expression.span)
        .ok()?;
    // Arithmetic operands become function arguments. The grouping needed by
    // an outer operator is redundant there. Use the parsed operands rather
    // than stripping characters from source that may contain nested groups.
    if source.starts_with('(') && source.ends_with(')') && !source.contains('#')
        && let ExprKind::Binary(_, left, right) = expression.kind
        && !left.span.from_expansion() && !right.span.from_expansion()
        && expression.span.contains(left.span) && expression.span.contains(right.span)
    {
        source = clippy_utils::sugg::Sugg::hir_opt(cx, expression)?.into_string();
    }
    if let ExprKind::Block(block, None) = expression.kind
        && !block.span.from_expansion()
        && expression.span.contains(block.span)
    {
        source = cx.sess().source_map().span_to_snippet(block.span).ok()?;
    }
    let mut kind = cx.typeck_results().expr_ty(expression);
    let mut dereferences = String::new();

    while let ty::Ref(_, inner, _) = kind.kind() {
        dereferences.push('*');
        kind = *inner;
    }

    Some(if dereferences.is_empty() {
        source
    } else {
        format!("{dereferences}({source})")
    })
}

fn shift_argument(cx: &LateContext<'_>, right: &Expr<'_>) -> Option<String> {
    let kind = integer(cx, right)?;
    let source = value(cx, right)?;

    if kind.name == "u32" {
        return Some(source);
    }

    if pure_constant(right)
        && let Some(amount) = constant(cx, right)
    {
        return Some(format!("{amount}_u32"));
    }

    match kind.name.as_str() {
        "u8" | "u16" => Some(format!("::core::primitive::u32::from({source})")),
        "i32" => Some(format!("::core::primitive::i32::unsigned_abs({source})")),
        "i8" | "i16" => Some(format!(
            "::core::primitive::u32::from(::core::primitive::{}::unsigned_abs({source}))",
            kind.name
        )),
        _ if bounds(cx, right, BOUNDS_BUDGET).is_some_and(|(_, high)| high <= 1) => {
            let count = if matches!(right.kind, ExprKind::Path(_) | ExprKind::Call(..) | ExprKind::MethodCall(..)) {
                source
            } else {
                format!("({source})")
            };
            Some(format!("<::core::primitive::u32 as ::core::convert::From<::core::primitive::bool>>::from({count} != 0)"))
        }
        _ => None,
    }
}

// Bits needed to represent the source as a signed integer (including its sign).
// Restrict the proof to fixed widths so it also holds on other targets.
fn signed_width(cx: &LateContext<'_>, expr: &Expr<'_>, budget: u32) -> Option<u64> {
    let next = budget.checked_sub(1)?;
    let types = cx.typeck_results();
    if !matches!(types.expr_ty(expr).kind(), ty::Int(_)) {
        return None;
    }
    if pure_constant(expr) {
        let value = constant(cx, expr)?;
        return Some(u64::from(129 - value.leading_zeros()));
    }
    let operation_width = |op, left, right| {
        let required = arithmetic_width(
            op,
            signed_width(cx, left, next / 2)?,
            signed_width(cx, right, next / 2)?,
        )?;
        (required <= fixed_width(types.expr_ty(expr))?).then_some(required)
    };
    let negated_width = |value| {
        let required = signed_width(cx, value, next)?.checked_add(1)?;
        (required <= fixed_width(types.expr_ty(expr))?).then_some(required)
    };
    let source = match expr.kind {
        ExprKind::Path(ref path) => {
            let Res::Local(binding) = cx.qpath_res(path, expr.hir_id) else {
                return None;
            };
            if binding.owner != types.hir_owner {
                return None;
            }
            wrapped_input(cx, binding)?
        }
        ExprKind::Block(block, None) => return signed_width(cx, block.expr?, next),
        ExprKind::DropTemps(value) => return signed_width(cx, value, next),
        ExprKind::Binary(op, left, right) if !types.is_method_call(expr) => {
            return operation_width(op.node, left, right);
        }
        ExprKind::Unary(UnOp::Neg, value) if !types.is_method_call(expr) => return negated_width(value),
        ExprKind::Cast(source, _) => source,
        ExprKind::Call(function, [left, right]) => {
            let ty::FnDef(id, _) = *types.expr_ty(function).kind() else { return None };
            return operation_width(named_arithmetic(cx, id)?, left, right);
        }
        ExprKind::Call(function, [source]) => {
            let ty::FnDef(id, _) = *types.expr_ty(function).kind() else {
                return None;
            };
            if primitive_impl(cx, id).is_some() && cx.tcx.item_name(id).as_str() == "wrapping_neg" {
                return negated_width(source);
            }
            let from = cx.tcx.get_diagnostic_item(rustc_span::sym::From)?;
            if cx.tcx.trait_of_assoc(id) != Some(from) {
                return None;
            }
            source
        }
        _ => return None,
    };
    let width = match types.expr_ty(source).kind() {
        ty::Int(t) => t.bit_width(),
        ty::Uint(t) => t.bit_width().map(|width| width + 1),
        ty::Bool => Some(2),
        _ => None,
    }?;
    let target = fixed_width(types.expr_ty(expr))?;
    (width <= target).then_some(width)
}

fn arithmetic_width(op: BinOpKind, a: u64, b: u64) -> Option<u64> {
    // Signed addition needs a carry bit. A full product fits in the sum of the widths.
    match op {
        BinOpKind::Add | BinOpKind::Sub => Some(a.max(b) + 1),
        BinOpKind::Mul => Some(a + b),
        _ => None,
    }
}

fn widened_operation_fits(
    cx: &LateContext<'_>,
    op: BinOpKind,
    left: &Expr<'_>,
    right: &Expr<'_>,
) -> bool {
    let types = cx.typeck_results();
    let left_ty = types.expr_ty(left);
    let ty::Int(target) = left_ty.kind() else {
        return false;
    };
    if types.expr_ty(right) != left_ty {
        return false;
    }
    let (Some(bits), Some(a), Some(b)) = (
        target.bit_width(),
        signed_width(cx, left, BOUNDS_BUDGET),
        signed_width(cx, right, BOUNDS_BUDGET),
    ) else {
        return false;
    };
    arithmetic_width(op, a, b).is_some_and(|needed| needed <= bits)
}

fn compound_literal(
    cx: &LateContext<'_>,
    op: BinOpKind,
    place: &Expr<'_>,
    rhs: &Expr<'_>,
) -> Option<String> {
    // Built-in compound assignment evaluates the RHS first. A literal and a
    // local place let us read the old value without moving any side effects.
    let ExprKind::Path(ref path) = place.kind else {
        return None;
    };
    if !matches!(
        cx.qpath_res(path, place.hir_id),
        rustc_hir::def::Res::Local(_)
    ) || !cx.typeck_results().expr_ty(place).is_integral()
        || place.span.from_expansion()
        || rhs.span.from_expansion()
        || !pure_constant(rhs)
    {
        return None;
    }
    let amount = constant(cx, rhs)?;
    let kind = integer(cx, place)?;
    let portable_bits = kind.portable_bits();
    let (method, argument) = match op {
        BinOpKind::Add if amount == 0 => ("wrapping_add", value(cx, rhs)?),
        BinOpKind::Sub if amount == 0 => ("wrapping_sub", value(cx, rhs)?),
        BinOpKind::Mul if amount <= 1 => ("wrapping_mul", value(cx, rhs)?),
        BinOpKind::Div if amount > 0 => ("wrapping_div", value(cx, rhs)?),
        BinOpKind::Rem if amount > 0 => ("wrapping_rem", value(cx, rhs)?),
        BinOpKind::Shl if amount < u128::from(portable_bits) => {
            ("wrapping_shl", format!("{amount}_u32"))
        }
        BinOpKind::Shr if amount < u128::from(portable_bits) => {
            ("wrapping_shr", format!("{amount}_u32"))
        }
        _ => return None,
    };
    let variable = value(cx, place)?;
    Some(format!(
        "{variable} = ::core::primitive::{}::{method}({variable}, {argument})",
        kind.name
    ))
}

pub(super) fn replacement(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<String> {
    let written = cx
        .sess()
        .source_map()
        .span_to_snippet(expression.span)
        .ok()?;
    if written.contains("//") || written.contains("/*") || written.contains("#[") {
        return None;
    }

    if let ExprKind::AssignOp(op, place, rhs) = expression.kind {
        // Replacing a parenthesized assignment inside another expression could
        // change which part of the surrounding expression belongs to the RHS.
        if !matches!(
            cx.tcx.parent_hir_node(expression.hir_id),
            rustc_hir::Node::Stmt(_)
        ) {
            return None;
        }
        return compound_literal(cx, op.node.into(), place, rhs);
    }
    let kind = integer(cx, expression)?;
    let (method, left, right) = match expression.kind {
        ExprKind::Binary(op, left, right)
            if integer(cx, left).is_some() && integer(cx, right).is_some() =>
        {
            let method = match op.node {
                BinOpKind::Div | BinOpKind::Rem if bounds(cx, right, BOUNDS_BUDGET)?.0 > 0 => {
                    if op.node == BinOpKind::Div {
                        "wrapping_div"
                    } else {
                        "wrapping_rem"
                    }
                }
                BinOpKind::Shl | BinOpKind::Shr
                    if bounds(cx, right, BOUNDS_BUDGET)?.1 < u128::from(kind.portable_bits()) =>
                {
                    if op.node == BinOpKind::Shl {
                        "wrapping_shl"
                    } else {
                        "wrapping_shr"
                    }
                }
                BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul => {
                    let zero_left = constant(cx, left) == Some(0);
                    let zero_right = constant(cx, right) == Some(0);
                    let identity = match op.node {
                        BinOpKind::Add => zero_left || zero_right,
                        BinOpKind::Sub => zero_right,
                        BinOpKind::Mul => {
                            zero_left
                                || zero_right
                                || constant(cx, left) == Some(1)
                                || constant(cx, right) == Some(1)
                        }
                        _ => false,
                    };
                    let bounded = bounds(cx, left, BOUNDS_BUDGET)
                        .zip(bounds(cx, right, BOUNDS_BUDGET))
                        .and_then(|(left, right)| result_bounds(op.node, left, right))
                        .is_some_and(|range| range.1 <= kind.portable_maximum());

                    if !identity && !bounded && !widened_operation_fits(cx, op.node, left, right) {
                        return None;
                    }

                    match op.node {
                        BinOpKind::Add => "wrapping_add",
                        BinOpKind::Sub => "wrapping_sub",
                        BinOpKind::Mul => "wrapping_mul",
                        _ => return None,
                    }
                }
                _ => return None,
            };
            let right = if matches!(op.node, BinOpKind::Shl | BinOpKind::Shr) {
                shift_argument(cx, right)?
            } else {
                value(cx, right)?
            };

            (method, value(cx, left)?, Some(right))
        }
        ExprKind::Unary(UnOp::Neg, value_expr)
            if !kind.unsigned
                && (bounds(cx, value_expr, BOUNDS_BUDGET).is_some()
                    || signed_width(cx, value_expr, BOUNDS_BUDGET)
                        .is_some_and(|width| width < u64::from(kind.portable_bits()))) =>
        {
            ("wrapping_neg", value(cx, value_expr)?, None)
        }
        _ => return None,
    };
    let arguments = match right {
        Some(right) => format!("{left}, {right}"),
        None => left,
    };

    Some(format!(
        "::core::primitive::{}::{method}({arguments})",
        kind.name
    ))
}

fn wrapping_method(op: BinOpKind) -> Option<&'static str> {
    match op {
        BinOpKind::Add => Some("wrapping_add"),
        BinOpKind::Sub => Some("wrapping_sub"),
        BinOpKind::Mul => Some("wrapping_mul"),
        BinOpKind::Shl => Some("wrapping_shl"),
        BinOpKind::Shr => Some("wrapping_shr"),
        _ => None,
    }
}

fn wrapping_argument(cx: &LateContext<'_>, op: BinOpKind, right: &Expr<'_>) -> Option<String> {
    if !matches!(op, BinOpKind::Shl | BinOpKind::Shr) {
        return value(cx, right);
    }

    let kind = integer(cx, right)?;
    let source = value(cx, right)?;

    match kind.name.as_str() {
        "u32" => Some(source),
        "u8" | "u16" => Some(format!("::core::primitive::u32::from({source})")),
        "i32" => Some(format!("::core::primitive::i32::cast_unsigned({source})")),
        "i8" | "i16" => Some(format!(
            "::core::primitive::u32::from(::core::primitive::{}::cast_unsigned({source}))",
            kind.name
        )),
        _ => None,
    }
}

fn independently_typed(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    match expression.kind {
        ExprKind::Cast(_, target) => matches!(target.kind, rustc_hir::TyKind::Path(_)),
        ExprKind::Path(ref path) => {
            let rustc_hir::def::Res::Local(binding) = cx.qpath_res(path, expression.hir_id) else {
                return false;
            };

            match cx.tcx.parent_hir_node(binding) {
                rustc_hir::Node::Param(_) => matches!(
                    cx.tcx.def_kind(cx.tcx.hir_enclosing_body_owner(binding)),
                    rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::AssocFn
                ),
                rustc_hir::Node::LetStmt(local) => local.ty.is_some_and(|ty| matches!(ty.kind, rustc_hir::TyKind::Path(_))),
                _ => false,
            }
        }
        _ => false,
    }
}

fn wrapping_call(kind: &Integer, op: BinOpKind, left: &str, right: &str, inferred_width: bool) -> Option<String> {
    let method = wrapping_method(op)?;

    if inferred_width && matches!(kind.name.as_str(), "usize" | "isize") {
        let operator = match op {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            _ => return Some(format!("::core::primitive::{}::{method}({left}, {right})", kind.name)),
        };

        return Some(format!("(::core::num::Wrapping({left}) {operator} ::core::num::Wrapping({right})).0"));
    }

    Some(format!("::core::primitive::{}::{method}({left}, {right})", kind.name))
}

fn borrowable_place(cx: &LateContext<'_>, mut place: &Expr<'_>) -> bool {
    let types = cx.typeck_results();
    loop {
        if place.span.from_expansion() {
            return false;
        }

        match place.kind {
            ExprKind::Path(ref path) => {
                return matches!(cx.qpath_res(path, place.hir_id), rustc_hir::def::Res::Local(_));
            }
            ExprKind::Field(base, _) => {
                if matches!(types.expr_ty_adjusted(base).kind(), ty::Adt(adt, _) if adt.repr().packed()) {
                    return false;
                }
                place = base;
            }
            ExprKind::Index(base, _, _) => place = base,
            ExprKind::Unary(UnOp::Deref, reference) => {
                return !types.expr_ty_adjusted(reference).is_raw_ptr();
            }
            _ => return false,
        }
    }
}

fn boundary_update(
    cx: &LateContext<'_>,
    kind: &Integer,
    operation: BinOpKind,
    place: &str,
    amount: &str,
) -> Option<String> {
    if !cx.sess().edition().at_least_rust_2024()
        || !matches!(operation, BinOpKind::Add | BinOpKind::Sub | BinOpKind::Mul)
    {
        return None;
    }
    let suffix = match kind.name.as_str() {
        "usize" => "index",
        "isize" => "offset",
        _ => return None,
    };
    let helper = format!("{}_{suffix}", wrapping_method(operation)?);
    let boundary = boundary_conversion::CRATE;
    boundary_conversion::available(cx, &helper, 0).then(|| {
        format!("let ::core::result::Result::Ok(()) = ::{boundary}::{helper}({amount}, &mut {place})")
    })
}

fn wrapping_assignment(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    operation: BinOpKind,
    place: &Expr<'_>,
    right: &Expr<'_>,
) -> Option<String> {
    // A trait's assignment operator evaluates its operands in a different order.
    let types = cx.typeck_results();
    if types.is_method_call(expression)
        || !types.expr_ty(place).is_integral()
        || !types.expr_ty(right).is_integral()
        || !matches!(cx.tcx.parent_hir_node(expression.hir_id), rustc_hir::Node::Stmt(_))
        || !borrowable_place(cx, place)
    {
        return None;
    }

    let kind = integer(cx, place)?;
    let target = value(cx, place)?;
    let argument = wrapping_argument(cx, operation, right)?;
    // A function call also holds its operand temporaries through the update.
    if let Some(update) = boundary_update(cx, &kind, operation, &target, &argument) {
        return Some(update);
    }

    let inferred_width = independently_typed(cx, place);
    if matches!(place.kind, ExprKind::Path(_)) && pure_constant(right) {
        let call = wrapping_call(&kind, operation, &target, &argument, inferred_width)?;
        return Some(format!("{target} = {call}"));
    }

    // A match evaluates the amount before borrowing the place and retains the
    // operands' temporaries until the end of the statement. Borrowing the place
    // once also preserves side effects and bounds checks in an indexed target.
    let amount = "explicit_arithmetic_amount";
    let destination = "explicit_arithmetic_place";
    let old = format!("*{destination}");
    let call = wrapping_call(&kind, operation, &old, amount, inferred_width)?;
    Some(format!(
        "match ({argument}, &mut {target}) {{ ({amount}, {destination}) => {old} = {call} }}",
    ))
}

pub(super) fn wrapping_replacement(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<String> {
    let written = cx.sess().source_map().span_to_snippet(expression.span).ok()?;
    if written.contains("//") || written.contains("/*") {
        return None;
    }

    match expression.kind {
        ExprKind::Binary(op, left, right) => {
            let kind = integer(cx, left)?;
            integer(cx, right)?;
            let inferred_width = independently_typed(cx, left) || independently_typed(cx, right);
            let left = value(cx, left)?;
            let right = wrapping_argument(cx, op.node, right)?;

            wrapping_call(&kind, op.node, &left, &right, inferred_width)
        }
        ExprKind::Unary(UnOp::Neg, operand) => {
            let kind = integer(cx, operand)?;
            let inferred_width = independently_typed(cx, operand);
            let operand = value(cx, operand)?;

            if kind.name == "isize" && inferred_width {
                Some(format!("(-::core::num::Wrapping({operand})).0"))
            } else {
                Some(format!("::core::primitive::{}::wrapping_neg({operand})", kind.name))
            }
        }
        ExprKind::AssignOp(op, place, right) => {
            wrapping_assignment(cx, expression, op.node.into(), place, right)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_refuse_underflow_and_large_products() {
        assert_eq!(result_bounds(BinOpKind::Sub, (0, 7), (1, 2)), None);
        assert_eq!(result_bounds(BinOpKind::Mul, (0, u128::MAX), (0, 2)), None);
        assert_eq!(result_bounds(BinOpKind::Add, (0, 7), (1, 3)), Some((1, 10)));
        assert_eq!(result_bounds(BinOpKind::Sub, (3, 7), (1, 2)), Some((1, 6)));
    }
}
