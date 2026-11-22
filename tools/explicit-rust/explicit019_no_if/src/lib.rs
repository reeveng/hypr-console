#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

#[path = "../../shared/macro_input.rs"]
mod macro_input;
mod pattern_cases;
mod type_sources;

use clippy_utils::diagnostics::{span_lint_and_help, span_lint_and_then};
use clippy_utils::source::{indent_of, reindent_multiline};
use clippy_utils::{is_in_const_context, sym};
use rustc_hir::attrs::AttributeKind;
use rustc_hir::def::{CtorKind, CtorOf, DefKind, Res};
use rustc_hir::{
    Arm, Attribute, BinOpKind, Expr, ExprKind, HirId, ItemKind, LetExpr, MatchSource, Node,
    PatKind, UnOp,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use std::collections::HashMap;
use type_sources::{
    cfg_can_shadow_locals, conditional_definition, conditional_import, conditional_type,
    fixed_primitive_source, uses_conditional_binding,
};

dylint_linting::impl_late_lint! {
    /// EXPLICIT019: an `if` or a match over a boolean leaves the outcomes
    /// unnamed. Match on the enum that describes the decision. A plain
    /// predicate can become an `Option<()>` through `bool::then_some`.
    /// A `From<bool>` implementation may match its input directly when each
    /// arm maps it to a distinct unit variant of the resulting enum.
    ///
    /// A match guard is the same `if` written on an arm: `Variant if cond =>`
    /// asks a question whose false answer is spelled by falling through to
    /// whatever arm comes next, which is the outcome without a name again.
    /// Put the question in the pattern or match on the enum that answers it.
    /// Guards inside macro input, such as `matches!(x, P if c)`, are exempt.
    ///
    /// `let … else` was exempted here once, on the argument that both of its
    /// outcomes are written. Only one of them is: the other is spelled `else`
    /// and names nothing. EXPLICIT023 is that rule, in its own crate because
    /// the tree is still walking toward it.
    pub EXPLICIT019_NO_IF,
    Deny,
    "use an enum match for a binary decision",
    Explicit019NoIf {
        structural_only: dylint_linting::config_or_default("explicit019_structural_only"),
        replacements: HashMap::new(),
    }
}

pub struct Explicit019NoIf {
    structural_only: bool,
    replacements: HashMap<HirId, Vec<Span>>,
}

const PRESENT: &str = "::core::option::Option::Some(_)";
const ABSENT: &str = "::core::option::Option::None";
const MET: &str = "::core::option::Option::Some(())";
const OK: &str = "::core::result::Result::Ok(_)";
const ERR: &str = "::core::result::Result::Err(_)";
const ENUM_HELP: &str = "match on an enum that names the outcomes";

fn predicate_as_option(predicate: &str) -> String {
    // Pass the expected `bool` type to the original expression as `if` did.
    format!("::core::primitive::bool::then_some({predicate}, ())")
}

fn has_let_condition(condition: &Expr<'_>) -> bool {
    match condition.kind {
        ExprKind::Let(_) => true,
        ExprKind::DropTemps(inner) => has_let_condition(inner),
        ExprKind::Binary(op, left, right) if op.node == BinOpKind::And => {
            has_let_condition(left) || has_let_condition(right)
        }
        _ => false,
    }
}

fn indent_branch(source: &str, indent: usize) -> Option<String> {
    let mut offset = 0;
    for token in tokenize(source, FrontmatterAllowed::No) {
        let next = offset + token.len as usize;
        if token.kind != TokenKind::Whitespace && source[offset..next].contains('\n') {
            return None;
        }
        offset = next;
    }
    Some(reindent_multiline(source, true, Some(indent)))
}

fn named_cases<'tcx>(
    cx: &LateContext<'tcx>,
    condition: &LetExpr<'tcx>,
) -> Option<(String, String)> {
    let map = cx.sess().source_map();
    let pattern = condition.pat.span;
    let input = condition.init.span;
    if pattern.from_expansion()
        || input.from_expansion()
        || map
            .span_to_snippet(condition.span.with_hi(pattern.lo()))
            .ok()?
            .trim()
            != "let"
        || map
            .span_to_snippet(pattern.with_lo(pattern.hi()).with_hi(input.lo()))
            .ok()?
            .trim()
            != "="
        || condition.span.hi() != input.hi()
    {
        return None;
    }
    let pattern = map.span_to_snippet(pattern).ok()?;
    if pattern.contains('\n') || has_attributes(&pattern) {
        return None;
    }
    let cases = pattern_cases::complement(cx, condition.pat)?;
    (!cases.is_empty()).then(|| (cases.join(" | "), pattern))
}

