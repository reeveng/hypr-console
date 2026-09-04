#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::macros::{is_panic, root_macro_call_first_node};
use clippy_utils::sym;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT004: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`,
    /// and `unreachable!` are forbidden. They turn a typed error path into an
    /// implicit one — exactly the behaviour the Explicit-Rust suite forbids.
    ///
    /// The two halves are found two different ways. `unwrap` and `expect` are
    /// method calls and are matched by name. The four macros are matched on
    /// the macro backtrace rather than on what they lower to, because what
    /// they lower to is not a promise the compiler makes.
    pub EXPLICIT004_NO_PANIC,
    Deny,
    "`unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` are forbidden"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit004NoPanic {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        // `unwrap()` and `expect(...)` are method calls.
        if let ExprKind::MethodCall(path, _, _, _) = expr.kind {
            if matches!(path.ident.name, rustc_span::sym::unwrap | rustc_span::sym::expect) {
                let method = path.ident.name;
                span_lint_and_help(
                    cx,
                    EXPLICIT004_NO_PANIC,
                    expr.span,
                    format!("`{}` is forbidden: it turns a typed error path into an implicit panic", method),
                    None,
                    "use `?`, `match`, or return a `Result<T, E>` so the failure is visible at the call site",
                );
                return;
            }
        }
        // `panic!`, `todo!`, `unimplemented!` and `unreachable!` are macros,
        // and what they lower to is the compiler's business and has changed
        // under this lint before: matching the callee symbol quietly stopped
        // catching three of the four. So they are read off the macro backtrace
        // instead, which is the name somebody actually wrote.
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
                    format!("`{macro_name}` is forbidden: it turns a typed error path into an implicit panic"),
                    None,
                    "use `?`, `match`, or return a `Result<T, E>` so the failure is visible at the call site",
                );
                return;
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}