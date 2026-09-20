//! EXPLICIT047: a closure takes what it writes, rather than holding the
//! permission to write it.
//!
//! EXPLICIT044 says a function decides from what it was handed. A closure that
//! captures a binding mutably is the case that rule does not reach: the value
//! was handed to the closure at the moment it was written, so what comes out is
//! an `FnMut` whose type says `()` and whose call writes to something the
//! caller cannot see from the signature. The permission and the thing are one
//! value, carried together, and every caller from then on has both whether or
//! not it wanted either.
//!
//! Kast separates them. A closure there captures the pointer and not the
//! access, and the access is part of the function's type: `inc` and `dec` over
//! the same `x` are both `() -> () with mutable_access[x]`, and the permission
//! is required at the call rather than held since the capture. The idea is
//! GhostCell's, made ergonomic by being a language feature rather than a
//! library, which is also why their example compiles and the Rust one does not.
//!
//! The answer at a site here is usually the same shape as the arithmetic
//! modules in `console-panel`: the closure takes what it writes as an argument
//! and the caller passes it, so the writing is at the call and a check can hand
//! it something of its own.
//!
//! Denied, and what let it out was one argument rather than a list of fixes.
//! Some of what it found was an iterator word standing in for a loop -- a
//! `try_for_each` over something that writes, a `filter` that inserts as it
//! goes, a `map` that asks a machine -- and those became the `for` they always
//! were, where the writing is a statement the reader can see. The rest were the
//! functions that take a closure and hand it nothing: a wait, a stretch of an
//! apply, the sentence a failed check prints, the line a run draws quietly.
//! Each has a second spelling now, `_handed`, that takes the thing the question
//! is asked with and hands it in at every ask -- EXPLICIT044's sentence about a
//! function, said about a closure -- which is what `Device::until` had been
//! doing all along.
//!
//! It still catches the borrow and not the `move` closure that owns what it
//! writes, which is the larger half and wants the page #108 in the backlog is
//! asking for.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::UpvarCapture;

dylint_linting::declare_late_lint! {
    /// EXPLICIT047: a closure that captures a binding mutably carries the
    /// permission to write it, so the call site cannot see that calling writes
    /// anything. Take what is written as an argument instead.
    pub EXPLICIT047_NO_CAPTURED_PERMISSION,
    Deny,
    "a closure holding the permission to write something it captured"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit047NoCapturedPermission {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Closure(closure) = expr.kind else {
            return;
        };

        let written: Vec<String> = cx
            .typeck_results()
            .closure_min_captures_flattened(closure.def_id)
            .filter(|capture| {
                matches!(
                    capture.info.capture_kind,
                    UpvarCapture::ByRef(rustc_middle::ty::BorrowKind::Mutable)
                )
            })
            .map(|capture| capture.to_string(cx.tcx))
            .collect();

        let Some(first) = written.first() else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT047_NO_CAPTURED_PERMISSION,
            expr.span,
            format!("this closure captured `{first}` with the permission to write it"),
            None,
            "hand it in at the call instead: a closure that takes what it writes as an argument \
             says so in its own type, the caller can see that calling it writes something, and a \
             check can pass one of its own rather than arranging the real thing. EXPLICIT044 is \
             the same rule about a function, which a closure is",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
