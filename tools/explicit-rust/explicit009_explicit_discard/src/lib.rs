#![feature(rustc_private)]
#![warn(unused_extern_crates)]

#[path = "../../shared/infallible_discard.rs"]
mod infallible_discard;

extern crate rustc_hir;
extern crate rustc_errors;
extern crate rustc_middle;

use clippy_utils::{diagnostics::span_lint_and_then, fn_def_id, sym};
use clippy_utils::ty::is_must_use_ty;
use rustc_errors::Applicability;
use rustc_hir::{Stmt, StmtKind, find_attr};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;

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

fn requires_handling(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> bool {
    ty.ty_adt_def().is_some_and(|adt| {
        [sym::Result, sym::MutexGuard, sym::RwLockReadGuard, sym::RwLockWriteGuard]
            .iter()
            .any(|&name| cx.tcx.is_diagnostic_item(name, adt.did()))
    })
}

impl<'tcx> LateLintPass<'tcx> for Explicit009ExplicitDiscard {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        // `let _ = …` is the spelling this rule asks for, and it is a
        // `StmtKind::Let`, so it never reaches here. Only a bare expression
        // statement does.
        let StmtKind::Semi(expr) = stmt.kind else {
            return;
        };
        let ty = cx.typeck_results().expr_ty(expr);
        let must_use_call = fn_def_id(cx, expr)
            .is_some_and(|function| find_attr!(cx.tcx, function, MustUse { .. }));
        let must_use_type = is_must_use_ty(cx, ty);
        if !must_use_call && !must_use_type {
            return;
        }
        span_lint_and_then(
            cx,
            EXPLICIT009_EXPLICIT_DISCARD,
            stmt.span,
            "a `#[must_use]` value is being discarded silently",
            |diagnostic| {
                // A bare path can move and drop a local, while `let _` leaves
                // it in place. Calls produce a new temporary to discard.
                // EXPLICIT060 still requires handling a fault or must-use type.
                if !stmt.span.from_expansion() && !expr.span.from_expansion()
                    && stmt.span.lo() == expr.span.lo()
                    && infallible_discard::is_call(expr)
                    && let Some(pattern) = infallible_discard::discard_pattern(cx, ty)
                {
                    diagnostic.span_suggestion(expr.span.shrink_to_lo(), "acknowledge the only possible result",
                        format!("let {pattern} = "), Applicability::MachineApplicable);
                } else if infallible_discard::is_call(expr)
                    && !stmt.span.from_expansion()
                    && !expr.span.from_expansion()
                    && stmt.span.lo() == expr.span.lo()
                    && !must_use_type
                    && !requires_handling(cx, ty)
                {
                    diagnostic.span_suggestion(
                        stmt.span.shrink_to_lo(),
                        "make the discarded return value explicit",
                        "let _ = ",
                        Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("handle the value or explicitly discard it while preserving its drop behavior");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
