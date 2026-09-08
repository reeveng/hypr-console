//! EXPLICIT023: `let … else` is forbidden, for the reason EXPLICIT019 is.
//!
//! 019 exempted it, and the exemption was wrong. What it said was that both of
//! a `let … else`'s outcomes are already written and one of them has to leave,
//! so there is no path decided by omission. The first half of that is not true.
//! One outcome is written as a pattern and the other is written as the word
//! `else`, which is not a name for anything -- `let Some(held) = asked else`
//! never says `None`, exactly as the `if let` 019 forbids never says it. The
//! case that leaves is the case nobody had to think about, which is the whole
//! of what this suite is against.
//!
//! And it is the second spelling of a decision the workspace already has one
//! spelling for. Two forms that mean the same thing are two shapes a reader has
//! to hold, and the one that is not a `match` is the one that does not compose:
//! it cannot bind more than the pattern binds, it cannot have a third case, and
//! the day it needs one it is rewritten into the `match` it should have been.
//!
//! What replaces it is a `match` in the initializer, which is the same
//! statement one line longer:
//!
//!     let said = match std::fs::read_to_string(&at) {
//!         Ok(said) => said,
//!         Err(fault) => return Err(fault.to_string()),
//!     };
//!
//! That is not the nesting a guard clause is written to avoid. A `match` on the
//! right of a `let` is an expression, the binding stays where it was, and
//! everything after it keeps its indentation -- EXPLICIT013 leaves it alone for
//! the same reason. What changes is that `Err` is on the screen with a name on
//! it, where EXPLICIT016 can ask that the variants be named too.
//!
//! An irrefutable `let` is not this and never arrives here. `let Ok(at) =
//! at(&home);` has no `else` because `Result<T, Never>` has no `Err` to meet;
//! that is EXPLICIT002 being kept, not a decision being hidden.
//!
//! It was registered `Warn` when it was written, because the tree broke it
//! everywhere that day -- `let … else` was the guard clause here. It is `Deny`
//! now: the last call site is gone, and by the ratchet's one law the level
//! does not go back.

#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::{LocalKind, Stmt, StmtKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};

dylint_linting::declare_early_lint! {
    /// EXPLICIT023: `let … else` is forbidden. Its `else` is not a name for
    /// the case that did not bind, and a decision this workspace can write as
    /// a `match` is one it writes as a `match`. Put the `match` in the
    /// initializer -- `let held = match value { Some(held) => held, None => …
    /// };` -- and the binding stays where it was.
    pub EXPLICIT023_NO_LET_ELSE,
    Deny,
    "`let … else` is forbidden; write the `match` in the initializer"
}

// Tests are exempt, as they are for every rule here. `opts.test` is true only
// for the harness build of a target, so the ordinary build of the same library
// is linted as production.
fn is_test_build(cx: &EarlyContext<'_>) -> bool {
    cx.sess().opts.test
}

impl EarlyLintPass for Explicit023NoLetElse {
    fn check_stmt(&mut self, cx: &EarlyContext<'_>, stmt: &Stmt) {
        if is_test_build(cx) {
            return;
        }

        let binding = match &stmt.kind {
            StmtKind::Let(local) => local,
            StmtKind::Expr(..)
            | StmtKind::Item(..)
            | StmtKind::Semi(..)
            | StmtKind::Empty
            | StmtKind::MacCall(..) => return,
        };

        if !matches!(binding.kind, LocalKind::InitElse(..)) {
            return;
        }

        // A macro's `let … else` is the macro author's.
        if stmt.span.from_expansion() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT023_NO_LET_ELSE,
            stmt.span,
            "`let … else` leaves an outcome without a name: the case that did not bind is spelled `else`",
            None,
            "write the decision as a `match` in the initializer -- `let held = match value { Some(held) => held, None => … };` -- so the case that leaves is named as well as the case that binds",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
