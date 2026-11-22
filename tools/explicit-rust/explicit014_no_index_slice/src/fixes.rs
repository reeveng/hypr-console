//! Destructure fixed arrays when their length proves the access exists.

use clippy_utils::higher::Range;
use rustc_ast::{LitKind, RangeLimits};
use rustc_hir::{
    BinOpKind, BindingMode, BorrowKind, ConstArg, ConstArgKind, Expr, ExprKind, Node, PatKind,
    QPath, Ty, TyKind, def::Res, def_id::DefId,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{
    self,
    adjustment::{Adjust, AutoBorrow, AutoBorrowMutability},
};
use rustc_span::{Span, hygiene::ExpnKind};

const MAX_PREFIX: u16 = 16;

pub(super) fn literal_length(cx: &LateContext<'_>, size: &ConstArg<'_>) -> Option<u64> {
    if size.span.from_expansion() {
        return None;
    }
    match size.kind {
        ConstArgKind::Literal { lit: LitKind::Int(length, _), negated: false } => {
            u64::try_from(length.get()).ok()
        }
        ConstArgKind::Anon(length) => {
            position(cx.tcx.hir_body(length.body).value, 8).map(u64::from)
        }
        _ => None,
    }
}

fn type_length<'tcx>(mut ty: &'tcx Ty<'tcx>, depth: u8) -> Option<&'tcx ConstArg<'tcx>> {
    while let TyKind::Ref(_, inner) = ty.kind {
        ty = inner.ty;
    }
    let TyKind::Array(item, length) = ty.kind else {
        return None;
    };
    match depth.checked_sub(1) {
        Some(next) => type_length(item, next),
        None => Some(length),
    }
}

enum Length<'tcx> {
    Written(&'tcx ConstArg<'tcx>),
    Listed(usize),
}

// A suffix is relative to the array length. Prove that length from source
// syntax so a different cfg or target cannot shift the chosen element.
fn written_length<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    depth: u8,
    budget: u8,
) -> Option<Length<'tcx>> {
    let next = budget.checked_sub(1)?;
    if expr.span.from_expansion() {
        return None;
    }
    match expr.kind {
        ExprKind::Index(outer, _, _) => written_length(cx, outer, depth.checked_add(1)?, next),
        ExprKind::AddrOf(BorrowKind::Ref, _, inner) => written_length(cx, inner, depth, next),
        ExprKind::Array(items) if depth == 0 => Some(Length::Listed(items.len())),
        ExprKind::Repeat(item, length) => match depth.checked_sub(1) {
            Some(inner) => written_length(cx, item, inner, next),
            None => Some(Length::Written(length)),
        },
        ExprKind::Path(QPath::Resolved(_, path)) => {
            let Res::Local(binding) = path.res else {
                return None;
            };
            let owner = cx.tcx.hir_enclosing_body_owner(binding);
            if owner != cx.tcx.hir_enclosing_body_owner(expr.hir_id) {
                return None;
            }
            match cx.tcx.parent_hir_node(binding) {
                Node::LetStmt(local) if local.pat.hir_id == binding => match local.ty {
                    Some(ty) => type_length(ty, depth).map(Length::Written),
                    None => written_length(cx, local.init?, depth, next),
                },
                Node::Param(param) if param.pat.hir_id == binding => {
                    let body = cx.tcx.hir_body_owned_by(owner);
                    let offset = body.params.iter().position(|input| input.hir_id == param.hir_id)?;
                    let declaration = cx.tcx.hir_fn_decl_by_hir_id(cx.tcx.local_def_id_to_hir_id(owner))?;
                    type_length(declaration.inputs.get(offset)?, depth).map(Length::Written)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn same_length_on_other_targets<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    depth: u8,
    length: u64,
) -> bool {
    // Configuration attributes can remove array elements or choose a different
    // binding before HIR is built. Allow only functions without attributes in
    // their parameter list or body.
    if !function_source(cx, base).is_some_and(|text| {
        !tokenize(&text, FrontmatterAllowed::No).any(|token| token.kind == TokenKind::Pound)
    }) {
        return false;
    }
    length_matches(cx, base, depth, length)
}

fn length_matches<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    depth: u8,
    length: u64,
) -> bool {
    let found = match written_length(cx, base, depth, 8) {
        Some(Length::Listed(length)) => u64::try_from(length).ok(),
        Some(Length::Written(size)) => literal_length(cx, size),
        None => None,
    };
    found == Some(length)
}

// A fixed-size local buffer and every use in the loop can be scoped to the
// same unconditional block, even when other branches of the function use cfg.
pub(super) fn loop_length<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    depth: u8,
    length: u64,
    body: Span,
) -> bool {
    if same_length_on_other_targets(cx, base, depth, length) {
        return true;
    }
    let ExprKind::Path(QPath::Resolved(_, path)) = base.kind else {
        return false;
    };
    let Res::Local(binding) = path.res else {
        return false;
    };
    let Node::LetStmt(local) = cx.tcx.parent_hir_node(binding) else {
        return false;
    };
    let local_literal = local.init.is_some_and(|init| {
        matches!(init.kind, ExprKind::Array(_) | ExprKind::Repeat(..))
    });
    if local.pat.hir_id != binding
        || local.span.from_expansion()
        || local.ty.is_none() && !local_literal
    {
        return false;
    }
    let Some(block) = cx.tcx.hir_parent_iter(binding).find_map(|(_, node)| match node {
        Node::Block(block) => Some(block.span),
        _ => None,
    }) else {
        return false;
    };
    if block.from_expansion() || !block.contains(body) || !block.contains(local.span) {
        return false;
    }
    let Ok(text) = cx.sess().source_map().span_to_snippet(block) else {
        return false;
    };
    // Macros taking an identifier can introduce a conditional shadow binding.
    !tokenize(&text, FrontmatterAllowed::No)
        .any(|token| matches!(token.kind, TokenKind::Pound | TokenKind::Bang))
        && length_matches(cx, base, depth, length)
}

pub(super) fn length_named_by<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    definition: DefId,
    bound: Span,
) -> Option<()> {
    // Conditional bindings and macros can make the two authored names resolve
    // differently in another configuration. Check both their spelling and
    // resolution in a body without either form of conditional scope.
    let source = function_source(cx, base)?;
    if tokenize(&source, FrontmatterAllowed::No)
        .any(|token| matches!(token.kind, TokenKind::Pound | TokenKind::Bang))
    {
        return None;
    }
    let Length::Written(size) = written_length(cx, base, 0, 8)? else {
        return None;
    };
    let ConstArgKind::Anon(value) = size.kind else {
        return None;
    };
    let declared = cx.tcx.hir_body(value.body).value;
    let ExprKind::Path(QPath::Resolved(None, path)) = declared.kind else {
        return None;
    };
    (path.res.opt_def_id() == Some(definition) && clean(cx, declared.span)? == clean(cx, bound)?)
        .then_some(())
}

