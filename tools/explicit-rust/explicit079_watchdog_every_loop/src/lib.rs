//! EXPLICIT079: what runs forever is asked to prove it is still running.
//!
//! The code that flies runs where nobody is watching, so the machine has to
//! watch it: a task that hangs looks identical, from outside, to a task that
//! is working, and the difference between a machine that reboots itself and
//! a machine somebody has to hold a power button about is a question the task
//! answers on purpose -- the kick, the feed, the "I am here" a watchdog
//! listens for. A task that runs a loop without one is a task whose hang is
//! discovered by whoever is holding the device.
//!
//! What this rule can see statically is the loop worth asking: in the flight
//! crates, a `for` whose source is endless or whose body can block -- a
//! channel, an event stream, a lock, a read -- is a task in the sense that
//! matters, and the rule asks it for its kick. A loop over a finite list is
//! not a task and is left alone. The watchdog itself is a machine this tree
//! has not written yet; the rule arrives `Warn` holding the shape for it, so
//! the first task loop the kernel writes meets its question on the day it is
//! written rather than on the day it hangs.
//!
//! The rule is asked of the flight crates (`console-kernel` and its family)
//! alone: the desktop runs under a session that restarts it, and its
//! equivalent answer is `console-test-stages`' `stopping`, which puts back
//! what was true before the first press.
//!
//! Its ui case is built as a crate named like the kernel's, because the rule
//! is asked only of `console-kernel*` and the case is the one way to press it
//! here on a desktop.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::higher::ForLoop;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};

dylint_linting::declare_late_lint! {
    /// EXPLICIT079: in the flight crates, a `for` over an endless source or
    /// with a blocking call inside it kicks the watchdog, so a task that
    /// hangs is reboots-itself rather than somebody-has-to-hold-the-button.
    pub EXPLICIT079_WATCHDOG_EVERY_LOOP,
    Warn,
    "a task loop with no kick in it; the watchdog is the difference between a hang and a reboot"
}

fn is_flight(cx: &LateContext<'_>) -> bool {
    cx.tcx
        .crate_name(LOCAL_CRATE)
        .as_str()
        .starts_with("console_kernel")
}

// The loop worth asking: its source never ends, or its body can be stopped
// by a machine that has gone wrong.
fn is_a_task(path: &str) -> Option<&'static str> {
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "repeat" | "cycle" | "from_fn" | "successors" if path.contains("iter::") => {
            Some("the source has no end in it")
        }
        "iter" if path.contains("mpsc") => Some(A_CHANNEL),
        "recv" | "lock" | "join" if path.contains("mpsc") || path.contains("sync") || path.contains("thread") => {
            Some("the body can be stopped by a machine that has gone wrong")
        }
        _ => None,
    }
}

const A_CHANNEL: &str = "the source is a channel, which ends when a machine dies";

// A receiver handed to the `for` itself names no call of its own -- the
// lowering's `into_iter` is the trait's -- so its type is what is read.
fn is_a_receiver(cx: &LateContext<'_>, held: Ty<'_>) -> bool {
    match held.peel_refs().kind() {
        ty::Adt(adt, _) => cx.tcx.def_path_str(adt.did()).ends_with("mpsc::Receiver"),
        _ => false,
    }
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

// What the loop holds: the reason it is a task, and whether it kicks.
struct Asked<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    why: Option<&'static str>,
    kicks: bool,
}

impl<'a, 'tcx> Visitor<'tcx> for Asked<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if let Some(named) = what_is_called(self.cx, expr) {
            let last = named.rsplit("::").next().unwrap_or_default();

            if matches!(last, "watchdog" | "feed" | "fed" | "kick") || named.contains("watchdog") {
                self.kicks = true;
            }

            if self.why.is_none() {
                self.why = is_a_task(named.as_str());
            }
        }

        intravisit::walk_expr(self, expr);
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit079WatchdogEveryLoop {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if !is_flight(cx) {
            return;
        }

        // A `for` is sugar the compiler writes, so the loop is read back out
        // of its lowering: what it walks and what its body does, and nothing
        // written beside it in the block it sits in.
        let Some(walked) = ForLoop::hir(expr) else {
            return;
        };

        let mut asked = Asked {
            cx,
            why: None,
            kicks: false,
        };

        if is_a_receiver(cx, cx.typeck_results().expr_ty(walked.arg)) {
            asked.why = Some(A_CHANNEL);
        }

        asked.visit_expr(walked.arg);
        asked.visit_expr(walked.body);

        let Some(why) = asked.why else {
            return;
        };

        if asked.kicks {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT079_WATCHDOG_EVERY_LOOP,
            walked.span,
            format!("this loop is a task -- {why} -- and kicks no watchdog"),
            None,
            "call the watchdog inside the loop, so a hang is discovered by the machine and \
             answered with a reboot rather than discovered by whoever is holding the device. \
             Where the loop really is short and cannot hang, allow this rule here and let the \
             reason say what ends it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "console_kernel_case");
}
