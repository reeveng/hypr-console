//! EXPLICIT060: a discard says why it was a decision, or it was not one.
//!
//! EXPLICIT009 took the bare `f();` on a `#[must_use]` value and made every
//! discard write itself as `let _ = …`, which is the spelling that says
//! somebody meant it. This rule is the sentence that spelling was waiting
//! for: a `let _ =` over a `Result` -- or over anything else the type has
//! marked as must-not-drop -- is a fault decided away in four characters, and
//! the reader cannot tell a fault that was somebody else's to answer from one
//! nobody looked at.
//!
//! What the rule asks for is the same door EXPLICIT018 opens for every
//! `allow`: `let _ = …` is fine, and the site says which harm it decided
//! against and why that harm cannot happen here. The match with both arms
//! named is the other spelling -- a `Result` met by a `match` is a value the
//! rest of the rules can ask about, and a fault handed upward is a fault the
//! caller can decide about, which is what `?` is for.
//!
//! It arrives `Warn`: the tree has `let _ =` over fallible values in the
//! tests, mostly where a teardown cannot answer and the fault is genuinely
//! nobody's to hold, and each of those wants its sentence.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::ty::is_must_use_ty;
use rustc_hir::{Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT060: `let _ = …` over a `Result` or a `#[must_use]` value is a
    /// discard with nothing said about it. Say why the fault was yours to
    /// drop, with an allow whose reason names the harm decided against.
    pub EXPLICIT060_NO_REASONLESS_DISCARD,
    Warn,
    "a discarded fault or `#[must_use]` value with no reason at the discard"
}

impl<'tcx> LateLintPass<'tcx> for Explicit060NoReasonlessDiscard {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        let StmtKind::Let(local) = stmt.kind else {
            return;
        };

        let rustc_hir::LetStmt { pat, init: Some(thrown), .. } = local else {
            return;
        };

        // Only the plain `_`: a name (`let _gone = …`) is a binding somebody
        // can still read, and a tuple has arms of its own.
        if !matches!(pat.kind, rustc_hir::PatKind::Wild) {
            return;
        }

        let ty = cx.typeck_results().expr_ty(thrown);

        let falls = match ty.kind() {
            rustc_middle::ty::Adt(adt, _) => cx
                .tcx
                .is_diagnostic_item(rustc_span::sym::Result, adt.did()),
            _ => false,
        };

        if !falls && !is_must_use_ty(cx, ty) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT060_NO_REASONLESS_DISCARD,
            stmt.span,
            "a fault or a value the type says not to drop, discarded with nothing said",
            None,
            "meet it with a `match` naming both arms, hand it upward with `?`, or keep the \
             `let _ =` and allow this rule here with the reason saying which harm was decided \
             against and why it cannot happen here",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
