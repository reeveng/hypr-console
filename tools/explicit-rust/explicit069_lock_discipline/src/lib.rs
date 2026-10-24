//! EXPLICIT069: a lock is a promise, and promises have an order.
//!
//! This rule holds the two halves of concurrency this tree can be asked
//! statically. The first is who may cross a thread boundary: an
//! `unsafe impl Send` or `unsafe impl Sync` is a claim that the compiler was
//! told to stop checking, and it needs its reason at the site -- what the
//! type holds that makes the claim true -- the same way EXPLICIT012 asks an
//! `unsafe` block for its invariant and EXPLICIT018 asks every `allow`.
//! `static mut` is gone by EXPLICIT068; this is the other spelling of "trust
//! me" that threading can write.
//!
//! The second is what a claim of ordering is worth: an atomic used at
//! `Relaxed`, `Acquire`, `Release` or `AcqRel` is a memory-ordering decision
//! that is *correct only in combination with other atomics' decisions*, and
//! the combination is invisible at the call site -- it lives in whatever
//! happens to be on the other end of the fence, in another crate. `SeqCst`
//! is the one order that needs nothing from its other end. So a weaker
//! ordering here is a performance decision whose cost is paid by whoever
//! edits the other side, and the rule asks that it be said: `SeqCst` by
//! default, and a weaker order only with an allow whose reason names the
//! half that is being relied on elsewhere.
//!
//! The half this rule cannot read -- a lock held across a blocking call, a
//! second lock taken under a first with no order declared -- is a runtime
//! shape rather than a syntactic one, and it belongs to review and to
//! `console-actor`: the actor's own thread is how this tree avoids holding
//! locks at all, and a lock with a wait inside it is nearly always an actor
//! that wanted writing.
//!
//! It arrives `Warn`: the tree's atomics are few and mostly already `SeqCst`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, Item, ItemKind, QPath};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT069: an `unsafe impl Send` or `Sync` says what makes it true,
    /// and an atomic weaker than `SeqCst` says the half of the ordering it
    /// relies on elsewhere.
    pub EXPLICIT069_LOCK_DISCIPLINE,
    Warn,
    "a threading promise written without its reason; the site says what makes it true"
}

fn crosses_a_thread(path: &str) -> Option<&'static str> {
    match path {
        "core::marker::Send" | "std::marker::Send" => {
            Some("this claims the value crosses a thread boundary safely")
        }
        "core::marker::Sync" | "std::marker::Sync" => {
            Some("this claims the value may be shared across threads safely")
        }
        _ => None,
    }
}

fn orders_less_than_said(path: &str) -> Option<&'static str> {
    match path {
        "core::sync::atomic::Ordering::Relaxed" | "std::sync::atomic::Ordering::Relaxed" => {
            Some("`Relaxed` orders nothing beyond the atomic itself")
        }
        "core::sync::atomic::Ordering::Acquire" | "std::sync::atomic::Ordering::Acquire" => {
            Some("`Acquire` is only correct with a `Release` it can rely on elsewhere")
        }
        "core::sync::atomic::Ordering::Release" | "std::sync::atomic::Ordering::Release" => {
            Some("`Release` is only correct with an `Acquire` it can rely on elsewhere")
        }
        "core::sync::atomic::Ordering::AcqRel" | "std::sync::atomic::Ordering::AcqRel" => {
            Some("`AcqRel` is only correct with the other half written elsewhere")
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit069LockDiscipline {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Impl(under) = item.kind else {
            return;
        };

        let Some(header) = under.of_trait else {
            return;
        };

        if header.safety != rustc_hir::Safety::Unsafe {
            return;
        }

        let path = header.trait_ref.path;

        let Res::Def(DefKind::Trait, id) = path.res else {
            return;
        };

        let Some(harm) = crosses_a_thread(cx.tcx.def_path_str(id).as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT069_LOCK_DISCIPLINE,
            item.span,
            format!("an `unsafe impl` written here, and {harm}"),
            None,
            "say what the type holds that makes it true, as the `reason` on an allow for this \
             rule here -- the way a `SAFETY:` says what an `unsafe` block stands on",
        );
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Path(QPath::Resolved(_, path)) = expr.kind else {
            return;
        };

        let Res::Def(_, id) = path.res else {
            return;
        };

        let Some(harm) = orders_less_than_said(cx.tcx.def_path_str(id).as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT069_LOCK_DISCIPLINE,
            expr.span,
            harm,
            None,
            "write `SeqCst`, which needs nothing from its other end, or allow this rule here \
             with the reason naming the half of the ordering written elsewhere",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