pub(super) fn function_source(cx: &LateContext<'_>, access: &Expr<'_>) -> Option<String> {
    let owner = cx.tcx.hir_enclosing_body_owner(access.hir_id);
    let signature = cx.tcx.hir_span(cx.tcx.local_def_id_to_hir_id(owner));
    let map = cx.sess().source_map();
    if !signature.from_expansion() {
        let body = cx.tcx.hir_body_owned_by(owner).value.span;
        let declaration = map.span_to_snippet(signature).ok()?;
        let body = map.span_to_snippet(body).ok()?;
        return Some(format!("{declaration}\n{body}"));
    }

    // Archmage's CPU-target wrappers retain the written function. Their
    // generated item and body spans can both describe only the attribute.
    let mut reviewed = false;
    for expansion in signature.macro_backtrace() {
        if matches!(expansion.kind, ExpnKind::Macro(..)) {
            let id = expansion.macro_def_id?;
            let name = cx.tcx.item_name(id);
            if cx.tcx.crate_name(id.krate).as_str() != "archmage_macros"
                || !matches!(name.as_str(), "arcane" | "rite")
            {
                return None;
            }
            reviewed = true;
        }
    }
    if !reviewed {
        return None;
    }
    let authored = super::authored_function::authored_function(cx, signature, &[access.span])?;
    let text = map.span_to_snippet(authored).ok()?;
    // The reviewed target-feature wrapper retains subsequent item attributes.
    // Inlining cannot change the parameter types, array elements or loop body.
    let trimmed = text.trim_start();
    if let Some(item) = ["#[inline]", "#[inline(always)]", "#[inline(never)]"]
        .into_iter()
        .find_map(|attribute| trimmed.strip_prefix(attribute))
    {
        return Some(item.to_owned());
    }
    Some(text)
}

