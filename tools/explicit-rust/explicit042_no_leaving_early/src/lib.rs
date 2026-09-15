//! EXPLICIT042: a program ends by returning from `main`, not by leaving from
//! wherever it happened to be standing.
//!
//! `std::process::exit` does not unwind and does not drop. Everything the
//! process was holding is still held at the moment the kernel takes it away:
//! a file written beside the live one and not yet renamed over it, a claim on
//! an input device, a lock in `/run`, and -- the one that matters here -- a
//! child started `Alongside`.
//!
//! `console-program-lifetime` is the whole argument for why that last one is
//! not a small thing. `Alongside` is two mechanisms and needs both: a death
//! signal, so a parent that is killed takes its child with it, and a `Drop`,
//! so a parent that returns does too. An `exit` is neither. It is not a kill,
//! so the death signal never fires; it is not a return, so the drop never
//! runs. The child outlives the program that started it, which is the exact
//! outcome the type was written to make impossible, and it is reached by the
//! one line nobody reads because it is the last line of an error arm.
//!
//! What every site here is doing is saying a fault and failing, which is what
//! `ExitCode` is: `main` returns `ExitCode::FAILURE` and the run ends the
//! same way, with the stack unwound on the way out. Where the fault is found
//! deeper than `main`, it comes back as a fault and `main` decides -- which is
//! 001's rule about a failure met being a failure said, arriving at the last
//! place in the program that can still say it.
//!
//! `abort` is denied for the same reason and a louder one: it does not even
//! flush. It is here so that nobody answers this rule by reaching for the
//! other function on the same page.
//!
//! What is not touched is a process this one did not start and is not: `exit`
//! read off somebody else's `ExitStatus` is a number that came back, not a
//! way of leaving.
//!
//! It arrived denied, with the tree breaking it in a handful of binaries and
//! every one of them the same shape: an error arm that says a sentence and
//! goes. Most were a `main` with nothing to return, and became one that returns
//! an `ExitCode`. The two that were deeper became a fault with a name and a
//! variant per way of failing, with `main` reading the run's number off the
//! variant -- which is EXPLICIT038 arriving at the last place in a program that
//! can still say anything. Neither is a larger program than what was there.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT042: `std::process::exit` leaves without unwinding, so nothing
    /// the program was holding is dropped -- including a child started
    /// `Alongside`, whose whole promise is the drop that never runs. Return
    /// from `main` with an `ExitCode` instead.
    pub EXPLICIT042_NO_LEAVING_EARLY,
    Deny,
    "leaving the process without unwinding; nothing held is dropped"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Read off the resolved path, so a `use std::process::exit` is the same
// leaving. `abort` is the same fault without the flush.
fn leaves_without_unwinding(path: &str) -> Option<&'static str> {
    match path {
        "std::process::exit" => {
            Some("the stack is not unwound, so nothing this program is holding is dropped")
        }
        "std::process::abort" => {
            Some("the stack is not unwound and the buffers are not flushed either")
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit042NoLeavingEarly {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };

        let ExprKind::Path(QPath::Resolved(_, path)) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = path.res else {
            return;
        };

        let named = cx.tcx.def_path_str(id);

        let Some(harm) = leaves_without_unwinding(named.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT042_NO_LEAVING_EARLY,
            expr.span,
            format!("`{named}` ends the process here: {harm}"),
            None,
            "say the fault and hand it back: `main` returns `std::process::ExitCode`, and \
             `ExitCode::FAILURE` ends the run the same way with the stack unwound on the way out. A \
             child started `console_program_lifetime::Lifetime::Alongside` is killed by that drop and by \
             nothing else, so an `exit` under one is a program left running with nobody to stop it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
