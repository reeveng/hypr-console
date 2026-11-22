//! EXPLICIT103: how many threads there are is not left to the length of a
//! list.
//!
//! A `spawn` inside a loop starts one thread for each thing the loop walks,
//! so the number of threads this program runs is the number of things, and
//! the number of things is whatever arrived: connections accepted, entries
//! in an archive, files in a directory somebody filled, messages on a bus. A
//! thread is a stack and a kernel task, and a few thousand of them is a
//! machine that has stopped answering. EXPLICIT029 says this of a program
//! started per item, where the cost is a fork; EXPLICIT035 asks how long a
//! thread lives; this is the third question about the same line, which is
//! how many of them there will be, and the loop answers "as many as I am
//! given" without anybody having decided so.
//!
//! The answer is a number somebody wrote. A fixed handful of workers reading
//! one queue -- a `sync_channel`, which is EXPLICIT097's bound doing the
//! other half of the job -- does the same work with a ceiling on it, and
//! what does not fit waits where it can be seen. `console-actor` is the
//! shape for one of them. A scope is fine where the list is this program's
//! own: `std::thread::scope` over a table typed out in the source cannot
//! grow.
//!
//! It reads the loops EXPLICIT028 and 029 read: a `for`, and the iterator
//! words that run a closure once for each item. Where the list has a length
//! somebody typed, or was bounded before the loop, the site says so.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, HirId, Node};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT103: no thread started from inside a loop. A fixed number of
    /// workers read one bounded queue.
    pub EXPLICIT103_NO_THREAD_PER_ITEM,
    Warn,
    "a thread started for each item is as many threads as whoever filled the list decided"
}

const ONCE_FOR_EACH: [&str; 9] =
    ["for_each", "try_for_each", "map", "filter_map", "flat_map", "filter", "inspect", "fold", "map_while"];

fn starts_a_thread(path: &str) -> bool {
    let scoped = path.contains("thread::Scope") && path.ends_with("::spawn");

    scoped
        || matches!(
            path,
            "std::thread::spawn" | "std::thread::Builder::spawn" | "std::thread::Builder::spawn_scoped"
        )
}

// Whether a closure is the body an iterator word runs once for each item.
fn runs_for_each(cx: &LateContext<'_>, closure: HirId) -> bool {
    match cx.tcx.parent_hir_node(closure) {
        Node::Expr(above) => match above.kind {
            ExprKind::MethodCall(path, ..) => ONCE_FOR_EACH.contains(&path.ident.name.as_str()),
            _ => false,
        },
        _ => false,
    }
}

fn is_in_a_loop(cx: &LateContext<'_>, from: HirId) -> bool {
    let mut at = from;

    // The walk ends at the function: a loop in a caller is the caller's.
    for _ in 0..64 {
        match cx.tcx.parent_hir_node(at) {
            Node::Expr(above) => {
                match above.kind {
                    ExprKind::Loop(..) => return true,
                    ExprKind::Closure(..) if !runs_for_each(cx, above.hir_id) => return false,
                    ExprKind::Closure(..) => return true,
                    _ => {}
                }

                at = above.hir_id;
            }
            Node::Block(above) => at = above.hir_id,
            Node::Stmt(above) => at = above.hir_id,
            Node::LetStmt(above) => at = above.hir_id,
            Node::Arm(above) => at = above.hir_id,
            Node::ExprField(above) => at = above.hir_id,
            _ => return false,
        }
    }

    false
}

impl<'tcx> LateLintPass<'tcx> for Explicit103NoThreadPerItem {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let id = match expr.kind {
            ExprKind::Call(called, _) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id) else {
                    return;
                };

                id
            }
            ExprKind::MethodCall(..) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                id
            }
            _ => return,
        };

        let named = cx.tcx.def_path_str(id);

        if !starts_a_thread(named.as_str()) || !is_in_a_loop(cx, expr.hir_id) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT103_NO_THREAD_PER_ITEM,
            expr.span,
            "a thread is started here once for each turn of the loop, so the list decides how many there are",
            None,
            "start a number of workers somebody wrote and hand them the items through one bounded \
             queue, so what does not fit waits. Where the list has a length typed out in the source \
             or was bounded before this loop, allow this rule here and let the reason say by what",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