fn has_attributes(source: &str) -> bool {
    tokenize(source, FrontmatterAllowed::No).any(|token| token.kind == TokenKind::Pound)
}

fn enum_test<'tcx>(
    cx: &LateContext<'tcx>,
    condition: &'tcx Expr<'tcx>,
) -> Option<(String, &'static str, &'static str)> {
    let condition = condition.peel_drop_temps();
    let (predicate, negated) = match condition.kind {
        ExprKind::Unary(UnOp::Not, inner) => (inner, true),
        _ => (condition, false),
    };
    let ExprKind::MethodCall(method, receiver, [], _) = predicate.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let receiver_type = types.expr_ty(receiver);
    let predicate_def = types.type_dependent_def_id(predicate.hir_id)?;
    // A direct local needs no deref or projection to evaluate the predicate.
    if !matches!(receiver.kind, ExprKind::Path(ref path)
        if matches!(cx.qpath_res(path, receiver.hir_id), Res::Local(_)))
        || cx.tcx.inherent_impl_of_assoc(predicate_def).is_none()
        || uses_conditional_binding(cx, receiver)
        || [condition.span, predicate.span, receiver.span]
            .iter()
            .any(|span| span.from_expansion())
    {
        return None;
    }
    let diagnostic_name = receiver_type
        .ty_adt_def()
        .and_then(|adt| cx.tcx.get_diagnostic_name(adt.did()));
    let (yes, no) = match (diagnostic_name, method.ident.as_str()) {
        (Some(sym::Option), "is_some") => (PRESENT, ABSENT),
        (Some(sym::Option), "is_none") => (ABSENT, PRESENT),
        (Some(sym::Result), "is_ok") => (OK, ERR),
        (Some(sym::Result), "is_err") => (ERR, OK),
        (_, "is_empty") if receiver_type.peel_refs().is_slice() => ("[]", "[_, ..]"),
        _ => return None,
    };
    let map = cx.sess().source_map();
    let text = map.span_to_snippet(condition.span).ok()?;
    if text.contains("//") || text.contains("/*") || has_attributes(&text) {
        return None;
    }
    let receiver = map.span_to_snippet(receiver.span).ok()?;
    Some(if negated {
        (receiver, no, yes)
    } else {
        (receiver, yes, no)
    })
}

fn stable_place(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    match expression.kind {
        ExprKind::Path(ref path) => {
            matches!(cx.qpath_res(path, expression.hir_id), Res::Local(_))
        }
        ExprKind::Field(base, _) | ExprKind::AddrOf(_, _, base) => stable_place(cx, base),
        _ => false,
    }
}

fn has_drop_free_scrutinee(cx: &LateContext<'_>, scrutinee: &Expr<'_>) -> bool {
    if stable_place(cx, scrutinee) {
        return true;
    }
    if cx
        .typeck_results()
        .expr_ty(scrutinee)
        .needs_drop(cx.tcx, cx.typing_env())
    {
        return false;
    }
    match scrutinee.kind {
        // A zero-argument call on a place introduces no argument temporaries.
        ExprKind::MethodCall(_, receiver, [], _) => stable_place(cx, receiver),
        ExprKind::Tup(fields) => fields
            .iter()
            .all(|field| has_drop_free_scrutinee(cx, field)),
        _ => false,
    }
}

