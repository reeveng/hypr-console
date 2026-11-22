//! EXPLICIT023: `let … else` is forbidden, for the reason EXPLICIT019 is.
//!
//! 019 exempted it, and the exemption was wrong. What it said was that both of
//! a `let … else`'s outcomes are already written and one of them has to leave,
//! so there is no path decided by omission. The first half of that is not true.
//! One outcome is written as a pattern and the other is written as the word
//! `else`, which is not a name for anything -- `let Some(held) = asked else`
//! never says `None`, exactly as the `if let` 019 forbids never says it. The
//! case that leaves is the case no one had to think about, which is the whole
//! of what this suite is against.
//!
//! And it is the second spelling of a decision the workspace already has one
//! spelling for. Two forms that mean the same thing are two shapes a reader has
//! to hold, and the one that is not a `match` is the one that does not compose:
//! it cannot bind more than the pattern binds, it cannot have a third case, and
//! the day it needs one it is rewritten into the `match` it should have been.
//!
//! What replaces it is a `match` in the initializer, which is the same
//! statement one line longer:
//!
//!     let said = match std::fs::read_to_string(&at) {
//!         Ok(said) => said,
//!         Err(fault) => return Err(fault.to_string()),
//!     };
//!
//! That is not the nesting a guard clause is written to avoid. A `match` on the
//! right of a `let` is an expression, the binding stays where it was, and
//! everything after it keeps its indentation -- EXPLICIT013 leaves it alone for
//! the same reason. What changes is that `Err` is on the screen with a name on
//! it, where EXPLICIT016 can ask that the variants be named too.
//!
//! An irrefutable `let` is not this and never arrives here. `let Ok(at) =
//! at(&home);` has no `else` because `Result<T, Never>` has no `Err` to meet;
//! that is EXPLICIT002 being kept, not a decision being hidden.
//!
//! It was registered `Warn` when it was written, because the tree broke it
//! everywhere that day -- `let … else` was the guard clause here. It is `Deny`
//! now: the last call site is gone, and by the ratchet's one law the level
//! does not go back.

#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

mod local_enums;
#[path = "../../shared/local_type_path.rs"]
mod local_type_path;

use clippy_utils::{
    diagnostics::span_lint_and_then, is_lang_item_or_ctor,
    macros::{is_panic, root_macro_call},
};
use rustc_errors::Applicability;
use rustc_hir::{
    BindingMode, Block, Expr, ExprKind, LangItem, LetStmt, LocalSource, MatchSource, Pat, PatExpr,
    PatExprKind, PatKind, QPath, Stmt, StmtKind,
    def::Res,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::TyKind;
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT023: `let … else` is forbidden. Its `else` is not a name for
    /// the case that did not bind, and a decision this workspace can write as
    /// a `match` is one it writes as a `match`. Put the `match` in the
    /// initializer -- `let held = match value { Some(held) => held, None => …
    /// };` -- and the binding stays where it was.
    pub EXPLICIT023_NO_LET_ELSE,
    Deny,
    "`let … else` is forbidden; write the `match` in the initializer"
}

struct Binding<'hir> {
    pattern: &'hir Pat<'hir>,
    modifier: &'static str,
    name: String,
    carried: String,
    shorthand: Option<String>,
}

struct Alternatives {
    missing: Vec<String>,
    // A failing match on this pattern has an empty Option as its scrutinee.
    empty_on_failure: bool,
}

fn collect_variant<'hir>(
    cx: &LateContext<'_>,
    pattern: &Pat<'_>,
    path: &QPath<'_>,
    fields: impl IntoIterator<Item = &'hir Pat<'hir>>,
    source: &str,
    bindings: &mut Vec<Binding<'hir>>,
) -> Option<Alternatives> {
    let missing = local_enums::siblings(cx, pattern, path)?;
    for field in fields {
        if !collect_pattern(cx, field, source, bindings)?.missing.is_empty() {
            return None;
        }
    }
    Some(Alternatives { missing, empty_on_failure: false })
}