pub(super) fn element_pattern<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    at: u16,
    length: u64,
    element: &str,
) -> Option<String> {
    let suffix = length.checked_sub(u64::from(at).checked_add(1)?)?;
    if at <= MAX_PREFIX {
        let prefix = "_, ".repeat(usize::from(at));
        return Some(format!("[{prefix}{element}, ..]"));
    }
    if suffix <= u64::from(MAX_PREFIX) && same_length_on_other_targets(cx, base, 0, length) {
        let ignored = ", _".repeat(usize::try_from(suffix).ok()?);
        return Some(format!("[.., {element}{ignored}]"));
    }
    None
}

fn local_index(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(_, path)) => matches!(path.res, Res::Local(_)),
        ExprKind::Unary(rustc_hir::UnOp::Deref, reference) => {
            cx.typeck_results().expr_ty(reference).is_ref() && local_index(cx, reference)
        }
        _ => false,
    }
}

fn local_place(cx: &LateContext<'_>, expr: &Expr<'_>, indexed_field: bool) -> bool {
    match expr.kind {
        ExprKind::Path(QPath::Resolved(_, path)) => matches!(path.res, Res::Local(_)),
        ExprKind::Field(base, _) => local_place(cx, base, indexed_field),
        ExprKind::Index(base, at, _) if indexed_field => {
            cx.typeck_results().expr_ty(at).is_usize()
                && local_index(cx, at)
                && local_place(cx, base, true)
        }
        _ => false,
    }
}

// Wrap a pattern in each preceding array projection. Every index must be
// valid for the array it selects from, even when its result is another array.
fn array_pattern<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    pattern: String,
) -> Option<(&'tcx Expr<'tcx>, String)> {
    if local_place(cx, base, false) {
        return Some((base, pattern));
    }
    let ExprKind::Index(outer, index, _) = base.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let ty::Array(element, length) = types.expr_ty(outer).peel_refs().kind() else {
        return None;
    };
    if !element.is_array() || base.span.from_expansion() {
        return None;
    }
    let at = position(index, 8)?;
    let outer_pattern = element_pattern(cx, outer, at, length.try_to_target_usize(cx.tcx)?, &pattern)?;
    array_pattern(cx, outer, outer_pattern)
}

