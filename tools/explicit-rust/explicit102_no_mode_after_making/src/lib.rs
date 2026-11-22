//! EXPLICIT102: a file is made with the mode it is meant to have.
//!
//! `File::create` makes a file anybody may read, less whatever the umask
//! takes away, and a `set_permissions` on the next line narrows it. Between
//! the two the file exists with the wrong mode, and anything that opens it in
//! that gap holds a descriptor the narrowing does not take back -- a
//! descriptor is checked when it is opened and never again. For a key, a
//! token, a session's own secret, that gap is the whole of the exposure, and
//! it is wide enough: a watcher on the directory is told the moment the name
//! appears. It is EXPLICIT084's fault with the order turned round. There the
//! check came before the act; here the protection comes after it.
//!
//! The other half is the name. `std::fs::set_permissions` takes a path, and a
//! path is walked again when it is handed over, so what gets the new mode is
//! whatever is at that name by then -- through a link, if somebody put one
//! there, which is how a `chmod` meant for a file this program made becomes a
//! `chmod` of a file it did not.
//!
//! Both have one answer and the standard library has it. A mode is said in
//! the call that makes the file, `OpenOptions::mode` beside `create_new`, so
//! there is no moment at which it is wrong; `console-core-atomic-writes` is
//! where a write is made whole and is where that belongs. And a mode that
//! really must change later is changed on the thing that was opened --
//! `File::set_permissions`, which is `fchmod` -- rather than on its name.
//!
//! So the rule asks one call: `std::fs::set_permissions`, the one that takes
//! a path. It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT102: no `std::fs::set_permissions`. Say the mode where the
    /// file is made, or change it on the open file.
    pub EXPLICIT102_NO_MODE_AFTER_MAKING,
    Warn,
    "a mode set by name after the file exists leaves a gap before it, and a path walked again"
}

impl<'tcx> LateLintPass<'tcx> for Explicit102NoModeAfterMaking {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };

        let ExprKind::Path(ref qpath) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = cx.qpath_res(qpath, called.hir_id) else {
            return;
        };

        if cx.tcx.def_path_str(id) != "std::fs::set_permissions" {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT102_NO_MODE_AFTER_MAKING,
            expr.span,
            "`std::fs::set_permissions` changes whatever is at this name now, after it has already existed with another mode",
            None,
            "say the mode in the call that makes the file -- `OpenOptions::mode` beside `create_new` \
             -- so it is never wrong, and change one later on the open file with \
             `File::set_permissions`, which follows no name. Where the file was not made here and \
             cannot be opened, allow this rule and let the reason say whose it is",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