fn collect_sequence<'hir>(
    cx: &LateContext<'_>,
    fields: impl IntoIterator<Item = &'hir Pat<'hir>>,
    len: usize,
    delimiters: (&str, &str),
    source: &str,
    bindings: &mut Vec<Binding<'hir>>,
) -> Option<Alternatives> {
    let (open, close) = delimiters;
    let mut missing = Vec::new();
    let mut slots = vec!["_".to_owned(); len];
    // A sequence fails whenever an element fails. The remaining positions
    // are irrelevant, so this names all failures without a product of cases.
    for (index, field) in fields.into_iter().enumerate() {
        let alternatives = collect_pattern(cx, field, source, bindings)?;
        for failure in alternatives.missing {
            slots[index] = failure;
            missing.push(format!("{open}{}{close}", slots.join(", ")));
        }
        slots[index] = "_".to_owned();
    }
    // Even a failed element with no payload can leave an owned sibling.
    Some(Alternatives { missing, empty_on_failure: false })
}

fn collect_pattern<'hir>(
    cx: &LateContext<'_>,
    pattern: &'hir Pat<'hir>,
    source: &str,
    bindings: &mut Vec<Binding<'hir>>,
) -> Option<Alternatives> {
    if pattern.span.from_expansion() {
        return None;
    }
    let unconditional = Alternatives {
        missing: Vec::new(),
        empty_on_failure: false,
    };
    match pattern.kind {
        PatKind::TupleStruct(ref path, [field], rest) if rest.as_opt_usize().is_none() => {
            let constructor = cx.qpath_res(path, pattern.hir_id).opt_def_id()?;
            let standard = [
                (
                    LangItem::OptionSome,
                    "::core::option::Option::Some",
                    "::core::option::Option::None",
                    true,
                ),
                (
                    LangItem::ResultOk,
                    "::core::result::Result::Ok",
                    "::core::result::Result::Err(_)",
                    false,
                ),
                (
                    LangItem::ResultErr,
                    "::core::result::Result::Err",
                    "::core::result::Result::Ok(_)",
                    false,
                ),
            ]
            .into_iter()
            .find_map(|(item, matched, alternate, empty)| {
                is_lang_item_or_ctor(cx, constructor, item).then_some((matched, alternate, empty))
            });
            let Some((matched, alternate, empty_alternate)) = standard else {
                return collect_variant(cx, pattern, path, [field], source, bindings);
            };
            let field = collect_pattern(cx, field, source, bindings)?;
            let empty_on_failure = empty_alternate && field.missing.is_empty();
            let mut missing: Vec<_> = field
                .missing
                .into_iter()
                .map(|fallback| format!("{matched}({fallback})"))
                .collect();
            missing.push(alternate.to_owned());
            Some(Alternatives {
                missing,
                empty_on_failure,
            })
        }
        PatKind::TupleStruct(ref path, fields, _) => {
            collect_variant(cx, pattern, path, fields, source, bindings)
        }
        PatKind::Struct(ref path, fields, _) => {
            let first_binding = bindings.len();
            let variants = collect_variant(
                cx, pattern, path, fields.iter().map(|field| field.pat), source, bindings,
            )?;
            for field in fields.iter().filter(|field| field.is_shorthand) {
                let binding = bindings[first_binding..]
                    .iter_mut()
                    .find(|binding| binding.pattern.hir_id == field.pat.hir_id)?;
                binding.shorthand = Some(
                    cx.sess().source_map().span_to_snippet(field.ident.span).ok()?,
                );
            }
            Some(variants)
        }
        PatKind::Expr(PatExpr { kind: PatExprKind::Path(path), .. }) => {
            collect_variant(cx, pattern, path, [], source, bindings)
        }
        PatKind::Tuple(fields, rest) if rest.as_opt_usize().is_none() => {
            let close = if fields.len() == 1 { ",)" } else { ")" };
            collect_sequence(cx, fields, fields.len(), ("(", close), source, bindings)
        }
        PatKind::Slice(before, None, after) => {
            let TyKind::Array(_, length) = cx.typeck_results().pat_ty(pattern).peel_refs().kind() else {
                return None;
            };
            let len = before.len() + after.len();
            if length.try_to_target_usize(cx.tcx)? != u64::try_from(len).ok()? {
                return None;
            }
            collect_sequence(cx, before.iter().chain(after), len, ("[", "]"), source, bindings)
        }
        PatKind::Ref(inner, ..) => {
            let inner = collect_pattern(cx, inner, source, bindings)?;
            inner.missing.is_empty().then_some(unconditional)
        }
        PatKind::Wild => Some(unconditional),
        PatKind::Binding(mode, _, ident, None) => {
            let modifier = match mode {
                BindingMode::NONE => "",
                BindingMode::MUT => "mut ",
                _ => return None,
            };
            let name = cx.sess().source_map().span_to_snippet(ident.span).ok()?;
            let carried = fresh_name(source, bindings, "matched", &name);
            bindings.push(Binding {
                pattern,
                modifier,
                name,
                carried,
                shorthand: None,
            });
            Some(unconditional)
        }
        _ => None,
    }
}