fn unbraced_condition<'tcx>(
    cx: &LateContext<'tcx>,
    condition: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let condition = condition.peel_drop_temps();
    if let ExprKind::Block(block, None) = condition.kind
        && block.stmts.is_empty()
    {
        let source = cx
            .sess()
            .source_map()
            .span_to_snippet(condition.span)
            .ok()?;
        if condition.span.from_expansion()
            || has_attributes(&source)
            || source.contains("//")
            || source.contains("/*")
        {
            return None;
        }
        return unbraced_condition(cx, block.expr?);
    }
    Some(condition)
}

fn literal_test<'tcx>(
    cx: &LateContext<'tcx>,
    condition: &'tcx Expr<'tcx>,
) -> Option<(String, String, String)> {
    let condition = condition.peel_drop_temps();
    let ExprKind::Binary(operator, left, right) = condition.kind else {
        return None;
    };
    let (value, literal) = match (left.kind, right.kind) {
        (_, ExprKind::Lit(_)) => (left, right),
        (ExprKind::Lit(_), _) => (right, left),
        _ => return None,
    };
    // Primitive comparisons on a place have no user-defined equality or
    // temporary whose lifetime would change when matched directly.
    let types = cx.typeck_results();
    let ty = types.expr_ty(value);
    if !matches!(operator.node, BinOpKind::Eq | BinOpKind::Ne)
        || (!ty.is_integral() && !ty.is_char())
        || ty != types.expr_ty(literal)
        || !stable_place(cx, value)
        || uses_conditional_binding(cx, value)
        || [condition.span, value.span, literal.span]
            .iter()
            .any(|span| span.from_expansion())
    {
        return None;
    }
    let map = cx.sess().source_map();
    let text = map.span_to_snippet(condition.span).ok()?;
    if text.contains("//") || text.contains("/*") || has_attributes(&text) {
        return None;
    }
    let value = map.span_to_snippet(value.span).ok()?;
    let literal = map.span_to_snippet(literal.span).ok()?;
    let other = "_".to_owned();
    Some(match operator.node {
        BinOpKind::Eq => (value, literal, other),
        _ => (value, other, literal),
    })
}

fn direct_cases<'tcx>(
    cx: &LateContext<'tcx>,
    condition: &'tcx Expr<'tcx>,
    drop_before_arms: bool,
) -> Option<(String, String, String)> {
    let original = condition.peel_drop_temps();
    let condition = unbraced_condition(cx, condition)?;
    if cfg_can_shadow_locals(cx, condition) {
        return None;
    }
    enum_test(cx, condition)
        .map(|(value, yes, no)| (value, yes.to_owned(), no.to_owned()))
        .or_else(|| literal_test(cx, condition))
        .or_else(|| {
            let braced = original.hir_id != condition.hir_id;
            ordering_test(
                cx,
                condition,
                drop_before_arms || (braced && cx.sess().edition().at_least_rust_2024()),
            )
        })
}

