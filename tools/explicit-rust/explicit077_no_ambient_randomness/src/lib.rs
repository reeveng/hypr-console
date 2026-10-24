//! EXPLICIT077: a run can be asked twice, and answered the same.
//!
//! EXPLICIT039 took the clock: a reading nothing handed in cannot be
//! depended on to repeat. Randomness is the same fault one dimension further
//! -- `thread_rng` and its kin are an input no caller named, an input that
//! cannot even be *asked* for its value, and a program that decides from
//! them is a program whose decisions cannot be replayed. When such a program
//! goes wrong, the fault cannot be reproduced, because the input that made
//! it has gone and cannot be read again.
//!
//! The spelling this rule asks for is the one the whole rest of this suite
//! already asks: hand it in. A seed is an input like any other -- read at the
//! edge, where the clock and `CONSOLE_HOST` are read, and passed inward --
//! and then a run is replayable exactly: pressed a second time with the same
//! seed, it answers with the same decisions, which is the property that
//! makes a check out of a fault. The wall clock is caught for the same
//! reason libraries are held to it by EXPLICIT039: `SystemTime` is an input
//! the machine's owner moves.
//!
//! It arrives `Warn`, over a tree that has no randomness in it, which is the
//! distance this rule holds at zero: the first `thread_rng` somebody writes
//! will have to argue with it on the day it is written.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_session::config::CrateType;

dylint_linting::declare_late_lint! {
    /// EXPLICIT077: `thread_rng`, `random`, `getrandom` and the wall clock in
    /// a library are inputs no caller named, and a decision read from them
    /// cannot be replayed. Hand in the seed and the instant.
    pub EXPLICIT077_NO_AMBIENT_RANDOMNESS,
    Warn,
    "a decision read from a source nobody handed in; hand in the seed and the instant"
}

// The edge is the binary, the same line EXPLICIT039 draws: a program's `main`
// is where a machine's answers become somebody's parameters.
fn is_the_edge(cx: &LateContext<'_>) -> bool {
    cx.tcx
        .crate_types()
        .iter()
        .any(|kind| matches!(kind, CrateType::Executable))
}

// Both families, read off the resolved path so a re-export does not slip
// past.
fn from_nowhere(path: &str) -> Option<&'static str> {
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "thread_rng" | "random" if path.contains("rand") => Some(
            "the generator is an input no caller named, and a decision read from it cannot be \
             replayed",
        ),
        "getrandom" => Some("the bytes are an input no caller named, and cannot be read again"),
        "now" if path.ends_with("SystemTime::now") => Some(
            "the wall clock is moved by the machine's owner, and is not an answer to depend on",
        ),
        _ => None,
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

impl<'tcx> LateLintPass<'tcx> for Explicit077NoAmbientRandomness {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        let Some(harm) = from_nowhere(named.as_str()) else {
            return;
        };

        // The wall clock belongs to the edge, the way EXPLICIT039 allows it
        // there; the generator belongs to no library, so no binary gets that
        // door and has to hand in the seed.
        if named.ends_with("SystemTime::now") && is_the_edge(cx) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT077_NO_AMBIENT_RANDOMNESS,
            expr.span,
            format!("`{named}`: {harm}"),
            None,
            "read the seed at the edge and pass it inward, so a run can be replayed exactly -- \
             pressed a second time with the same seed, it answers with the same decisions, which \
             is the property that turns a fault into a check",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