fn fresh_name(source: &str, bindings: &[Binding<'_>], prefix: &str, name: &str) -> String {
    let stem = name.trim_start_matches("r#").trim_start_matches('_');
    let stem = if stem.is_empty() { "value" } else { stem };
    let base = format!("{prefix}_{stem}");
    let available = |candidate: &str| {
        !source.contains(candidate) && bindings.iter().all(|binding| binding.carried != candidate)
    };
    if available(&base) {
        return base;
    }
    (2..)
        .map(|suffix| format!("{base}_{suffix}"))
        .find(|candidate| available(candidate))
        .expect("a finite file cannot contain every numbered binding")
}

fn local_place(cx: &LateContext<'_>, input: &Expr<'_>) -> bool {
    match input.kind {
        ExprKind::Path(ref path) => matches!(cx.qpath_res(path, input.hir_id), Res::Local(_)),
        ExprKind::Field(base, _) | ExprKind::AddrOf(_, _, base) => local_place(cx, base),
        _ => false,
    }
}

fn call_value(input: &Expr<'_>) -> bool {
    matches!(input.kind, ExprKind::Call(..) | ExprKind::MethodCall(..)
        | ExprKind::Match(_, _, MatchSource::TryDesugar(_)))
}

fn staged_call(cx: &LateContext<'_>, input: &Expr<'_>) -> bool {
    let ExprKind::Block(block, None) = input.kind else { return false; };
    let ([Stmt { kind: StmtKind::Let(local), .. }], Some(value)) = (block.stmts, block.expr) else {
        return false;
    };
    let PatKind::TupleStruct(ref path, [binding], _) = local.pat.kind else { return false; };
    let PatKind::Binding(BindingMode::NONE, local_id, _, None) = binding.kind else { return false; };
    local.els.is_none()
        && local.init.is_some_and(call_value)
        && cx.qpath_res(path, local.pat.hir_id).opt_def_id()
            .is_some_and(|id| is_lang_item_or_ctor(cx, id, LangItem::ResultOk))
        && matches!(value.kind, ExprKind::Path(ref path)
            if cx.qpath_res(path, value.hir_id) == Res::Local(local_id))
}

fn scoped_value(cx: &LateContext<'_>, input: &Expr<'_>) -> bool {
    call_value(input) || matches!(input.kind, ExprKind::Tup(fields)
        if fields.iter().all(|field| local_place(cx, field) || call_value(field) || staged_call(cx, field)))
}

fn indented(text: &str, source_indent: &str, target_indent: &str) -> Option<String> {
    let mut offset = 0;
    for token in tokenize(text, FrontmatterAllowed::No) {
        let start = offset;
        offset += token.len as usize;
        if token.kind != TokenKind::Whitespace && text[start..offset].contains('\n') {
            return None;
        }
    }
    Some(text.replace(&format!("\n{source_indent}"), &format!("\n{target_indent}")))
}

fn failure_action(cx: &LateContext<'_>, block: &Block<'_>) -> Option<Span> {
    let leave = match (block.stmts, block.expr) {
        ([], Some(leave)) => leave,
        ([Stmt { kind: StmtKind::Semi(leave), .. }], None) => leave,
        _ => return None,
    };
    if !leave.span.from_expansion()
        && matches!(leave.kind, ExprKind::Ret(_) | ExprKind::Break(..) | ExprKind::Continue(_))
    {
        return Some(leave.span);
    }
    let panic = root_macro_call(leave.span)?;
    (is_panic(cx, panic.def_id)
        && !panic.span.from_expansion()
        && block.span.contains(panic.span)
        && cx.typeck_results().expr_ty(leave).is_never())
    .then_some(panic.span)
}

fn named_match(cx: &LateContext<'_>, stmt: &Stmt<'_>, binding: &LetStmt<'_>) -> Option<String> {
    if !cx.sess().edition().at_least_rust_2024() || binding.ty.is_some() || binding.super_.is_some()
    {
        return None;
    }
    let input = binding.init?;
    let leave = failure_action(cx, binding.els?)?;
    if [binding.pat.span, input.span]
        .iter()
        .any(|span| span.from_expansion())
    {
        return None;
    }

    let map = cx.sess().source_map();
    let original = map.span_to_snippet(stmt.span).ok()?;
    if original.contains("//")
        || original.contains("/*")
        || original.contains('#')
        || !original.starts_with("let ")
        || !original.ends_with(';')
    {
        return None;
    }
    let file = map.lookup_source_file(stmt.span.lo());
    let source = file.src.as_ref()?;
    let mut values = Vec::new();
    let alternatives = collect_pattern(cx, binding.pat, source, &mut values)?;
    if alternatives.missing.is_empty() {
        return None;
    }
    let scoped = !local_place(cx, input);
    if scoped
        && (!scoped_value(cx, input)
            || (!alternatives.empty_on_failure
                && cx
                    .typeck_results()
                    .expr_ty(input)
                    .needs_drop(cx.tcx, cx.typing_env())))
    {
        // A successful match can move its bindings out of a temporary. On
        // failure, however, let-else drops the temporary before the else
        // block. Only match an owned value that has no destructor to run on
        // that path.
        return None;
    }
    let mut pattern = map.span_to_snippet(binding.pat.span).ok()?;
    for value in values.iter().rev() {
        let start = (value.pattern.span.lo() - binding.pat.span.lo()).0 as usize;
        let end = (value.pattern.span.hi() - binding.pat.span.lo()).0 as usize;
        match &value.shorthand {
            Some(field) => pattern.replace_range(start..end, &format!("{field}: {}", value.carried)),
            None => pattern.replace_range(start..end, &value.carried),
        }
    }
    let written = values
        .iter()
        .map(|value| format!("{}{}", value.modifier, value.name))
        .collect::<Vec<_>>()
        .join(", ");
    let carried = values
        .iter()
        .map(|value| value.carried.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let (written, carried) = if values.len() == 1 {
        (written, carried)
    } else {
        (format!("({written})"), format!("({carried})"))
    };
    let input = map.span_to_snippet(input.span).ok()?;
    let outer = map.indentation_before(stmt.span)?;
    let composite = matches!(binding.pat.kind, PatKind::Tuple(..) | PatKind::Slice(..));
    let failure_indent = map.indentation_before(leave).unwrap_or_default();
    let step = failure_indent.strip_prefix(&outer).filter(|step| !step.is_empty())
        .filter(|_| composite).unwrap_or("    ");
    let leave = map.span_to_snippet(leave).ok()?;
    if leave.contains('\n') || (!composite && (input.contains('\n') || pattern.contains('\n'))) {
        return None;
    }
    let decision = |input: &str, indent: &str| {
        let arm = format!("{indent}{step}");
        let pattern = indented(&pattern, &outer, &arm)?;
        let separator = if composite { format!("\n{arm}| ") } else { " | ".to_owned() };
        let fallback = alternatives.missing.join(&separator);
        Some(format!(
            "match {input} {{\n{arm}{pattern} => {carried},\n{arm}{fallback} => {leave},\n{indent}}}"
        ))
    };
    let initializer = if scoped {
        let prefix = if alternatives.empty_on_failure {
            "optional"
        } else {
            "outcome"
        };
        let name = values.first().map_or("value", |value| value.name.as_str());
        let held = fresh_name(source, &values, prefix, name);
        let inner = format!("{outer}{step}");
        let decision = decision(&held, &inner)?;
        let input = indented(&input, &outer, &inner)?;
        format!("{{\n{inner}let {held} = {input};\n\n{inner}{decision}\n{outer}}}")
    } else {
        decision(&input, &outer)?
    };
    if values.is_empty() {
        Some(format!("{initializer};"))
    } else {
        Some(format!("let {written} = {initializer};"))
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit023NoLetElse {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        let StmtKind::Let(binding) = stmt.kind else {
            return;
        };

        if binding.els.is_none() || !matches!(binding.source, LocalSource::Normal) {
            return;
        }

        // A macro's `let … else` is the macro author's.
        if stmt.span.from_expansion() {
            return;
        }

        span_lint_and_then(
            cx,
            EXPLICIT023_NO_LET_ELSE,
            stmt.span,
            "`let … else` leaves an outcome without a name: the case that did not bind is spelled `else`",
            |diagnostic| {
                if let Some(replacement) = named_match(cx, stmt, binding) {
                    diagnostic.span_suggestion(
                        stmt.span,
                        "name the matching and failed cases",
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("write the decision as a `match` in the initializer -- `let held = match value { Some(held) => held, None => … };` -- so the case that leaves is named as well as the case that binds");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
