//! EXPLICIT085: `/tmp` belongs to everybody, so nothing is kept there by name.
//!
//! A directory every user on the machine can write to is a directory where a
//! name is a guess somebody else can make first. `temp_dir().join(format!(
//! "console-{named}-{pid}"))` is such a name -- a pid is a small number and
//! anybody can read which ones are taken -- so whoever makes that path before
//! this desktop does decides what is at the end of it: a link to a file the
//! writer may write and the guesser may not, or a directory the guesser owns
//! under something that believed it had made one. The fault needs no skill and
//! no race worth the name, and it is why every system has a per-user runtime
//! directory that only its owner can enter.
//!
//! The second half is quieter. Two crates here answer an unset
//! `XDG_RUNTIME_DIR` with `/tmp`, which is EXPLICIT033 exactly: an absence
//! answered by a value, and the value is the one place on the machine that is
//! not private. A session with no runtime directory is a fault to say, not a
//! reason to move the socket somewhere anybody can stand.
//!
//! `console-core-temporary-directories` is where a place to put something for
//! a while is asked for, and it is the one crate this rule does not ask: what
//! it hands back should be made under the runtime directory, by a call that
//! fails if the name was already there, with a mode only its owner passes.
//! Everything else names neither `/tmp` nor `temp_dir`.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT085: no `std::env::temp_dir` and no literal under `/tmp`,
    /// `/var/tmp` or `/dev/shm` outside `console-core-temporary-directories`.
    pub EXPLICIT085_NO_SHARED_TEMPORARY,
    Warn,
    "a name in a directory everybody can write to is a name somebody else can make first"
}

const SHARED: [&str; 3] = ["/tmp", "/var/tmp", "/dev/shm"];

fn owns_the_question(cx: &LateContext<'_>) -> bool {
    cx.tcx.crate_name(LOCAL_CRATE).as_str() == "console_core_temporary_directories"
}

fn is_shared(said: &str) -> bool {
    SHARED.iter().any(|shared| {
        said.strip_prefix(shared)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

fn names_a_shared_place(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    match expr.kind {
        ExprKind::Lit(said) => match said.node {
            rustc_ast::LitKind::Str(text, _) if is_shared(text.as_str()) => Some(format!("\"{text}\"")),
            _ => None,
        },
        ExprKind::Call(called, _) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(DefKind::Fn, id) = cx.qpath_res(qpath, called.hir_id) else {
                return None;
            };

            match cx.tcx.def_path_str(id).as_str() {
                "std::env::temp_dir" => Some("std::env::temp_dir".to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit085NoSharedTemporary {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() || owns_the_question(cx) {
            return;
        }

        let Some(named) = names_a_shared_place(cx, expr) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT085_NO_SHARED_TEMPORARY,
            expr.span,
            format!("`{named}` is a place every user of this machine can write to"),
            None,
            "ask `console-core-temporary-directories` for somewhere to put it, which is made under \
             the session's own runtime directory by a call that fails when the name is already \
             there. A session with no runtime directory is a fault to say rather than a reason to \
             stand somewhere shared. Where the path is only a value and nothing is ever made at it, \
             allow this rule here and let the reason say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
