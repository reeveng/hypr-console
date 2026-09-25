//! EXPLICIT046: a jump out of a nested loop says which loop it leaves.
//!
//! EXPLICIT019 wants both outcomes of a decision named, and EXPLICIT023 says
//! the same of a `let ... else` whose `else` names nothing. A bare `break`
//! inside two loops is the case neither of them reaches. It leaves one of the
//! two and the code does not say which, so the reader counts braces, and the
//! next person to wrap a loop around it changes where it goes without touching
//! the line.
//!
//! The rule is only about the nested case. One loop has one answer and a label
//! there would be ceremony, which is why this does not read as a rule about
//! labels: it is a rule about a jump whose destination is a guess.
//!
//! Kast is where the shape came from. It has no anonymous non-local exit at
//! all: `unwindable_block` makes a block and hands the body a token, and
//! `unwind token value` names the token it returns to, so the destination of a
//! jump is a value that had to be passed in before it could be used. A label is
//! the smallest version of that available here.
//!
//! What is not touched is a jump out of a loop inside a closure. A closure is
//! its own body and the loops outside it are not reachable from within, so the
//! `break` has one destination and says it by being where it is.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Destination, Expr, ExprKind, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT046: a bare `break` or `continue` inside a nested loop leaves
    /// one of the loops and does not say which, so the destination moves when
    /// someone wraps another loop around it. Name the loop with a label.
    pub EXPLICIT046_NO_UNNAMED_JUMP,
    Deny,
    "a jump out of a nested loop that does not name the loop it leaves"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn jumping(kind: &ExprKind<'_>) -> Option<(Destination, &'static str)> {
    match kind {
        ExprKind::Break(to, _value) => Some((*to, "break")),
        ExprKind::Continue(to) => Some((*to, "continue")),
        _ => None,
    }
}

// How many loops this jump is standing inside, stopping at the closure or the
// body that ends the walk. A labelled block is not a loop and is not counted:
// a `break` cannot reach one without naming it.
fn loops_around(cx: &LateContext<'_>, expr: &Expr<'_>) -> usize {
    let mut found = 0;

    for (_id, node) in cx.tcx.hir_parent_iter(expr.hir_id) {
        match node {
            Node::Expr(above) => match above.kind {
                ExprKind::Loop(..) => found += 1,
                ExprKind::Closure(..) => return found,
                _ => {}
            },
            Node::Item(_) => return found,
            _ => {}
        }
    }

    found
}

impl<'tcx> LateLintPass<'tcx> for Explicit046NoUnnamedJump {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let Some((to, word)) = jumping(&expr.kind) else {
            return;
        };

        let None = to.label else {
            return;
        };

        match loops_around(cx, expr) > 1 {
            true => span_lint_and_help(
                cx,
                EXPLICIT046_NO_UNNAMED_JUMP,
                expr.span,
                format!("this `{word}` is inside more than one loop and names none of them"),
                None,
                "label the loop this leaves and say it here: `'over_rows: for ...` and \
                 `break 'over_rows`. A bare jump leaves the innermost loop, which is a fact about \
                 where the line happens to sit rather than about what it means, and it moves the \
                 next time someone wraps a loop around it",
            ),
            false => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
