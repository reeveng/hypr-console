#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{
    diagnostics::span_lint_and_then, source::reindent_multiline,
    usage::contains_return_break_continue_macro,
};
use rustc_errors::Applicability;
use rustc_hir::{
    Expr, ExprKind, Node, UnOp,
    attrs::AttributeKind,
    def::{DefKind, Res},
    def_id::CRATE_DEF_ID,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::adjustment::{Adjust, DerefAdjustKind};
use rustc_middle::ty::{self, Ty};

mod enum_defaults;
#[path = "../../shared/local_type_path.rs"]
mod local_type_path;
#[path = "../../shared/macro_input.rs"]
mod macro_input;

dylint_linting::declare_late_lint! {
    /// EXPLICIT033: an absence answered by a value no one wrote down.
    ///
    /// EXPLICIT001 is this rule over a `Result`, where what is swallowed is
    /// someone else's error. This is the same three methods over an `Option`,
    /// where there is no error to swallow and the fault is quieter: a `None`
    /// meant something, and `unwrap_or_default()` is the one spelling of the
    /// answer that never says what.
    ///
    /// `opt.unwrap_or(0)` reads as a number and is a sentinel -- EXPLICIT025's
    /// fault, arrived at by a different road. `opt.unwrap_or_default()` does not
    /// even name the value: the reader has to know the type to know whether the
    /// machine just ran on an empty string, a zero, a false or an empty list,
    /// and a type that changes under it changes the answer without touching the
    /// line. `unwrap_or_else(|| ...)` is a block with no name on it at the one
    /// place a name was owed.
    ///
    /// A `match` with both arms costs two lines and says which two things can
    /// be true. Where the absence is someone else's business, hand it on:
    /// return the `Option`, or `ok_or` a fault that says what was missing.
    /// Where it really does have a chosen answer, the answer is written at the
    /// call site in full, which is the whole of what this rule asks for.
    ///
    /// `Option::unwrap_or` over a default that is itself the subject -- a
    /// palette's fallback color, a layout's minimum -- is the shape that wants
    /// a named constant rather than a literal, and then the `match` reads as
    /// the sentence it is.
    pub EXPLICIT033_NO_UNNAMED_ABSENCE,
    Deny,
    "an `Option`'s `None` must be answered in a `match` that names it, not by `unwrap_or`"
}

// `Option`, whatever it is called at the point of use, for the reason
// EXPLICIT001 asks the type rather than the spelling: an alias answers the same
// as `std`'s own.
fn is_option(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        rustc_middle::ty::Adt(adt, _) => cx
            .tcx
            .is_diagnostic_item(rustc_span::sym::Option, adt.did()),
        _ => false,
    }
}

// The three that produce a value. `ok_or`, `ok_or_else` and `map_or` are not
// here: the first two turn the absence into a fault, which is naming it, and
// `map_or` is asked about in EXPLICIT025 where it is a sentinel or not on the
// strength of what it hands back.
const ANSWERS: &[&str] = &["unwrap_or", "unwrap_or_else", "unwrap_or_default"];

