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
    /// EXPLICIT033: an absence answered by a value nobody wrote down.
    ///
    /// EXPLICIT001 is this rule over a `Result`, where what is swallowed is
    /// somebody else's error. This is the same three methods over an `Option`,
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
    /// be true. Where the absence is somebody else's business, hand it on:
    /// return the `Option`, or `ok_or` a fault that says what was missing.
    /// Where it really does have a chosen answer, the answer is written at the
    /// call site in full, which is the whole of what this rule asks for.
    ///
    /// `Option::unwrap_or` over a default that is itself the subject -- a
    /// palette's fallback colour, a layout's minimum -- is the shape that wants
    /// a named constant rather than a literal, and then the `match` reads as
    /// the sentence it is.
    pub EXPLICIT033_NO_UNNAMED_ABSENCE,
    Deny,
    "an `Option`'s `None` must be answered in a `match` that names it, not by `unwrap_or`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// `Option`, whatever it is called at the point of use, for the reason
// EXPLICIT001 asks the type rather than the spelling: an alias answers the same
// as `std`'s own.
fn is_option(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        rustc_middle::ty::Adt(adt, _) => {
            cx.tcx.is_diagnostic_item(rustc_span::sym::Option, adt.did())
        }
        _ => false,
    }
}

// The three that produce a value. `ok_or`, `ok_or_else` and `map_or` are not
// here: the first two turn the absence into a fault, which is naming it, and
// `map_or` is asked about in EXPLICIT025 where it is a sentinel or not on the
// strength of what it hands back.
const ANSWERS: &[&str] = &["unwrap_or", "unwrap_or_else", "unwrap_or_default"];

impl<'tcx> LateLintPass<'tcx> for Explicit033NoUnnamedAbsence {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        if !ANSWERS.contains(&path.ident.name.as_str()) {
            return;
        }
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_option(cx, recv_ty) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT033_NO_UNNAMED_ABSENCE,
            expr.span,
            format!("`{}` answers a `None` without saying what it meant", path.ident.name),
            None,
            "`match` both ways, or hand the absence on as an `Option` or a named fault",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