fn ordering_test<'tcx>(
    cx: &LateContext<'tcx>,
    condition: &'tcx Expr<'tcx>,
    drop_before_arms: bool,
) -> Option<(String, String, String)> {
    const LESS: &str = "::core::cmp::Ordering::Less";
    const EQUAL: &str = "::core::cmp::Ordering::Equal";
    const GREATER: &str = "::core::cmp::Ordering::Greater";
    let ExprKind::Binary(operator, left, right) = condition.kind else {
        return None;
    };
    let ty = cx.typeck_results().expr_ty(left);
    if is_in_const_context(cx)
        || (!ty.is_integral() && !ty.is_char())
        || ty != cx.typeck_results().expr_ty(right)
        || [left, right].iter().any(|operand| {
            !fixed_primitive_source(cx, operand) || uses_conditional_binding(cx, operand)
        })
        || [condition.span, left.span, right.span]
            .iter()
            .any(|span| span.from_expansion())
    {
        return None;
    }
    let (yes, no): (&[&str], &[&str]) = match operator.node {
        BinOpKind::Lt => (&[LESS], &[EQUAL, GREATER]),
        BinOpKind::Le => (&[LESS, EQUAL], &[GREATER]),
        BinOpKind::Eq => (&[EQUAL], &[LESS, GREATER]),
        BinOpKind::Ne => (&[LESS, GREATER], &[EQUAL]),
        BinOpKind::Ge => (&[EQUAL, GREATER], &[LESS]),
        BinOpKind::Gt => (&[GREATER], &[LESS, EQUAL]),
        _ => return None,
    };
    let map = cx.sess().source_map();
    let source = map.span_to_snippet(condition.span).ok()?;
    if source.contains("//") || source.contains("/*") || has_attributes(&source) {
        return None;
    }
    let left = map.span_to_snippet(left.span).ok()?;
    let right = map.span_to_snippet(right.span).ok()?;
    // Evaluate each operand by value as the primitive comparison did.
    let ordering = format!(
        "::core::cmp::Ord::cmp(&::core::convert::identity({left}), &::core::convert::identity({right}))"
    );
    let input = if drop_before_arms {
        // A statement gives condition temporaries the correct drop order
        // in every supported edition.
        format!("{{ let _explicit_ordering = {ordering}; _explicit_ordering }}")
    } else {
        ordering
    };
    Some((input, yes.join(" | "), no.join(" | ")))
}

fn named_match<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    indent: usize,
    structural_only: bool,
) -> Option<String> {
    let ExprKind::If(condition, yes, no) = expr.kind else {
        return None;
    };
    if !matches!(yes.kind, ExprKind::Block(..)) {
        return None;
    }
    let map = cx.sess().source_map();
    let source = |span| map.span_to_snippet(span).ok();
    if source(expr.span.with_hi(condition.span.lo()))?.trim() != "if"
        || !source(
            condition
                .span
                .with_lo(condition.span.hi())
                .with_hi(yes.span.lo()),
        )?
        .trim()
        .is_empty()
    {
        return None;
    }
    // A Rust 2024 no-else if-let drops its temporaries before a later
    // argument of an enclosing call. Give the match statement its own scope.
    let isolate = cx.sess().edition().at_least_rust_2024()
        && no.is_none()
        && !matches!(cx.tcx.parent_hir_node(expr.hir_id), Node::Stmt(_))
        && matches!(condition.peel_drop_temps().kind, ExprKind::Let(binding)
            if !has_drop_free_scrutinee(cx, binding.init));
    let indent = indent + if isolate { 4 } else { 0 };
    let last = no.unwrap_or(yes);
    if last.span.hi() != expr.span.hi()
        || [condition.span, yes.span, last.span]
            .iter()
            .any(|span| span.from_expansion())
    {
        return None;
    }
    let closing = " ".repeat(indent);
    let arm = " ".repeat(indent + 4);
    let (scrutinee, yes_case, no_case) = match condition.peel_drop_temps().kind {
        ExprKind::Let(binding) => {
            // In Rust 2024 a failing if-let drops its scrutinee before the else
            // expression. A match keeps it through its arms.
            if cx.sess().edition().at_least_rust_2024()
                && no.is_some()
                && !has_drop_free_scrutinee(cx, binding.init)
            {
                return None;
            }
            let (fallback, pattern) = named_cases(cx, binding)?;
            let input = source(binding.init.span)?;
            let input = indent_branch(&input, indent + 4)?;
            (input, pattern, fallback)
        }
        _ if !has_let_condition(condition)
            && let Some(cases) = direct_cases(cx, condition, true) =>
        {
            cases
        }
        _ if !structural_only && !has_let_condition(condition) && !is_in_const_context(cx) => {
            // The block drops boolean condition temporaries before either arm.
            let text = source(condition.span)?;
            let scrutinee = if matches!(
                condition.peel_drop_temps().kind,
                ExprKind::Path(_) | ExprKind::Lit(_)
            ) {
                predicate_as_option(&text)
            } else {
                let body = indent_branch(&text, indent + 4)?;
                let decision = predicate_as_option(&body);
                if cx.sess().edition().at_least_rust_2024() {
                    format!("{{\n{arm}{decision}\n{closing}}}")
                } else {
                    // Before 2024, a block's tail temporaries may outlive the
                    // block. Keep the predicate in a statement instead.
                    format!(
                        "{{\n{arm}let _explicit_decision = {decision};\n{arm}_explicit_decision\n{closing}}}"
                    )
                }
            };
            (scrutinee, MET.to_owned(), ABSENT.to_owned())
        }
        _ => return None,
    };
    let yes_text = indent_branch(&source(yes.span)?, indent + 4)?;
    let no_text = if let Some(no) = no {
        let separator = yes.span.with_lo(yes.span.hi()).with_hi(no.span.lo());
        if source(separator)?.trim() != "else" {
            return None;
        }
        match no.kind {
            ExprKind::If(..) => named_match(cx, no, indent + 4, structural_only)?,
            ExprKind::Block(..) => indent_branch(&source(no.span)?, indent + 4)?,
            _ => return None,
        }
    } else {
        "{}".to_owned()
    };
    let (first, second) = if yes_case == "_" {
        ((no_case, no_text), (yes_case, yes_text))
    } else {
        ((yes_case, yes_text), (no_case, no_text))
    };
    let replacement = format!(
        "match {scrutinee} {{\n{arm}{} => {},\n{arm}{} => {},\n{closing}}}",
        first.0, first.1, second.0, second.1,
    );
    if isolate {
        let outer = " ".repeat(indent - 4);
        Some(format!("{{\n{closing}{replacement};\n{outer}}}"))
    } else {
        Some(replacement)
    }
}

