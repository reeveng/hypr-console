#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

#[path = "../../shared/local_type_path.rs"]
mod local_type_path;
mod slice_array;
mod test_assertions;
mod test_reads;
mod test_results;

use clippy_utils::diagnostics::{span_lint_and_help, span_lint_and_then};
use clippy_utils::is_in_const_context;
use clippy_utils::macros::{is_panic, root_macro_call_first_node};
use clippy_utils::sym;
use rustc_hir::{Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;

dylint_linting::impl_late_lint! {
    /// EXPLICIT004: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`,
    /// and `unreachable!` are forbidden. They turn a typed error path into an
    /// implicit one — exactly the behavior the Explicit-Rust suite forbids.
    ///
    /// With `explicit004_fixed_array_bounds = true`, a failed conversion
    /// from a checked slice to a borrowed fixed-size array follows the same
    /// error path as its original bounds lookup.
    ///
    /// The two halves are found two different ways. `unwrap` and `expect` are
    /// method calls and are matched by name. The four macros are matched on
    /// the macro backtrace rather than on what they lower to, because what
    /// they lower to is not a promise the compiler makes.
    pub EXPLICIT004_NO_PANIC,
    Deny,
    "`unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` are forbidden",
    Explicit004NoPanic {
        test_results: dylint_linting::config_or_default("explicit004_test_results"),
        fixed_array_bounds: dylint_linting::config_or_default("explicit004_fixed_array_bounds"),
        suggestions: Default::default(),
        referenced: None,
    }
}

type Suggestion = (&'static str, Vec<(rustc_span::Span, String)>);

pub struct Explicit004NoPanic {
    test_results: bool,
    fixed_array_bounds: bool,
    referenced: Option<std::collections::HashSet<rustc_hir::def_id::LocalDefId>>,
    suggestions: std::collections::HashMap<rustc_hir::HirId, Suggestion>,
}

fn infallible_result(cx: &LateContext<'_>, call: &Expr<'_>, receiver: &Expr<'_>) -> Option<String> {
    if call.span.from_expansion()
        || receiver.span.from_expansion()
        || is_in_const_context(cx)
        || !cx.sess().edition().at_least_rust_2018()
    {
        return None;
    }
    let types = cx.typeck_results();
    let receiver_ty = types.expr_ty(receiver);
    let ty::Adt(result, arguments) = receiver_ty.kind() else {
        return None;
    };
    if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
        || !types.expr_adjustments(receiver).is_empty()
    {
        return None;
    }
    let method = types.type_dependent_def_id(call.hir_id)?;
    if cx
        .tcx
        .type_of(cx.tcx.inherent_impl_of_assoc(method)?)
        .instantiate(cx.tcx, arguments)
        .skip_norm_wip()
        != receiver_ty
    {
        return None;
    }
    let ty::Adt(error, _) = arguments.type_at(1).kind() else {
        return None;
    };
    if !error.is_enum() || !error.variants().is_empty() || error.is_variant_list_non_exhaustive() {
        return None;
    }
    let map = cx.sess().source_map();
    let source = map.span_to_snippet(call.span).ok()?;
    if source.contains("//") || source.contains("/*") {
        return None;
    }
    let receiver = map.span_to_snippet(receiver.span).ok()?;
    let matched = format!(
        "match {receiver} {{ ::core::result::Result::Ok(_explicit_value) => _explicit_value, \
         ::core::result::Result::Err(_explicit_error) => match _explicit_error {{}} }}"
    );
    Some(match cx.tcx.parent_hir_node(call.hir_id) {
        Node::LetStmt(_) | Node::Block(_) | Node::Arm(_) => matched,
        _ => format!("({matched})"),
    })
}

impl<'tcx> LateLintPass<'tcx> for Explicit004NoPanic {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx rustc_hir::FnDecl<'tcx>,
        body: &'tcx rustc_hir::Body<'tcx>,
        _span: rustc_span::Span,
        id: rustc_hir::def_id::LocalDefId,
    ) {
        if self.test_results
            && let Some((first, suggestion)) = test_results::propagate(cx, id, decl, body)
                .map(|(first, edits)| (first, ("report failed test setup through its concrete error type", edits)))
                .or_else(|| test_reads::edits(cx, id, decl, body)
                    .map(|(first, edits)| (first, ("report failed reads where they are evaluated", edits))))
            && !self
                .referenced
                .get_or_insert_with(|| test_results::referenced_functions(cx))
                .contains(&id)
        {
            self.suggestions.insert(first, suggestion);
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if self.test_results
            && cx.sess().opts.test
            && expr.span.from_expansion()
            && let Some((owner, site, edits)) = test_assertions::edits(cx, expr)
            && !self
                .referenced
                .get_or_insert_with(|| test_results::referenced_functions(cx))
                .contains(&owner)
        {
            self.suggestions.insert(
                site,
                ("compare the returned success variant directly", edits),
            );
        }
        // `unwrap()` and `expect(...)` are method calls.
        if let ExprKind::MethodCall(path, receiver, args, _) = expr.kind {
            if matches!(
                path.ident.name,
                rustc_span::sym::unwrap | rustc_span::sym::expect
            ) {
                let method = path.ident.name;
                span_lint_and_then(
                    cx,
                    EXPLICIT004_NO_PANIC,
                    expr.span,
                    format!(
                        "`{}` is forbidden: it turns a typed error path into an implicit panic",
                        method
                    ),
                    |diagnostic| {
                        // Only remove an expect message with no evaluation effects.
                        let no_effects = match (method, args) {
                            (sym::unwrap, []) => true,
                            (sym::expect, [message]) => {
                                matches!(message.kind, ExprKind::Lit(_))
                                    && cx.typeck_results().expr_ty(message).peel_refs().is_str()
                            }
                            _ => false,
                        };
                        if let Some((message, edits)) = self.suggestions.remove(&expr.hir_id) {
                            diagnostic.multipart_suggestion(
                                message,
                                edits,
                                rustc_errors::Applicability::MachineApplicable,
                            );
                        } else if self.fixed_array_bounds && no_effects
                            && let Some(replacement) = slice_array::replacement(cx, expr, receiver)
                        {
                            diagnostic.span_suggestion(
                                expr.span,
                                "report a wrong array length through the checked slice's bounds error",
                                replacement,
                                rustc_errors::Applicability::MachineApplicable,
                            );
                        } else if no_effects
                            && let Some(replacement) = infallible_result(cx, expr, receiver)
                        {
                            diagnostic.span_suggestion(
                                expr.span,
                                "extract the success value by matching the uninhabited error",
                                replacement,
                                rustc_errors::Applicability::MachineApplicable,
                            );
                        } else {
                            diagnostic.help("use `?`, `match`, or return a `Result<T, E>` so the failure is visible at the call site");
                        }
                    },
                );
                return;
            }
        }
        // `panic!`, `todo!`, `unimplemented!` and `unreachable!` are macros,
        // and what they lower to is the compiler's business and has changed
        // under this lint before: matching the callee symbol quietly stopped
        // catching three of the four. So they are read off the macro backtrace
        // instead, which is the name someone actually wrote.
        if let Some(call) = root_macro_call_first_node(cx, expr) {
            let named = cx.tcx.get_diagnostic_name(call.def_id);
            let macro_name = if is_panic(cx, call.def_id) {
                Some("panic!")
            } else if named == Some(sym::todo_macro) {
                Some("todo!")
            } else if named == Some(sym::unimplemented_macro) {
                Some("unimplemented!")
            } else if named == Some(rustc_span::sym::unreachable_macro) {
                Some("unreachable!")
            } else {
                None
            };
            if let Some(macro_name) = macro_name {
                span_lint_and_help(
                    cx,
                    EXPLICIT004_NO_PANIC,
                    call.span,
                    format!(
                        "`{macro_name}` is forbidden: it turns a typed error path into an implicit panic"
                    ),
                    None,
                    "use `?`, `match`, or return a `Result<T, E>` so the failure is visible at the call site",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn test_results() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_tests")
        .dylint_toml("explicit004_test_results = true")
        .run();
}

#[test]
fn fixed_array_bounds() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_bounds")
        .dylint_toml("explicit004_fixed_array_bounds = true")
        .run();
}
