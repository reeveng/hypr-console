#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT001: a failure met is a failure said, whatever the signature
    /// promises.
    ///
    /// A lint cannot read "fallible" off a signature, so this reads it off the
    /// body instead: a function that swallows somebody else's error -- with
    /// `unwrap_or`, `unwrap_or_else`, `unwrap_or_default`, `ok`, `is_ok`,
    /// `is_err` -- is a function that has met a failure and decided not to
    /// mention it, and the caller cannot know there was one.
    ///
    /// This used to let a function that returns `Result` swallow anyway, on the
    /// grounds that it had already said it could fail. What that permitted was
    /// the shape nobody reads twice: a `strip_prefix` that quietly hands back
    /// the whole path, a clock reading that quietly becomes zero, a file that
    /// could not be read quietly becoming an empty one. Saying somewhere that
    /// you can fail is not the same as saying that you did. Name the other way
    /// instead -- a `match` with both arms, and the failing arm's binding
    /// saying what went wrong -- which is what `Err(_outside_the_tree)` does
    /// and what `unwrap_or_default` never can.
    pub EXPLICIT001_FALLIBLE_RESULT,
    Deny,
    "a function that swallows a failure must return `Result<T, E>`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
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

// The ways a `Result` is turned into a plain value without a word said. The
// panicking ones (`unwrap`, `expect`) belong to EXPLICIT004 and are left to it.
const SWALLOWS: &[&str] = &[
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "ok",
    "is_ok",
    "is_err",
];

impl<'tcx> LateLintPass<'tcx> for Explicit001FallibleResult {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        if !SWALLOWS.contains(&path.ident.name.as_str()) {
            return;
        }
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_result(cx, recv_ty) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT001_FALLIBLE_RESULT,
            expr.span,
            format!("`{}` turns a failure into a value with nothing said", path.ident.name),
            None,
            "propagate it with `?`, or `match` both ways and name the failing one",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
