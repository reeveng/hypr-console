//! EXPLICIT045: a conversion names the type it becomes.
//!
//! EXPLICIT010 denies `x.into()` where both ends are numbers, because a
//! widening that reads as "the same number" is a different range under the
//! same spelling. The reason it gives was never about arithmetic. It is that a
//! conversion naming neither the type it came from nor the type it becomes
//! says nothing at the call site, and the reader has to ask the compiler what
//! happened -- which is the same complaint EXPLICIT011 makes about `as` and
//! EXPLICIT033 makes about a default no one wrote down.
//!
//! `Target::from(x)` is the same conversion with the destination on the line,
//! and it is the one that stops compiling when the target type changes under
//! it rather than quietly converting to something else.
//!
//! Kast took it further than this rule can: there is no implicit coercion at
//! all, and a cast is a value someone wrote -- `impl (123 :: Int32) as String`
//! -- so the pair is declared, both halves are named, and `as` retrieves that
//! declaration instead of inventing one.
//!
//! What is not touched is the `From` a `?` runs. It is the same trait and it
//! names nothing at the site either, but the fault it becomes is on the
//! function's own signature a few lines up, which is exactly what `.into()`
//! does not have. EXPLICIT017 already makes that `?` stand alone where it can
//! be read.
//!
//! Numbers are left to EXPLICIT010, which says more about them than this can:
//! it can name the two widths.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT045: `x.into()` names neither end of the conversion, so what it
    /// becomes is whatever the surrounding code happened to want. Write
    /// `Target::from(x)` and put the destination on the line.
    pub EXPLICIT045_NO_UNNAMED_CONVERSION,
    Deny,
    "`into()` names neither type; write the destination as `Target::from(x)`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn is_numeric(ty: Ty<'_>) -> bool {
    matches!(ty.kind(), rustc_middle::ty::Int(_) | rustc_middle::ty::Uint(_) | rustc_middle::ty::Float(_))
}

impl<'tcx> LateLintPass<'tcx> for Explicit045NoUnnamedConversion {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };

        if path.ident.name.as_str() != "into" {
            return;
        }

        let from = cx.typeck_results().expr_ty_adjusted(receiver);
        let to = cx.typeck_results().expr_ty(expr);

        if from == to {
            return;
        }

        if is_numeric(from) && is_numeric(to) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT045_NO_UNNAMED_CONVERSION,
            expr.span,
            format!("`into()` turns `{from}` into `{to}` and the line says neither"),
            None,
            "write the destination out -- `Target::from(x)` -- so the reader sees what this becomes \
             without asking the compiler, and so the call stops compiling when the type around it \
             changes rather than converting to the new one behind the line",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
