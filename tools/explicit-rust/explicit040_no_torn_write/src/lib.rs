//! EXPLICIT040: a file is written whole or not at all, and there is one place
//! that knows how.
//!
//! `console-core-atomic-writes` was written because nothing in this workspace
//! called `fsync`, and its head is the whole argument: a rename is atomic
//! against a reader and says nothing about power, so a machine that stops
//! between the write and the commit comes back to a name that resolves to a
//! file of no length. On a handheld that is not a thought experiment, it is
//! the battery running out. `fs::write` straight over the live file is worse
//! again -- it truncates and then fills, so there is a window in which the
//! file genuinely is half of itself.
//!
//! That argument was made once and then half-kept. The crate exists and is
//! used, and the raw spelling is still in most of the crates that keep
//! anything: a settings file, a wallpaper's memory of what it was showing, a
//! staged copy, a palette. Each of those is a file this desktop
//! reads on the next boot, which is the one moment the fault shows up and the
//! one moment nobody is watching.
//!
//! It is EXPLICIT001's rule about swallowing a failure, said about the other
//! end of the same call. `unwrap_or_default()` on a read turns "could not read
//! this" into "this was empty"; a torn write is what *makes* it empty, and the
//! two of them together are a desktop that silently forgets what it was told,
//! with no line anywhere saying so.
//!
//! So the write goes through `whole` or `settled` and the rule denies the
//! spellings that do not. The owning crate carries the allow, which is how
//! EXPLICIT026 draws the same boundary for a name out of the environment: a
//! list of exempt crates inside this lint would not be greppable, and a second
//! crate wanting to write its own way has to come and take the allow off the
//! first one.
//!
//! What it stops at is the declaration of intent rather than the bytes. It
//! asks about `create`, `truncate`, `append` and `write` on an `OpenOptions`,
//! because those are where a program says it is about to change a file;
//! `write_all` further down is the same call by then and would fire on a
//! socket and on stdout as well.
//!
//! Reading is not touched here. A file opened for reading is `File::open`,
//! which this rule never sees, and what a read owes is the other half of that
//! crate: `Held` separates absent, which means the default, from unreadable,
//! which is a fault.
//!
//! It arrived `Warn` and is denied. Most of what it found was one call and the
//! answer to nearly all of it was the same two words, which is the largest
//! sweep since 033 and the least argued-with: a settings file, a panel's memory
//! of the tab it was on, the session that decides what comes back after a
//! reboot, the icon store, the staged tree, the public copy. Two of them were
//! more than a sweep -- the session was streamed into an open file a line at a
//! time and is now built whole and written once, and the add-on packer had
//! written its own beside-and-rename without either `fsync`, which is the half
//! of the argument this crate was written for. What is left carrying the allow
//! is the four kinds that are not a file being replaced: a kernel knob under
//! `/sys`, which has nothing beside it to write and nothing to rename over; a
//! log appended to a line at a time; a lock, whose whole point is the open file
//! rather than its bytes; and this crate itself.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT040: writing a file straight over the live one leaves it half
    /// of itself if the machine stops, and writing beside it without an
    /// `fsync` leaves a name pointing at nothing. `console_core_atomic_writes`
    /// is where that is done properly, once.
    pub EXPLICIT040_NO_TORN_WRITE,
    Deny,
    "a file written outside the crate that knows how to write one whole"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Where a program says it is about to change a file. The two that open one and
// the four an `OpenOptions` is built out of; everything after them is bytes,
// and by then the file is already open and already truncated.
fn changes_a_file(path: &str) -> Option<&'static str> {
    match path {
        "std::fs::write" => Some("this truncates the live file and then fills it, so there is a window in which the file is half of itself"),
        "std::fs::File::create" | "std::fs::File::create_new" => {
            Some("this opens the live file and empties it before anything has been written")
        }
        "std::fs::OpenOptions::write" | "std::fs::OpenOptions::append" => {
            Some("this opens the live file to be changed, and nothing here commits it")
        }
        "std::fs::OpenOptions::create" | "std::fs::OpenOptions::create_new" | "std::fs::OpenOptions::truncate" => {
            Some("this makes or empties the live file, and a machine that stops after it leaves that")
        }
        _ => None,
    }
}

// What a call resolves to, whether it was written as a path or as a method.
// The `OpenOptions` half is always a method and the `fs` half never is.
fn what_is_called(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    match expr.kind {
        ExprKind::Call(called, _) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id) else {
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

impl<'tcx> LateLintPass<'tcx> for Explicit040NoTornWrite {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        let Some(harm) = changes_a_file(named.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT040_NO_TORN_WRITE,
            expr.span,
            format!("`{named}` changes a file in place: {harm}"),
            None,
            "`console_core_atomic_writes::whole` writes the bytes beside the file, commits them, renames \
             over the name and commits the directory, so a machine that stops has either the old file or \
             the new one; `settled` is the same for a file that is read back immediately. Where this crate \
             is the one that knows how, or where the file is a log being appended to and its loss costs \
             nothing, allow this rule here and let the reason say which",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