fn bool_match<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    scrutinee: &'tcx Expr<'tcx>,
    arms: &[Arm<'_>],
    structural_only: bool,
) -> Option<Vec<(Span, String)>> {
    let [first, second] = arms else {
        return None;
    };
    if !cx.sess().edition().at_least_rust_2018()
        || !cx.typeck_results().expr_ty(scrutinee).is_bool()
        || [first, second]
            .iter()
            .any(|arm| arm.guard.is_some() || arm.pat.span.from_expansion())
        || scrutinee.span.from_expansion()
    {
        return None;
    }
    let map = cx.sess().source_map();
    // A cfg attribute can hide a boolean arm from this compiler invocation.
    // Attributes inside the active bodies cannot change the set of arms.
    let mut start = expr.span.lo();
    for arm in [first, second] {
        let body = arm.body.span.source_callsite();
        if !expr.span.contains(body) || body.lo() < start {
            return None;
        }
        let header = expr.span.with_lo(start).with_hi(body.lo());
        if has_attributes(&map.span_to_snippet(header).ok()?) {
            return None;
        }
        start = body.hi();
    }
    if has_attributes(&map.span_to_snippet(expr.span.with_lo(start)).ok()?) {
        return None;
    }
    let pattern = |arm: &Arm<'_>| map.span_to_snippet(arm.pat.span).ok();
    let true_first = match (pattern(first)?.as_str(), pattern(second)?.as_str()) {
        ("true", "false" | "_") => true,
        ("false", "true" | "_") => false,
        _ => return None,
    };
    let (replacement, yes, no) =
        if let Some((value, yes, no)) = direct_cases(cx, scrutinee, false) {
            (value, yes, no)
        } else {
            if structural_only || is_in_const_context(cx) {
                return None;
            }
            let source = map.span_to_snippet(scrutinee.span).ok()?;
            (
                predicate_as_option(&source),
                MET.to_owned(),
                ABSENT.to_owned(),
            )
        };
    let (first_pattern, second_pattern) = if true_first { (yes, no) } else { (no, yes) };
    if first_pattern == "_" {
        let source = |span| map.span_to_snippet(span).ok();
        let first_arrow = first
            .pat
            .span
            .with_lo(first.pat.span.hi())
            .with_hi(first.body.span.lo());
        let second_arrow = second
            .pat
            .span
            .with_lo(second.pat.span.hi())
            .with_hi(second.body.span.lo());
        let opening = scrutinee
            .span
            .with_lo(scrutinee.span.hi())
            .with_hi(first.pat.span.lo());
        let closing = second
            .body
            .span
            .with_lo(second.body.span.hi())
            .with_hi(expr.span.hi());
        let separator = first
            .body
            .span
            .with_lo(first.body.span.hi())
            .with_hi(second.pat.span.lo());
        let separator = source(separator)?;
        if first.body.span.from_expansion()
            || second.body.span.from_expansion()
            || source(first_arrow)?.trim() != "=>"
            || source(second_arrow)?.trim() != "=>"
            || !matches!(separator.trim(), "" | ",")
            || source(opening)?.trim() != "{"
            || !matches!(source(closing)?.trim().trim_start_matches(',').trim(), "}")
        {
            return None;
        }
        let first_body = source(first.body.span)?;
        let second_body = source(second.body.span)?;
        let spacing = match separator.split_once(',') {
            Some((_, after_comma)) => after_comma,
            None => &separator,
        };
        let swapped =
            format!("{second_pattern} => {second_body},{spacing}{first_pattern} => {first_body}");
        return Some(vec![
            (scrutinee.span, replacement),
            (first.pat.span.with_hi(second.body.span.hi()), swapped),
        ]);
    }
    Some(vec![
        (scrutinee.span, replacement),
        (first.pat.span, first_pattern),
        (second.pat.span, second_pattern),
    ])
}

