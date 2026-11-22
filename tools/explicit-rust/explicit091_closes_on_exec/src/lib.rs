//! EXPLICIT091: a descriptor made here is not handed to every program
//! started after it.
//!
//! A descriptor survives `exec` unless it was told not to. The standard
//! library tells every one it opens; the kernel's own calls do not, and
//! `rustix` asks the kernel in the kernel's words, so `pipe()` hands back two
//! ends that every program this process starts from then on is born holding.
//! This desktop starts a great many programs. What they inherit is whatever
//! was open: the write end of a pipe, which now never reports that its writer
//! has gone, because a terminal three programs away still has it; the bus,
//! the lock, a socket to the compositor, in a program nobody meant to give
//! them to. The first is a wait that never ends, which `console-program-
//! lifetime` was written over; the second is a capability that leaked.
//!
//! The flag is `CLOEXEC`, and it has to be set in the call that makes the
//! descriptor -- setting it afterwards leaves a gap in which another thread
//! can start a program, which is why the kernel grew `pipe2`, `accept4` and
//! `dup3`. So the rule asks the calls that make one: the forms that take
//! flags say `CLOEXEC` among them, and the forms that take none -- `pipe`,
//! `dup`, `accept`, `socket` -- are the older spelling of a call that does.
//!
//! A descriptor that is *meant* to cross an `exec` is the other half, and it
//! is the one the site should say: it is made closing like the rest, and the
//! child is given its copy on purpose, where it is started.
//!
//! Immutable bindings can carry the flag to the call. Linux opens process
//! descriptors with close-on-exec already set (see pidfd_open(2)).
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::source::snippet_opt;
use rustc_errors::Applicability;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::Ty;

dylint_linting::impl_late_lint! {
    /// EXPLICIT091: a `rustix` call that makes a descriptor says `CLOEXEC`
    /// in the call that makes it, and the forms with no flags are not used.
    pub EXPLICIT091_CLOSES_ON_EXEC,
    Warn,
    "a descriptor made without CLOEXEC is inherited by every program started after it",
    Explicit091ClosesOnExec {
        local_pipes: dylint_linting::config_or_default("explicit091_local_pipes"),
    }
}

pub struct Explicit091ClosesOnExec {
    local_pipes: Vec<String>,
}

enum Made {
    WithNoFlags(&'static str),
    WithFlags(usize),
}

fn makes_a_descriptor(path: &str) -> Option<Made> {
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "pipe" => Some(Made::WithNoFlags("pipe_with")),
        "dup" => Some(Made::WithNoFlags("fcntl_dupfd_cloexec")),
        "accept" => Some(Made::WithNoFlags("accept_with")),
        "acceptfrom" => Some(Made::WithNoFlags("acceptfrom_with")),
        "socket" => Some(Made::WithNoFlags("socket_with")),
        "pipe_with" | "userfaultfd" => Some(Made::WithFlags(0)),
        "open" | "accept_with" | "acceptfrom_with" | "memfd_create" | "eventfd"
        | "timerfd_create" => Some(Made::WithFlags(1)),
        "openat" | "openat2" | "socket_with" | "socketpair" | "dup3" | "signalfd" => {
            Some(Made::WithFlags(2))
        }
        "init" if path.contains("inotify") => Some(Made::WithFlags(0)),
        "create" if path.contains("epoll") => Some(Made::WithFlags(0)),
        _ => None,
    }
}

fn has_cloexec<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>, flags: Ty<'tcx>) -> bool {
    let expr = clippy_utils::expr_or_init(cx, expr);
    if cx.typeck_results().expr_ty(expr) != flags {
        return false;
    }
    match expr.kind {
        ExprKind::Path(ref path) => matches!(cx.qpath_res(path, expr.hir_id),
            Res::Def(DefKind::AssocConst { .. }, id) if cx.tcx.crate_name(id.krate).as_str() == "rustix"
                && cx.tcx.item_name(id).as_str() == "CLOEXEC"),
        ExprKind::Binary(op, left, right) if op.node == BinOpKind::BitOr => {
            cx.typeck_results().expr_ty(left) == flags
                && cx.typeck_results().expr_ty(right) == flags
                && (has_cloexec(cx, left, flags) || has_cloexec(cx, right, flags))
        }
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit091ClosesOnExec {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, args) = expr.kind else {
            return;
        };

        let ExprKind::Path(ref qpath) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = cx.qpath_res(qpath, called.hir_id) else {
            return;
        };

        if cx.tcx.crate_name(id.krate).as_str() != "rustix" {
            return;
        }

        let named = cx.tcx.def_path_str(id);

        let said = match makes_a_descriptor(named.as_str()) {
            None => return,
            Some(Made::WithNoFlags(instead)) => {
                format!(
                    "`{named}` takes no flags, so what it makes is open in every program started after it; `{instead}` is the call that can say otherwise"
                )
            }
            Some(Made::WithFlags(index)) => match args.get(index) {
                Some(flags) if has_cloexec(cx, flags, cx.typeck_results().expr_ty(flags)) => {
                    return;
                }
                _ => {
                    format!("`{named}` makes a descriptor and nothing in the call says `CLOEXEC`")
                }
            },
        };

        span_lint_and_then(
            cx,
            EXPLICIT091_CLOSES_ON_EXEC,
            expr.span,
            said,
            |diagnostic| {
                let owner = cx.tcx.hir_enclosing_body_owner(expr.hir_id).to_def_id();
                // The test harness adds a same-named descriptor beside a test.
                // Its compiler disambiguator is absent from the authored path.
                let caller = cx.tcx.def_path(owner).data.iter().try_fold(
                    cx.tcx.crate_name(owner.krate).to_string(),
                    |mut path, part| {
                        let name = part.data.get_opt_name()?;
                        path.push_str("::");
                        path.push_str(name.as_str());
                        Some(path)
                    },
                );
                if args.is_empty()
                    && named == "rustix::pipe::pipe"
                    && cx.tcx.sess.target.os.desc() == "linux"
                    && caller.is_some_and(|path| self.local_pipes.contains(&path))
                    && let Some(source) = snippet_opt(cx, expr.span)
                    && matches!(
                        source.as_str(),
                        "::rustix::pipe::pipe()" | "rustix::pipe::pipe()"
                    )
                {
                    let prefix = source.trim_end_matches("pipe()");
                    diagnostic.span_suggestion(
                        expr.span,
                        "close both ends of this process-local pipe on exec",
                        format!("{prefix}pipe_with({prefix}PipeFlags::CLOEXEC)"),
                        Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help("set `CLOEXEC` in the flags that create the descriptor, and hand a child the descriptor it needs where the child is started");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}

#[test]
fn reviewed_pipes() {
    dylint_testing::ui::Test::example(env!("CARGO_PKG_NAME"), "owned_pipe")
        .dylint_toml(r#"explicit091_local_pipes = ["owned_pipe::local_test_channel", "owned_pipe::tests::receives_a_local_pipe"]"#)
        .run();
}