fn explicit_default<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    use_site: &Expr<'_>,
    budget: u8,
) -> Option<String> {
    let next = budget.checked_sub(1)?;
    let literal = match ty.kind() {
        ty::Bool => "false",
        ty::Int(_) | ty::Uint(_) => "0",
        ty::Float(_) => "0.0",
        ty::Char => "'\\0'",
        ty::Ref(_, inner, mutability) if mutability.is_not() && inner.is_str() => "\"\"",
        ty::Ref(_, inner, mutability) if mutability.is_not() && inner.is_slice() => "&[]",
        ty::Ref(_, inner, mutability) if mutability.is_mut() && inner.is_slice() => "&mut []",
        ty::Tuple(items) => {
            let elements = items
                .iter()
                .map(|item| explicit_default(cx, item, use_site, next))
                .collect::<Option<Vec<_>>>()?;
            let trailer = if elements.len() == 1 { "," } else { "" };
            return Some(format!("({}{trailer})", elements.join(", ")));
        }
        ty::Array(_, len) if len.try_to_target_usize(cx.tcx) == Some(0) => "[]",
        ty::Array(element, len) if cx.type_is_copy_modulo_regions(*element) => {
            let length = len.try_to_target_usize(cx.tcx)?;
            return Some(format!(
                "[{}; {length}]",
                explicit_default(cx, *element, use_site, next)?
            ));
        }
        ty::Adt(adt, _)
            if cx
                .tcx
                .is_diagnostic_item(rustc_span::sym::Option, adt.did()) =>
        {
            "::core::option::Option::None"
        }
        ty::Adt(adt, _) if adt.is_enum() => {
            return enum_defaults::local_unit_variant(cx, ty, use_site.hir_id);
        }
        ty::Adt(adt, _) if !rustc_hir::find_attr!(cx.tcx, CRATE_DEF_ID, NoStd) => {
            if cx.tcx.lang_items().string() == Some(adt.did()) {
                "::std::string::String::new()"
            } else if cx.tcx.is_diagnostic_item(rustc_span::sym::Vec, adt.did()) {
                // The standard Default implementation is for the global allocator.
                "::std::vec::Vec::new()"
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some(literal.to_owned())
}

fn inert_fallback(cx: &LateContext<'_>, value: &Expr<'_>) -> bool {
    if value.span.from_expansion() || !cx.typeck_results().expr_adjustments(value).is_empty() {
        return false;
    }
    match value.kind {
        ExprKind::Lit(_) => true,
        ExprKind::Unary(UnOp::Neg, literal) => matches!(literal.kind, ExprKind::Lit(_)),
        ExprKind::Path(ref path) => matches!(
            cx.qpath_res(path, value.hir_id),
            Res::Local(_)
                | Res::Def(
                    DefKind::Const { .. }
                        | DefKind::AssocConst { .. }
                        | DefKind::Ctor(_, rustc_hir::def::CtorKind::Const),
                    _,
                )
        ),
        ExprKind::Field(base, _) => {
            !cx.typeck_results()
                .expr_ty(base)
                .ty_adt_def()
                .is_some_and(|adt| adt.is_union())
                && inert_fallback(cx, base)
        }
        ExprKind::AddrOf(rustc_hir::BorrowKind::Ref, rustc_hir::Mutability::Not, base) => {
            inert_fallback(cx, base)
        }
        ExprKind::Tup(fields) | ExprKind::Array(fields) => {
            fields.iter().all(|field| inert_fallback(cx, field))
        }
        _ => false,
    }
}

fn fixed_type_fallback(cx: &LateContext<'_>, value: &Expr<'_>) -> bool {
    if inert_fallback(cx, value) {
        return true;
    }
    let types = cx.typeck_results();
    if value.span.from_expansion() || !types.expr_adjustments(value).is_empty() {
        return false;
    }
    match value.kind {
        ExprKind::Call(callee, _) => match types.expr_ty(callee).kind() {
            ty::FnDef(id, _) => cx.tcx.generics_of(*id).count() == 0,
            _ => false,
        },
        ExprKind::Struct(..) => {
            matches!(types.expr_ty(value).kind(), ty::Adt(_, args) if args.is_empty())
        }
        ExprKind::Tup(fields) | ExprKind::Array(fields) => {
            fields.iter().all(|field| fixed_type_fallback(cx, field))
        }
        _ => false,
    }
}

fn format_fallback(fallback: String, indent: usize) -> Option<String> {
    if !fallback.contains('\n') {
        return Some(fallback);
    }
    let mut offset = 0;
    for token in tokenize(&fallback, FrontmatterAllowed::No) {
        let end = offset + usize::try_from(token.len).ok()?;
        if matches!(token.kind, TokenKind::Literal { .. }) && fallback[offset..end].contains('\n')
        {
            return None;
        }
        offset = end;
    }
    Some(reindent_multiline(&fallback, true, Some(indent)))
}

fn match_fallback<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    receiver: &Expr<'tcx>,
    method: &str,
    arguments: &[Expr<'tcx>],
) -> Option<String> {
    let types = cx.typeck_results();
    let received = types.expr_ty_adjusted(receiver);
    let adjustments = types.expr_adjustments(receiver);
    let resolved = types.type_dependent_def_id(expr.hir_id)?;
    let implementation = cx.tcx.inherent_impl_of_assoc(resolved)?;
    if !cx.sess().edition().at_least_rust_2018()
        || !is_option(
            cx,
            cx.tcx
                .type_of(implementation)
                .instantiate_identity()
                .skip_norm_wip(),
        )
        || !matches!(received.kind(), ty::Adt(..))
        || (!adjustments.is_empty()
            && (!cx.type_is_copy_modulo_regions(received)
                || adjustments.iter().any(|adjustment| {
                    !matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin))
                })))
        || expr.span.from_expansion()
        || receiver.span.from_expansion()
        || !macro_input::outside_macro_input(cx, expr.span)
        || cx.tcx.hir_parent_iter(expr.hir_id).any(|(id, _)| {
            cx.tcx.hir_attrs(id).iter().any(|attr| {
                matches!(
                    attr,
                    rustc_hir::Attribute::Parsed(AttributeKind::NoImplicitPrelude)
                )
            })
        }) || cx.tcx.hir_root_module().item_ids.iter().any(|id| {
        matches!(cx.tcx.hir_item(*id).kind, rustc_hir::ItemKind::ExternCrate(Some(original), name)
                if name.name == rustc_span::sym::core && original != rustc_span::sym::core)
    }) {
        return None;
    }

    // Parentheses on the left of a binary operator keep a leading match
    // from being parsed as a complete statement.
    let parenthesize = match cx.tcx.parent_hir_node(expr.hir_id) {
        Node::LetStmt(_) | Node::Stmt(_) | Node::Block(_) | Node::Arm(_) | Node::ExprField(_) => {
            false
        }
        Node::Expr(parent) => match parent.kind {
            ExprKind::Ret(_)
            | ExprKind::MethodCall(..)
            | ExprKind::Field(..)
            | ExprKind::Index(..)
            | ExprKind::Tup(_)
            | ExprKind::Array(_)
            | ExprKind::Closure(_)
            | ExprKind::Match(..)
            | ExprKind::If(..) => false,
            ExprKind::Call(_, args) if args.iter().any(|arg| arg.hir_id == expr.hir_id) => false,
            ExprKind::Assign(_, value, _) | ExprKind::AssignOp(_, _, value)
                if value.hir_id == expr.hir_id =>
            {
                false
            }
            ExprKind::Binary(_, left, _) => left.hir_id == expr.hir_id,
            ExprKind::Unary(..) | ExprKind::Cast(..) | ExprKind::AddrOf(..) => true,
            _ => return None,
        },
        _ => return None,
    };

    let map = cx.sess().source_map();
    let written = map.span_to_snippet(expr.span).ok()?;
    if written.contains("//") || written.contains("/*") || written.contains('#') {
        return None;
    }
    let payload = types.expr_ty(expr);
    let copies_payload = cx.type_is_copy_modulo_regions(payload);
    let (fallback, eager) = match (method, arguments) {
        ("unwrap_or_default", []) => (explicit_default(cx, payload, expr, 8)?, false),
        ("unwrap_or", [value]) if fixed_type_fallback(cx, value) => {
            // A tuple keeps eager evaluation and moves in their original
            // order. Only inert Copy arguments may move into the None arm.
            (
                map.span_to_snippet(value.span).ok()?,
                !copies_payload || !inert_fallback(cx, value),
            )
        }
        ("unwrap_or_else", [argument]) if !argument.span.from_expansion() => {
            let ExprKind::Closure(closure) = argument.kind else {
                return None;
            };
            let value = cx.tcx.hir_body(closure.body).value;
            // A capture-free closure has nothing to move or drop when the
            // Some arm wins. Keep control flow local to its original closure.
            if closure.kind != rustc_hir::ClosureKind::Closure
                || closure.constness == rustc_hir::Constness::Const
                || !closure.bound_generic_params.is_empty()
                || types
                    .closure_min_captures_flattened(closure.def_id)
                    .next()
                    .is_some()
                || contains_return_break_continue_macro(value)
                || !matches!(closure.fn_decl.output, rustc_hir::FnRetTy::DefaultReturn(_))
            {
                return None;
            }
            (map.span_to_snippet(value.span).ok()?, false)
        }
        _ => return None,
    };

    let receiver = map.span_to_snippet(receiver.span).ok()?;
    let receiver = if adjustments.is_empty() {
        receiver
    } else {
        format!("{}({receiver})", "*".repeat(adjustments.len()))
    };
    let source = map.lookup_source_file(expr.span.lo());
    let source = source.src.as_ref()?;
    let binding = ["present", "option_value", "explicit_present"]
        .into_iter()
        .find(|name| !source.contains(name))?;
    let outer = map.indentation_before(expr.span)?;
    let inner = format!("{outer}    ");
    let fallback = format_fallback(fallback, inner.len())?;
    let some = format!("::core::option::Option::Some({binding})");
    let none = "::core::option::Option::None";
    let decision = if eager {
        let alternate = ["alternate", "fallback_value", "absence_value"]
            .into_iter()
            .find(|name| !source.contains(name))?;
        let present = if copies_payload {
            format!("({some}, _) => {binding},")
        } else {
            format!("({some}, {alternate}) => {{ ::core::mem::drop({alternate}); {binding} }},")
        };
        format!(
            "match ({receiver}, {fallback}) {{\n{inner}{present}\n{inner}({none}, {alternate}) => {alternate},\n{outer}}}"
        )
    } else {
        format!(
            "match {receiver} {{\n{inner}{some} => {binding},\n{inner}{none} => {fallback},\n{outer}}}"
        )
    };
    Some(if parenthesize {
        format!("({decision})")
    } else {
        decision
    })
}

impl<'tcx> LateLintPass<'tcx> for Explicit033NoUnnamedAbsence {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(path, receiver, arguments, _) = expr.kind else {
            return;
        };
        if !ANSWERS.contains(&path.ident.name.as_str()) {
            return;
        }
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_option(cx, recv_ty) {
            return;
        }

        span_lint_and_then(
            cx,
            EXPLICIT033_NO_UNNAMED_ABSENCE,
            expr.span,
            format!(
                "`{}` answers a `None` without saying what it meant",
                path.ident.name
            ),
            |diagnostic| {
                if let Some(replacement) =
                    match_fallback(cx, expr, receiver, path.ident.name.as_str(), arguments)
                {
                    diagnostic.span_suggestion(
                        expr.span,
                        "write both outcomes with the same fallback value",
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help(
                        "`match` both ways, or hand the absence on as an `Option` or a named fault",
                    );
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