fn can_suggest(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    // The replacement refers to `::core` through the extern prelude.
    !cx.tcx.hir_root_module().item_ids.iter().any(|id| {
        matches!(cx.tcx.hir_item(*id).kind, ItemKind::ExternCrate(Some(original), name)
            if name.name == sym::core && original != sym::core)
    }) && !cx.tcx.hir_parent_iter(expr.hir_id).any(|(id, _)| {
        cx.tcx
            .hir_attrs(id)
            .iter()
            .any(|attr| matches!(attr, Attribute::Parsed(AttributeKind::NoImplicitPrelude)))
    }) && macro_input::outside_macro_input(cx, expr.span)
}

impl Explicit019NoIf {
    fn inside_replacement(&self, cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
        cx.tcx.hir_parent_iter(expr.hir_id).any(|(parent, _)| {
            self.replacements
                .get(&parent)
                .is_some_and(|spans| spans.iter().any(|span| span.contains(expr.span)))
        })
    }
}

fn is_enum_conversion(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    input: &Expr<'_>,
    arms: &[Arm<'_>],
) -> bool {
    let [first, second] = arms else {
        return false;
    };
    let owner = expr.hir_id.owner.def_id;
    let Some(implementation) = cx.tcx.trait_impl_of_assoc(owner.to_def_id()) else {
        return false;
    };
    let trait_id = cx.tcx.impl_trait_ref(implementation).skip_binder().def_id;
    if !cx.tcx.is_diagnostic_item(sym::From, trait_id)
        || !cx.typeck_results().expr_ty(input).is_bool()
        || !cx
            .typeck_results()
            .expr_ty(expr)
            .ty_adt_def()
            .is_some_and(|adt| adt.is_enum())
    {
        return false;
    }

    let body = cx.tcx.hir_body_owned_by(owner);
    let [argument] = body.params else {
        return false;
    };
    let PatKind::Binding(_, binding, _, None) = argument.pat.kind else {
        return false;
    };
    let ExprKind::Path(ref path) = input.kind else {
        return false;
    };
    let ExprKind::Block(block, None) = body.value.kind else {
        return false;
    };
    if cx.qpath_res(path, input.hir_id) != Res::Local(binding)
        || !block.stmts.is_empty()
        || !block.expr.is_some_and(|tail| tail.hir_id == expr.hir_id)
        || first.guard.is_some()
        || second.guard.is_some()
    {
        return false;
    }

    let map = cx.sess().source_map();
    let literals = (
        map.span_to_snippet(first.pat.span),
        map.span_to_snippet(second.pat.span),
    );
    let (Ok(first_pattern), Ok(second_pattern)) = literals else {
        return false;
    };
    if !matches!(
        (first_pattern.as_str(), second_pattern.as_str()),
        ("true", "false") | ("false", "true")
    ) {
        return false;
    }

    let variant = |arm: &Arm<'_>| match arm.body.kind {
        ExprKind::Path(ref path) => match cx.qpath_res(path, arm.body.hir_id) {
            Res::Def(DefKind::Ctor(CtorOf::Variant, CtorKind::Const), variant) => Some(variant),
            _ => None,
        },
        _ => None,
    };
    matches!((variant(first), variant(second)), (Some(first), Some(second)) if first != second)
}

