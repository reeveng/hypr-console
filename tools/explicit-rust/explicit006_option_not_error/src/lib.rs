#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

#[path = "../../shared/macro_input.rs"]
mod macro_input;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::is_in_const_context;
use rustc_errors::Applicability;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};

dylint_linting::declare_late_lint! {
    /// EXPLICIT006: `Option` is for "may not exist", not for "went wrong".
    /// `.ok()` on a `Result` is the moment the two are confused: it takes an
    /// error that said what happened and returns a `None` that says nothing.
    /// A caller handed that `None` cannot tell an empty answer from a failure.
    pub EXPLICIT006_OPTION_NOT_ERROR,
    Deny,
    "`Result::ok()` throws away why it failed; `Option` is for absence, not for errors"
}

// `Result`, whatever it is called at the point of use. Asking the type rather
// than the spelling is what makes an alias answer the same as `std`'s own.
fn is_result(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        rustc_middle::ty::Adt(adt, _) => {
            cx.tcx.is_diagnostic_item(rustc_span::sym::Result, adt.did())
        }
        _ => false,
    }
}

fn impossible_variant(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    receiver: &Expr<'_>,
    name: &str,
) -> Option<String> {
    if call.span.from_expansion()
        || receiver.span.from_expansion()
        || !call.span.contains(receiver.span)
        || is_in_const_context(cx)
        || !cx.sess().edition().at_least_rust_2018()
    {
        return None;
    }
    let types = cx.typeck_results();
    let receiver_type = types.expr_ty(receiver);
    let ty::Adt(_, arguments) = receiver_type.kind() else {
        return None;
    };
    let method = types.type_dependent_def_id(call.hir_id)?;
    if !is_result(cx, receiver_type)
        || !types.expr_adjustments(receiver).is_empty()
        || cx
            .tcx
            .type_of(cx.tcx.inherent_impl_of_assoc(method)?)
            .instantiate(cx.tcx, arguments)
            .skip_norm_wip()
            != receiver_type
    {
        return None;
    }
    let (kept, discarded, discarded_type) = match name {
        "ok" => ("Ok", "Err", arguments.type_at(1)),
        _ => ("Err", "Ok", arguments.type_at(0)),
    };
    match discarded_type.kind() {
        ty::Never => {}
        ty::Adt(enum_type, _)
            if enum_type.is_enum()
                && enum_type.variants().is_empty()
                && !enum_type.is_variant_list_non_exhaustive() => {}
        _ => return None,
    }
    if !macro_input::outside_macro_input(cx, call.span) {
        return None;
    }
    let map = cx.sess().source_map();
    let file = map.lookup_source_file(call.span.lo());
    let contents = file.src.as_ref()?;
    let module = cx.tcx.parent_module(call.hir_id).to_local_def_id();
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while contents.contains(&name)
            || cx
                .tcx
                .module_children_local(module)
                .iter()
                .any(|child| child.ident.as_str() == name)
        {
            name.push('_');
        }
        name
    };
    let value = fresh("_result_value");
    let impossible = fresh("_impossible_variant");
    let suffix = map
        .span_to_snippet(call.span.with_lo(receiver.span.hi()))
        .ok()?;
    if suffix.contains("//") || suffix.contains("/*") {
        return None;
    }
    let source = map.span_to_snippet(receiver.span).ok()?;
    let matched = format!(
        "match {source} {{ ::core::result::Result::{kept}({value}) => \
         ::core::option::Option::Some({value}), \
         ::core::result::Result::{discarded}({impossible}) => match {impossible} {{}} }}"
    );
    let grouped = match cx.tcx.parent_hir_node(call.hir_id) {
        Node::LetStmt(local) => local.els.is_some(),
        Node::Expr(parent) => matches!(
            parent.kind,
            ExprKind::Binary(_, left, _)
                | ExprKind::Cast(left, _)
                | ExprKind::MethodCall(_, left, _, _)
                | ExprKind::Field(left, _)
                | ExprKind::Index(left, _, _)
                | ExprKind::Call(left, _)
                if left.hir_id == call.hir_id
        ),
        _ => false,
    };
    Some(if grouped { format!("({matched})") } else { matched })
}

impl<'tcx> LateLintPass<'tcx> for Explicit006OptionNotError {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        let name = path.ident.name.as_str();
        if name != "ok" && name != "err" {
            return;
        }
        // Only when the receiver really is a `Result`. `.ok()` on anything
        // else is someone's own method and none of this rule's business.
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_result(cx, recv_ty) {
            return;
        }
        let (what, help) = if name == "ok" {
            (
                "`Result::ok()` turns an error into `None`",
                "propagate the error with `?`, or `match` it; `Option` should mean the value may not exist",
            )
        } else {
            (
                "`Result::err()` throws away the value and keeps only the failure",
                "`match` the `Result` so both arms are written out",
            )
        };
        span_lint_and_then(cx, EXPLICIT006_OPTION_NOT_ERROR, expr.span, what, |diag| {
            if let Some(replacement) = impossible_variant(cx, expr, receiver, name) {
                diag.span_suggestion(
                    expr.span,
                    "name the variant that can occur",
                    replacement,
                    Applicability::MachineApplicable,
                );
            } else {
                diag.help(help);
            }
        });
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