// Only fold literal arithmetic that also fits the narrowest supported usize.
// A named constant or a macro can change with a feature or target.
pub(super) fn position(expression: &Expr<'_>, budget: u8) -> Option<u16> {
    let next = budget.checked_sub(1)?;
    if expression.span.from_expansion() {
        return None;
    }
    match expression.kind {
        ExprKind::Lit(literal) => match literal.node {
            LitKind::Int(value, _) => u16::try_from(value.get()).ok(),
            _ => None,
        },
        ExprKind::Binary(operator, left, right) => {
            let left = position(left, next)?;
            let right = position(right, next)?;
            match operator.node {
                BinOpKind::Add => left.checked_add(right),
                BinOpKind::Sub => left.checked_sub(right),
                BinOpKind::Mul => left.checked_mul(right),
                BinOpKind::Div => left.checked_div(right),
                BinOpKind::Rem => left.checked_rem(right),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn clean(cx: &LateContext<'_>, span: Span) -> Option<String> {
    if span.from_expansion() {
        return None;
    }
    let text = cx.sess().source_map().span_to_snippet(span).ok()?;
    (!text.contains("//") && !text.contains("/*")).then_some(text)
}

fn value_expression(cx: &LateContext<'_>, expression: &Expr<'_>, code: String) -> String {
    // A leading match/block before an operator can parse as a statement.
    if matches!(
        cx.tcx.parent_hir_node(expression.hir_id),
        Node::Expr(parent) if match parent.kind {
            ExprKind::Binary(..) | ExprKind::Cast(..) => true,
            ExprKind::MethodCall(_, receiver, _, _) => receiver.hir_id == expression.hir_id,
            _ => false,
        }
    ) {
        format!("({code})")
    } else {
        code
    }
}

pub(super) fn temporary_array_slice<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<Vec<(Span, String)>> {
    let ExprKind::Index(base, index, _) = expr.kind else { return None };
    if !matches!(base.kind, ExprKind::Array(_) | ExprKind::Repeat(..))
        || !cx.typeck_results().expr_ty(base).is_array()
    {
        return None;
    }
    let range = Range::hir(cx, index)?;
    if range.start.is_some() || range.end.is_some() {
        return None;
    }
    let Node::Expr(borrow) = cx.tcx.parent_hir_node(expr.hir_id) else { return None };
    let ExprKind::AddrOf(BorrowKind::Ref, mutability, _) = borrow.kind else { return None };
    let Node::LetStmt(binding) = cx.tcx.parent_hir_node(borrow.hir_id) else { return None };
    if !matches!(binding.pat.kind, PatKind::Binding(BindingMode::NONE | BindingMode::MUT, _, _, None))
        || binding.pat.span.from_expansion()
    {
        return None;
    }
    // A coercion at the let binding keeps the borrow expression extending
    // the temporary's lifetime. Passing it to get or AsRef would end it early.
    let reference = if mutability.is_mut() { "&mut " } else { "&" };
    clean(cx, borrow.span)?;
    let mut edits = vec![(borrow.span, format!("{reference}{}", clean(cx, base.span)?))];
    match binding.ty {
        None => edits.push((binding.pat.span.shrink_to_hi(), format!(": {reference}[_]"))),
        Some(t) if matches!(t.kind, TyKind::Ref(_, r) if matches!(r.ty.kind, TyKind::Slice(_))) => {}
        Some(_) => return None,
    }
    Some(edits)
}

fn borrow<'tcx>(
    cx: &LateContext<'tcx>,
    base: &'tcx Expr<'tcx>,
    index: &Expr<'_>,
    length: u64,
    mutability: rustc_hir::Mutability,
) -> Option<String> {
    let (pattern, slice) = if let Some(range) = Range::hir(cx, index) {
        if range.span.from_expansion()
            || (range.end.is_some() && !same_length_on_other_targets(cx, base, 0, length))
        {
            return None;
        }
        let start = match range.start {
            Some(start) => position(start, 8)?,
            None => 0,
        };
        let end = match (range.end, range.limits) {
            (None, RangeLimits::HalfOpen) => length,
            (Some(end), RangeLimits::HalfOpen) => u64::from(position(end, 8)?),
            (Some(end), RangeLimits::Closed) => u64::from(position(end, 8)?.checked_add(1)?),
            (None, RangeLimits::Closed) => return None,
        };
        let suffix = length.checked_sub(end)?;
        if u64::from(start) > end || u64::from(start).checked_add(suffix)? > u64::from(MAX_PREFIX) {
            return None;
        }
        let pattern = if start == 0 && suffix == 0 {
            "_explicit_part".to_owned()
        } else {
            let prefix = "_, ".repeat(usize::from(start));
            let suffix = ", _".repeat(usize::try_from(suffix).ok()?);
            format!("[{prefix}_explicit_part @ ..{suffix}]")
        };
        (pattern, true)
    } else {
        (element_pattern(cx, base, position(index, 8)?, length, "_explicit_part")?, false)
    };
    let (base, pattern) = array_pattern(cx, base, pattern)?;
    let held = clean(cx, base.span)?;
    let mut dereferences = String::new();
    let mut base_ty = cx.typeck_results().expr_ty(base);
    while let ty::Ref(_, inner, _) = base_ty.kind() {
        dereferences.push('*');
        base_ty = *inner;
    }
    let mutability = if mutability.is_mut() { "mut " } else { "" };
    // Match a reborrow of the array so the bindings borrow its elements. The
    // explicit slice annotation preserves the original value's unsized type.
    let answer = if slice {
        format!("let _explicit_slice: &{mutability}[_] = _explicit_part; _explicit_slice")
    } else {
        "_explicit_part".to_owned()
    };
    Some(format!(
        "{{ let {pattern} = &{mutability}{dereferences}{held}; {answer} }}"
    ))
}

pub(super) fn edits<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<Vec<(rustc_span::Span, String)>> {
    let ExprKind::Index(base, index, _) = expr.kind else {
        return None;
    };
    if base.span.from_expansion() {
        return None;
    }
    let types = cx.typeck_results();
    let ty::Array(element, length) = types.expr_ty(base).peel_refs().kind() else {
        return None;
    };
    let length = length.try_to_target_usize(cx.tcx)?;
    let parent = cx.tcx.parent_hir_node(expr.hir_id);
    let indexed_field = || matches!(base.kind, ExprKind::Field(..)) && local_place(cx, base, true);
    if let Node::Expr(addr) = parent
        && let ExprKind::AddrOf(BorrowKind::Ref, mutability, operand) = addr.kind
        && operand.hir_id == expr.hir_id
    {
        clean(cx, addr.span)?;
        let reference = borrow(cx, base, index, length, mutability)?;
        return Some(vec![(addr.span, value_expression(cx, addr, reference))]);
    }
    if let Node::Expr(assign) = parent
        && let Some(place) = match assign.kind {
            ExprKind::Assign(place, _, _) => Some(place),
            ExprKind::AssignOp(_, place, rhs) if types.expr_ty(place).is_primitive()
                && types.expr_ty(rhs).is_primitive() => Some(place),
            _ => None,
        }
        && place.hir_id == expr.hir_id
        && !types.expr_ty(expr).is_slice()
    {
        // Keep this on the left of the assignment so Rust still evaluates
        // the RHS before borrowing the array element.
        clean(cx, expr.span)?;
        let reference = borrow(cx, base, index, length, rustc_hir::Mutability::Mut)?;
        return Some(vec![(expr.span, format!("*{reference}"))]);
    }

    let n = position(index, 8)?;
    if !cx.type_is_copy_modulo_regions(*element) {
        return None;
    }
    if let Node::Expr(parent) = parent
        && let ExprKind::MethodCall(_, receiver, _, _) = parent.kind
        && receiver.hir_id == expr.hir_id
        && types.expr_ty(expr).is_primitive()
        && matches!(types.expr_adjustments(expr), [adjustment]
            if matches!(adjustment.kind, Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not))))
        && (indexed_field() || local_place(cx, base, false))
    {
        // Keep the implicit shared receiver borrowing the same element.
        let read = super::recovery::fixed_projection(cx, expr)?;
        return Some(vec![(expr.span, read)]);
    }
    // A place can also be assigned to, borrowed, or used as a method receiver.
    // Only replace a scalar in a context that reads it by value.
    let read = match parent {
        Node::Expr(parent) => match parent.kind {
            ExprKind::Binary(_, a, b) => {
                types.expr_ty(a).is_primitive() && types.expr_ty(b).is_primitive()
            }
            ExprKind::Cast(..) | ExprKind::Ret(..) | ExprKind::Array(..) | ExprKind::Tup(..)
            | ExprKind::Unary(rustc_hir::UnOp::Neg | rustc_hir::UnOp::Not, _) => true,
            ExprKind::Call(_, args) => {
                args.iter().any(|arg| arg.hir_id == expr.hir_id)
            }
            ExprKind::MethodCall(_, receiver, args, _) => {
                args.iter().any(|arg| arg.hir_id == expr.hir_id)
                    || receiver.hir_id == expr.hir_id
                        && types.expr_ty(expr).is_primitive()
                        && types.expr_adjustments(expr).is_empty()
            }
            ExprKind::Assign(_, rhs, _) => rhs.hir_id == expr.hir_id,
            ExprKind::AssignOp(_, lhs, rhs) => {
                rhs.hir_id == expr.hir_id && types.expr_ty(lhs).is_primitive()
                    && types.expr_ty(rhs).is_primitive()
            }
            _ => false,
        },
        Node::LetStmt(local) => matches!(
            local.pat.kind,
            PatKind::Binding(BindingMode::NONE, _, _, None)
        ),
        Node::Block(block) => block.expr.is_some_and(|tail| tail.hir_id == expr.hir_id),
        Node::ExprField(field) => field.expr.hir_id == expr.hir_id,
        _ => false,
    };
    if !read {
        return None;
    }
    let map = cx.sess().source_map();
    clean(cx, expr.span)?;
    let mut references = String::new();
    // When there is already a binding, unpack the array there. The other
    // elements stay in place and the scalar keeps its original name and type.
    let (name, binding_span) = if let Node::LetStmt(local) = parent
        && local.ty.is_none()
        && !local.pat.span.from_expansion()
        && let PatKind::Binding(BindingMode::NONE, _, _, None) = local.pat.kind
    {
        (map.span_to_snippet(local.pat.span).ok()?, Some(local.pat.span))
    } else {
        ("_explicit_element".to_owned(), None)
    };
    let pattern = element_pattern(cx, base, n, length, &name)?;
    // A field of an indexed element is still a place. Destructure that field
    // directly for a Copy read, leaving the earlier access in place.
    let (base, pattern) = if indexed_field() {
        (base, pattern)
    } else {
        array_pattern(cx, base, pattern)?
    };
    let held = clean(cx, base.span)?;
    let mut ty = types.expr_ty(base);
    while let ty::Ref(_, inner, mutability) = ty.kind() {
        references.push_str(if mutability.is_mut() { "&mut " } else { "&" });
        ty = *inner;
    }
    if let Some(binding_span) = binding_span {
        return Some(vec![
            (binding_span, format!("{references}{pattern}")),
            (expr.span, held),
        ]);
    }
    let read = format!("match {held} {{ {references}{pattern} => {name} }}");
    Some(vec![(expr.span, value_expression(cx, expr, read))])
}