impl<'tcx> LateLintPass<'tcx> for Explicit019NoIf {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        // A `while` desugars to an `if` no one wrote, and a macro's `if` is
        // the macro author's; what is linted is what someone typed here.
        if expr.span.from_expansion() {
            return;
        }

        if let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = expr.kind
            && cx.typeck_results().expr_ty(scrutinee).peel_refs().is_bool()
        {
            if is_enum_conversion(cx, expr, scrutinee, arms) {
                return;
            }
            span_lint_and_then(
                cx,
                EXPLICIT019_NO_IF,
                expr.span,
                "a match over a boolean needs an enum that names the outcomes",
                |diagnostic| {
                    if !self.inside_replacement(cx, expr)
                        && can_suggest(cx, expr)
                        && let Some(edits) =
                            bool_match(cx, expr, scrutinee, arms, self.structural_only)
                    {
                        self.replacements
                            .insert(expr.hir_id, edits.iter().map(|(span, _)| *span).collect());
                        diagnostic.multipart_suggestion(
                            "match the outcomes directly",
                            edits,
                            rustc_errors::Applicability::MachineApplicable,
                        );
                    } else {
                        diagnostic.help(ENUM_HELP);
                    }
                },
            );
            return;
        }
        if !matches!(expr.kind, ExprKind::If(..)) {
            return;
        }

        span_lint_and_then(
            cx,
            EXPLICIT019_NO_IF,
            expr.span,
            "`if` leaves an outcome implicit: the path not taken has no name here",
            |diagnostic| {
                let is_else_if = matches!(cx.tcx.parent_hir_node(expr.hir_id), Node::Expr(parent)
                    if matches!(parent.kind, ExprKind::If(_, _, Some(no)) if no.hir_id == expr.hir_id));
                if cx.sess().edition().at_least_rust_2018()
                    && !is_else_if
                    && !self.inside_replacement(cx, expr)
                    && can_suggest(cx, expr)
                    && let Some(indent) = indent_of(cx, expr.span)
                    && let Some(replacement) = named_match(cx, expr, indent, self.structural_only)
                {
                    // The outer replacement includes its arms. Nested edits take
                    // effect on Cargo's next compiler pass.
                    self.replacements.insert(expr.hir_id, vec![expr.span]);
                    diagnostic.span_suggestion(
                        expr.span,
                        "name both outcomes in a match",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help(ENUM_HELP);
                }
            },
        );
    }

    fn check_arm(&mut self, cx: &LateContext<'tcx>, arm: &'tcx Arm<'tcx>) {
        let Some(guard) = arm.guard else {
            return;
        };

        if arm.span.from_expansion() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT019_NO_IF,
            guard.span,
            "a guard is an `if` on an arm: its false answer falls through to the next arm without a name",
            None,
            "put the question in the pattern, or match on an enum that names its outcomes",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn legacy_editions() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui_legacy");
}

#[test]
fn structural_rewrites() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_structural")
        .dylint_toml("explicit019_structural_only = true")
        .run();
}
