//! EXPLICIT101: a file is not made for everybody to write, and nothing is
//! made to run as its owner.
//!
//! A mode is nine bits and three more, and two of the twelve are the ones
//! faults are made of. The bit that lets *others* write turns a file into a
//! message board: a configuration any program on the machine can rewrite, a
//! script in a directory root runs from, a socket anybody can speak on. The
//! set-user and set-group bits do the reverse -- whoever runs the file runs
//! it as whoever owns it, which is how a helper written for one job becomes
//! the way up. Both are written as digits in an octal number, `0o666` and
//! `0o4755`, that reads as a permission like any other and is checked by
//! nobody; `0o777` is the mode people type when something would not open and
//! they wanted it to.
//!
//! CLAUDE.md says a file's mode is derived from its path and content rather
//! than declared, and this is the same argument one layer down: where a mode
//! *is* written, in the code that makes a file, the two shapes that are a
//! door are not among the ones it may write. `0o600` and `0o644` and `0o755`
//! pass. Anything with the others-write bit or a set-id bit is asked about,
//! and so is `rustix`'s spelling of the same bits by name.
//!
//! The door it leaves is real and small. A directory that is meant to be
//! shared -- `/tmp` itself is `0o1777` -- carries the sticky bit, which this
//! rule lets through with the others-write bit beside it, because that pair
//! is the one arrangement in which a shared directory is safe. A program
//! that must be set-id says so at the site, and says who it becomes.
//!
//! It arrives `Warn`, and expects to find very little.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT101: a mode written in the source has no others-write bit
    /// without the sticky bit, and no set-user or set-group bit.
    pub EXPLICIT101_NO_OPEN_MODE,
    Warn,
    "a mode that lets everybody write, or runs a file as its owner, is a door written as a number"
}

const OTHERS_WRITE: u128 = 0o002;
const STICKY: u128 = 0o1000;
const SET_ID: u128 = 0o6000;

fn takes_a_mode(path: &str) -> bool {
    let last = path.rsplit("::").next().unwrap_or_default();

    let named = matches!(last, "mode" | "set_mode" | "from_mode" | "from_raw_mode" | "from_bits" | "from_bits_truncate" | "from_bits_retain");

    let about_files = ["OpenOptions", "DirBuilder", "Permissions", "Mode"].iter().any(|word| path.contains(word));

    named && about_files
}

fn what_it_opens(mode: u128) -> Option<&'static str> {
    if mode & SET_ID != 0 {
        return Some("sets the user or the group of whoever runs the file to its owner's");
    }

    if mode & OTHERS_WRITE != 0 && mode & STICKY == 0 {
        return Some("lets every user on the machine write to it");
    }

    None
}

fn written(expr: &Expr<'_>) -> Option<u128> {
    let ExprKind::Lit(said) = expr.kind else {
        return None;
    };

    match said.node {
        rustc_ast::LitKind::Int(value, _) => Some(value.get()),
        _ => None,
    }
}

fn said(cx: &LateContext<'_>, at: &Expr<'_>, what: String) {
    span_lint_and_help(
        cx,
        EXPLICIT101_NO_OPEN_MODE,
        at.span,
        what,
        None,
        "write the narrowest mode that does the job -- `0o600` for what one person keeps, `0o644` \
         for what others read, `0o755` for what they run. Where a directory is shared on purpose, \
         give it the sticky bit beside the write bit; where a program must run as its owner, allow \
         this rule here and let the reason say who it becomes and why",
    );
}

impl<'tcx> LateLintPass<'tcx> for Explicit101NoOpenMode {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        // `rustix` names the bits, and a name is as much a mode as a digit.
        if let ExprKind::Path(ref qpath) = expr.kind {
            let Res::Def(DefKind::AssocConst { .. }, id) = cx.qpath_res(qpath, expr.hir_id) else {
                return;
            };

            let named = cx.tcx.item_name(id);

            if cx.tcx.crate_name(id.krate).as_str() == "rustix" && matches!(named.as_str(), "WOTH" | "RWXO" | "SUID" | "SGID") {
                said(cx, expr, format!("`{named}` is a mode bit that opens a file to everybody or runs it as its owner"));
            }

            return;
        }

        let (id, given) = match expr.kind {
            ExprKind::Call(called, given) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id) else {
                    return;
                };

                (id, given)
            }
            ExprKind::MethodCall(_, _, given, _) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                (id, given)
            }
            _ => return,
        };

        if !takes_a_mode(cx.tcx.def_path_str(id).as_str()) {
            return;
        }

        for one in given {
            let Some(mode) = written(one) else {
                continue;
            };

            if let Some(opens) = what_it_opens(mode) {
                said(cx, one, format!("the mode `{mode:#o}` {opens}"));
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
