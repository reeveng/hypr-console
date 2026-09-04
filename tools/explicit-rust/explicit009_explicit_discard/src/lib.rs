#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::ty::is_must_use_ty;
use rustc_hir::{Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT009: a `#[must_use]` value thrown away must be thrown away in
    /// writing. `f();` and `let _ = f();` do the same thing to the value and
    /// different things to the reader: the second says the discard was a
    /// decision. rustc's own `unused_must_use` is a warning; here it is a
    /// denial with a required spelling.
    pub EXPLICIT009_EXPLICIT_DISCARD,
    Deny,
    "a discarded `#[must_use]` value must be written `let _ = …`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit009ExplicitDiscard {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        // `let _ = …` is the spelling this rule asks for, and it is a
        // `StmtKind::Let`, so it never reaches here. Only a bare expression
        // statement does.
        let StmtKind::Semi(expr) = stmt.kind else {
            return;
        };
        let ty = cx.typeck_results().expr_ty(expr);
        if !is_must_use_ty(cx, ty) {
            return;
        }
        span_lint_and_help(
            cx,
            EXPLICIT009_EXPLICIT_DISCARD,
            stmt.span,
            "a `#[must_use]` value is being discarded silently",
            None,
            "write `let _ = …;` so the discard is a decision somebody made on purpose",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
