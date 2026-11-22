//! EXPLICIT095: what runs in a signal handler, or between `fork` and `exec`,
//! is on a list, and the list is short.
//!
//! A signal handler runs wherever the signal found the thread, and a
//! `pre_exec` closure runs in a child that is a copy of one thread of a
//! process that may have had many. In both places every lock in the program
//! is in whatever state it was in at that instant, with nobody left who will
//! let go of it. The allocator has a lock. So does standard output, and so
//! does everything that formats. A handler that allocates, on the day the
//! signal lands inside `malloc`, waits for a lock its own thread holds, and
//! the program stops without a word -- which is found on a device, weeks
//! later, as a desktop that would not close. POSIX keeps a list of what may
//! be called there, and almost nothing in a standard library is on it.
//!
//! The language cannot say any of this. `install_handler` is `unsafe` and
//! `pre_exec` is `unsafe`, and EXPLICIT012 has each of them carry a sentence
//! promising the body is safe to run there -- a sentence that stays true only
//! until somebody adds a `format!` to it. This rule is that promise read by
//! the compiler: in the function handed to `install_handler` and in the
//! closure handed to `pre_exec`, a call is to `core`, which takes no lock and
//! touches no heap, or to `rustix`, which is the kernel asked directly.
//! Everything else is a call somebody has to answer for -- `std`, `alloc`,
//! and a function of this tree's, whose body the rule does not follow and a
//! reader of the handler should not have to.
//!
//! The shape it is pushing toward is the one the handlers here already have:
//! store to an atomic or write one byte down a pipe, and let a thread that
//! is allowed to do things do them. A C function that POSIX does list --
//! `setuid`, `_exit` -- is an allow that says so.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT095: the function handed to `install_handler` and the closure
    /// handed to `pre_exec` call into `core` and `rustix` and nothing else.
    pub EXPLICIT095_NO_UNSAFE_IN_A_HANDLER,
    Warn,
    "a call in a signal handler or before exec that may take a lock nobody is left to release"
}

struct Calls<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'a TypeckResults<'tcx>,
    unanswered: Vec<(String, Span)>,
}

impl<'tcx> Calls<'_, 'tcx> {
    fn asked(&mut self, id: DefId, at: Span) {
        if matches!(self.tcx.crate_name(id.krate).as_str(), "core" | "rustix") {
            return;
        }

        // A macro's call is reported where a person wrote the macro.
        let shown = match at.from_expansion() {
            true => at.source_callsite(),
            false => at,
        };

        self.unanswered.push((self.tcx.def_path_str(id), shown));
    }
}

impl<'tcx> Visitor<'tcx> for Calls<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        intravisit::walk_expr(self, expr);

        match expr.kind {
            ExprKind::Call(called, _) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                if let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = self.typeck.qpath_res(qpath, called.hir_id) {
                    self.asked(id, expr.span);
                }
            }
            ExprKind::MethodCall(..) => {
                if let Some(id) = self.typeck.type_dependent_def_id(expr.hir_id) {
                    self.asked(id, expr.span);
                }
            }
            _ => {}
        }
    }
}

// The body that will run where almost nothing may: a closure written at the
// call, or a function of this crate's named there.
fn the_body<'tcx>(cx: &LateContext<'tcx>, handed: &'tcx Expr<'tcx>) -> Option<&'tcx Body<'tcx>> {
    match handed.kind {
        ExprKind::Closure(closure) => Some(cx.tcx.hir_body(closure.body)),
        ExprKind::Path(ref qpath) => {
            let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, handed.hir_id) else {
                return None;
            };

            cx.tcx.hir_maybe_body_owned_by(id.as_local()?)
        }
        _ => None,
    }
}

fn where_it_runs<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> Option<(&'static str, &'tcx Expr<'tcx>)> {
    match expr.kind {
        ExprKind::MethodCall(_, _, [handed], _) => {
            let id = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;

            match cx.tcx.def_path_str(id).ends_with("CommandExt::pre_exec") {
                true => Some(("between `fork` and `exec`", handed)),
                false => None,
            }
        }
        ExprKind::Call(called, given) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(DefKind::Fn, id) = cx.qpath_res(qpath, called.hir_id) else {
                return None;
            };

            match cx.tcx.item_name(id).as_str() {
                "install_handler" => given.last().map(|handed| ("in a signal handler", handed)),
                _ => None,
            }
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit095NoUnsafeInAHandler {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some((place, handed)) = where_it_runs(cx, expr) else {
            return;
        };

        let Some(body) = the_body(cx, handed) else {
            return;
        };

        let mut calls = Calls { tcx: cx.tcx, typeck: cx.tcx.typeck_body(body.id()), unanswered: Vec::new() };

        calls.visit_expr(body.value);

        for (named, at) in calls.unanswered {
            span_lint_and_help(
                cx,
                EXPLICIT095_NO_UNSAFE_IN_A_HANDLER,
                at,
                format!("`{named}` is called {place}, where a lock held at that instant is held for good"),
                None,
                "store to an atomic or write a byte down a pipe, and let a thread that may do things \
                 do them; ask the kernel through `rustix`. Where the call is one POSIX lists as safe \
                 here, allow this rule at the call and let the reason say that it is",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
