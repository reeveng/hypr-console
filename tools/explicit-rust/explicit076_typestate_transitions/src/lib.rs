//! EXPLICIT076: a state changes through the door that owns it.
//!
//! `console-core-state-machine` is the shape the whole tree is written in: a
//! state, a request in, a new state out, and effects offered -- so every
//! change to a state is one answer a reader can find, and every state that
//! can follow this one is named where the machine is. The opposite of that
//! shape is the field a caller can set: `held.state = Running`, written
//! wherever the caller happens to be, makes the machine's states a
//! convention rather than a type -- nothing holds the invariant that the
//! change *means* what the machine says it means, and nothing catches a
//! second writer writing it from underneath the first.
//!
//! What this rule asks is the half that can be seen: an assignment to a
//! state-named field (`state`, `mode`, `phase`) on a value the function was
//! handed rather than on `self` -- a change written from outside the type
//! that owns it. The fix is the machine: the value wants a request, or a
//! method on the type, so the change goes through one door and the reader
//! can name it. The half this rule cannot see -- a state changed without a
//! transition at all, or a type that is one state and claims another -- is
//! EXPLICIT048's question and the machine's own, which is why this tree has
//! both.
//!
//! It arrives `Warn`, over a tree whose machines already change state
//! through requests, and the sites it prints are the ones that slipped past
//! the shape on their way in.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT076: an assignment to a state-named field on a value the
    /// function was handed is a change written from outside the type that
    /// owns it. Send a request, or move the change into a method.
    pub EXPLICIT076_TYPESTATE_TRANSITIONS,
    Warn,
    "a state field assigned from outside; the change wants the door that owns it"
}

// The names a state goes by when it is a field. An invented word is a fault
// this rule cannot name; these are the three the rest of the industry reads
// as a state, which is the vocabulary `vocabulary.conf` points at.
fn is_a_state(name: &str) -> bool {
    matches!(name, "state" | "mode" | "phase")
}

// Whose field is being written: `self` is the type itself changing its own
// mind, which is the method's to do; anything else is somebody else's.
fn is_self(cx: &LateContext<'_>, base: &Expr<'_>) -> bool {
    match base.kind {
        ExprKind::Path(QPath::Resolved(_, path)) => match path.res {
            Res::Local(hir_id) => cx.tcx.hir_name(hir_id).as_str() == "self",
            _ => false,
        },
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit076TypestateTransitions {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Assign(written, _, _) = expr.kind else {
            return;
        };

        let ExprKind::Field(base, field) = written.kind else {
            return;
        };

        if !is_a_state(field.name.as_str()) {
            return;
        }

        if is_self(cx, base) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT076_TYPESTATE_TRANSITIONS,
            expr.span,
            format!("`{field}` on a value the function was handed is being set from outside"),
            None,
            "send the change through the door that owns it: a request the machine answers, or a \
             method on the type, so every change is one place a reader can name and no second \
             writer writes the state from underneath the first. Where the value really is this \
             type's to set -- a field it handed out for one call -- allow this rule here and let \
             the reason say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
