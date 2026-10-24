//! EXPLICIT063: a fault met is acted on, or it was not met at all.
//!
//! EXPLICIT005 made a fallible value handled or propagated; EXPLICIT038 gave
//! every fault a type a caller can decide about. This rule is the sentence
//! after both of them: "handled" means something was *done*. A `match` whose
//! `Err` arm is empty -- `Err(_) => {}` -- meets the fault and does nothing:
//! no retry, no degraded answer, no safe mode, not even a propagation, just
//! the shape of handling with none of it inside. The empty arm reads as
//! "already thought about", which is exactly what makes it the arm nobody
//! looks at when the fault happens.
//!
//! Every error variant this tree can name maps to one of three actions, and
//! the arm is the place the mapping is written: *retry* it, where the fault
//! is a source that may come back; *degrade* to the answer that can be given
//! without what failed; *hand it upward* with `?`, which is a decision too --
//! the caller is told and can decide for themselves. Logging alone is not on
//! the list: a fault said only to the journal is EXPLICIT041's argument about
//! printing, said about recovery -- it is the recovery nobody performs.
//!
//! What this lint can see is the arm that does nothing at all, which is the
//! arm the rule can decide without guessing. Only an arm that can hold the
//! fault is asked: an empty `Ok(()) => {}` is the answer arriving with nothing
//! left to do, and the arm beside it is where the fault is met. The arm that logs and continues
//! is the harder fault and it wants a reading rather than a lint; where that
//! is the answer, the arm says what it degraded to and the allow is not
//! needed. It arrives `Warn`, printing the empty arms over the tree's
//! `Result`s.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::{is_lang_item_or_ctor, is_unit_expr};
use rustc_hir::def::Res;
use rustc_hir::{Arm, Expr, ExprKind, LangItem, Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT063: a `match` arm over a fault that answers with nothing has
    /// met the fault and done nothing. Retry, degrade, or hand the fault
    /// upward -- the arm is where the recovery is said.
    pub EXPLICIT063_FAULT_HAS_RECOVERY,
    Warn,
    "an arm over a fault that does nothing; recovery is an action, not an empty arm"
}

// An arm that can be reached by an `Err`. The one that cannot is an `Ok`, or
// an or-pattern of nothing else; a binding or a `_` holds the fault as well.
fn meets_the_fault(cx: &LateContext<'_>, pat: &Pat<'_>) -> bool {
    match pat.kind {
        PatKind::TupleStruct(ref qpath, _, _) => match cx.qpath_res(qpath, pat.hir_id) {
            Res::Def(_, made) => !is_lang_item_or_ctor(cx, made, LangItem::ResultOk),
            _ => true,
        },
        PatKind::Or(halves) => halves.iter().any(|half| meets_the_fault(cx, half)),
        PatKind::Binding(_, _, _, Some(inner)) => meets_the_fault(cx, inner),
        _ => true,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit063FaultHasRecovery {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Match(asked, arms, _) = expr.kind else {
            return;
        };

        let falls = match cx.typeck_results().expr_ty(asked).kind() {
            rustc_middle::ty::Adt(adt, _) => cx
                .tcx
                .is_diagnostic_item(rustc_span::sym::Result, adt.did()),
            _ => false,
        };

        if !falls {
            return;
        }

        for arm in arms {
            let Arm { pat, body, .. } = arm;

            if !is_unit_expr(body) {
                continue;
            }

            if !meets_the_fault(cx, pat) {
                continue;
            }

            span_lint_and_help(
                cx,
                EXPLICIT063_FAULT_HAS_RECOVERY,
                arm.span,
                "this arm meets a fault and does nothing",
                None,
                "the arm is where the recovery is said: retry it, degrade to the answer that can \
                 be given, or hand the fault upward with `?` so the caller decides. An arm that \
                 only logs has not recovered either -- it has said the fault out loud and \
                 carried on as though it were not there",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
