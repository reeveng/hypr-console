#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT035: a thread is started with its lifetime said at the call, the
    /// way a child is.
    ///
    /// `console-program-lifetime` is this argument already made about a
    /// `std::process::Child`: at the call site a thing started to run alongside
    /// and a thing started on somebody's behalf are the same three lines, and
    /// afterwards they are not the same at all. `Alongside` and `LetGo` exist
    /// so that a caller has to say which it meant, in a word still there to
    /// read a year later, and
    /// `console-manifest-engine/tests/the_children.rs` holds that shut.
    ///
    /// Threads were left out of it. `let _ = std::thread::spawn(...)` is the
    /// same decision taken by omission: the handle is dropped on the line that
    /// made it, so nothing can ask whether the thread is still running, nothing
    /// can wait for it, and nothing at the site says whether outliving its
    /// starter was meant or forgotten. A thread that panics takes its fault to
    /// the handle nobody kept.
    ///
    /// There are two answers and this rule asks for either. Hold the handle and
    /// `join` it where the thread ends, which is the shape whenever the thread
    /// is doing one thing the caller is waiting on. Or say that it was let go:
    /// `console_program_lifetime::threads::let_go` takes the handle and drops
    /// it, which is the same drop with a word on it, and it takes only a thread
    /// that answers nothing -- an answer let go is a value nobody will ever
    /// read.
    ///
    /// A thread cannot be killed from outside in Rust, so there is no
    /// `Alongside` for one. That asymmetry is the reason the word matters more
    /// here than it does for a child: for a child the type also does something,
    /// and for a thread the only thing on offer is saying what was meant.
    pub EXPLICIT035_NO_LOOSE_THREAD,
    Deny,
    "a thread whose handle is dropped where it is made says nothing about how long it lives"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// `thread::spawn` hands back a handle; `Builder::spawn` hands back one inside a
// `Result`. Both are the same decision, so the `Result` is peeled first rather
// than the rule being written twice.
fn holds_a_thread<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
    let rustc_middle::ty::Adt(held, args) = ty.peel_refs().kind() else {
        return false;
    };

    if cx.tcx.is_diagnostic_item(rustc_span::sym::Result, held.did()) {
        return args
            .types()
            .next()
            .is_some_and(|inside| holds_a_thread(cx, inside));
    }

    cx.tcx.def_path_str(held.did()) == "std::thread::JoinHandle"
}

fn dropped_where_it_was_made<'tcx>(stmt: &'tcx Stmt<'tcx>) -> Option<&'tcx rustc_hir::Expr<'tcx>> {
    match stmt.kind {
        StmtKind::Semi(made) => Some(made),
        StmtKind::Let(local) => match (local.pat.kind, local.init) {
            (PatKind::Wild, Some(made)) => Some(made),
            _ => None,
        },
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit035NoLooseThread {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        if stmt.span.from_expansion() {
            return;
        }
        let Some(made) = dropped_where_it_was_made(stmt) else {
            return;
        };
        if !holds_a_thread(cx, cx.typeck_results().expr_ty(made)) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT035_NO_LOOSE_THREAD,
            stmt.span,
            "this thread's handle is dropped on the line that made it, so nothing can ask whether it is \
             still running and nothing says whether outliving its starter was meant",
            None,
            "hold the handle and `join` it where the thread ends, or say that it was let go: \
             `console_program_lifetime::threads::let_go` is the same drop with a word on it, and it is \
             the argument `Alongside` and `LetGo` already make about a child",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
