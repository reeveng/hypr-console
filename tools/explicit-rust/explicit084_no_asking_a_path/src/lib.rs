//! EXPLICIT084: a file is not asked whether it is there; it is opened.
//!
//! `exists`, `is_file` and `is_dir` answer for the moment they were asked and
//! for no moment after it. Whatever the answer decides -- read it, make it,
//! leave it alone -- happens later, against a path that has had time to
//! become something else, so the line that follows is acting on a fact nobody
//! holds any more. That is a race in every program and a door in a privileged
//! one, and it has a quieter fault besides: all three answer `false` for a
//! file that is there and could not be looked at, which is EXPLICIT001's
//! complaint -- a failure met and said as an absence.
//!
//! The thing itself can be asked, and its answer cannot go stale. Opening a
//! file either hands back the file or says why not, and `NotFound` is one of
//! the things it says; `create_new` makes a file only where there was none,
//! in one step the kernel does not let anybody into the middle of. That is
//! the shape EXPLICIT016 already asks a fault to be read in --
//! `fault.kind() == NotFound`, with a name on both answers -- and it is
//! `console-waiting`'s argument about time, said about the filesystem: ask
//! the thing, not something that was true of it a moment ago.
//!
//! `metadata` is not asked about here. What it hands back is a value somebody
//! goes on to read, and its failure is a `Result`.
//!
//! It arrives `Warn`, over a tree that asks first in a great many places.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT084: no `exists`, `is_file`, `is_dir`, `is_symlink` or
    /// `try_exists` on a path. Open it, or make it with `create_new`, and
    /// meet what that says.
    pub EXPLICIT084_NO_ASKING_A_PATH,
    Warn,
    "a path asked whether it is there has answered for a moment that is already over"
}

fn asks_before_using(path: &str) -> bool {
    let last = path.rsplit("::").next().unwrap_or_default();

    let of_a_path = path.starts_with("std::path::Path::")
        && matches!(last, "exists" | "try_exists" | "is_file" | "is_dir" | "is_symlink");

    of_a_path || path == "std::fs::exists"
}

fn what_is_called(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    match expr.kind {
        ExprKind::Call(called, _) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id)
            else {
                return None;
            };

            Some(cx.tcx.def_path_str(id))
        }
        ExprKind::MethodCall(..) => {
            let id = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;

            Some(cx.tcx.def_path_str(id))
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit084NoAskingAPath {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        if !asks_before_using(named.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT084_NO_ASKING_A_PATH,
            expr.span,
            format!("`{named}` answers for now, and whatever it decides happens later"),
            None,
            "do the thing and meet its answer: open the file and read `fault.kind() == NotFound` as \
             the absence, or make it with `create_new`, which refuses a file that is already there. \
             Where the answer is the whole of what is wanted and nothing acts on it afterwards, \
             allow this rule here and let the reason say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
