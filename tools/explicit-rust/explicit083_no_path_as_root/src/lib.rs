//! EXPLICIT083: a path is a question asked again at every use, and root
//! should not ask it of a directory somebody else can write to.
//!
//! A path is not a file. It is a walk from the top of the tree that the
//! kernel makes again each time it is handed one, and every component of it
//! is whatever is there at that moment. The engine runs as root and lays
//! files into a home its owner can write to, so each `std::fs` call that
//! takes a path is root following directions the owner of that home can
//! change between one call and the next: a symlink where a directory was, and
//! the `chown` that was meant for a dotfile is a `chown` of `/etc/shadow`.
//! Nothing in the line says so and nothing fails.
//!
//! The answer is the one the kernel has had for a long time. A directory is
//! opened once, as a descriptor, and everything under it is reached from that
//! descriptor with a walk that may not leave it and does not follow a link --
//! `openat2` with `RESOLVE_BENEATH`, then `fchown` and `fchmod` on what was
//! opened, which is the file and not a name for it. That wants one crate to
//! hold it, the way `console-core-atomic-writes` holds a write, and
//! EXPLICIT040 already sends every write there: the fix is mostly made once,
//! behind a door this rule points at.
//!
//! It is asked of the crates that run as root and of the one their writes go
//! through, because that is where the two ends of the fault are -- a
//! privileged process and a directory it does not own. The desktop's own
//! programs run as the person whose files they are, and a path is theirs to
//! follow. Its case is built as a crate named like the engine's, as the
//! flight rules' are.
//!
//! It arrives `Warn`, well ahead of the code.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT083: in a crate that runs as root, nothing is opened, made,
    /// removed, renamed or given away by path. Open the directory once and
    /// reach beneath it by descriptor, without following links.
    pub EXPLICIT083_NO_PATH_AS_ROOT,
    Warn,
    "a path followed by root is directions somebody else can change between two calls"
}

fn runs_as_root(cx: &LateContext<'_>) -> bool {
    let named = cx.tcx.crate_name(LOCAL_CRATE);
    let named = named.as_str();

    named.starts_with("console_manifest_engine")
        || named.starts_with("console_login")
        || named == "console_core_atomic_writes"
}

fn follows_a_path(path: &str) -> bool {
    let last = path.rsplit("::").next().unwrap_or_default();

    let free = path
        .strip_prefix("std::fs::")
        .is_some_and(|rest| !rest.contains("::") && rest != "exists");

    let opened = matches!(
        path,
        "std::fs::File::open"
            | "std::fs::File::create"
            | "std::fs::File::create_new"
            | "std::fs::OpenOptions::open"
            | "std::fs::DirBuilder::create"
    );

    let given_away = path.starts_with("std::os::unix::fs::")
        && matches!(last, "chown" | "lchown" | "symlink" | "chroot");

    let asked = path.starts_with("std::path::Path::")
        && matches!(
            last,
            "metadata" | "symlink_metadata" | "read_dir" | "read_link" | "canonicalize"
        );

    free || opened || given_away || asked
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

impl<'tcx> LateLintPass<'tcx> for Explicit083NoPathAsRoot {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if !runs_as_root(cx) || expr.span.from_expansion() {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        if !follows_a_path(named.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT083_NO_PATH_AS_ROOT,
            expr.span,
            format!("`{named}` walks a path as root, and the walk is whatever is there when it is made"),
            None,
            "open the directory once and reach what is under it from that descriptor, with a walk \
             that may not leave it and follows no link (`openat2` and `RESOLVE_BENEATH`), then ask \
             and change the thing that was opened (`fchown`, `fchmod`, `fstat`) rather than its name. \
             Where nobody but root can write to any directory on the way, allow this rule here and \
             let the reason name the directory",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "console_manifest_engine_case");
}
