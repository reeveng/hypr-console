//! EXPLICIT039: the clock is read at the edge of a program and handed inward.
//!
//! EXPLICIT021 denies waiting on the clock and says why: a duration is an
//! assumption about a machine. This is the half of that argument the rule left
//! out. A function that asks what time it is has an input nothing handed it
//! and cannot be asked the same question twice: pressed a second time it
//! answers differently, and there is no arrangement of the test that makes it
//! answer the same. `console-input-controller` is the shape the rest of the
//! tree is walking toward for exactly this reason -- input is handed in and a
//! `Effect` is handed back, so every decision can be asked twice and answered
//! the same way -- and a `Instant::now()` inside one of those decisions is the
//! one thing that takes it back.
//!
//! The tree already writes it the right way in the places that were hard
//! enough to make someone think about it. `console-cpu-boost` decides nothing
//! from the clock: `hurrying.asked(now)` takes the instant as a parameter, and
//! the binary reads it. `console-program-runtime` does the same across its
//! loop. What this rule does is say that out loud and stop the other spelling,
//! which is in most of the crates that draw anything and was never a decision
//! anywhere.
//!
//! So: a library may not ask. A binary may, because a binary is where the
//! program meets the machine it is running on -- it is the same boundary
//! `console-core-places` draws for `HOME` and `console-core-external-programs`
//! draws for a program's name, and the reason it is drawn at the crate type
//! rather than at a list of function names is that the list would be every
//! `main` in the tree and would say nothing.
//!
//! An elapsed duration is not exempt and is where most of the sites are: a
//! `began` at the top of a function and an `elapsed()` at the bottom is two
//! readings of a clock and a subtraction, and it belongs to whoever is timing
//! the thing rather than to the thing. Hand in a `Since` and measure around
//! the call.
//!
//! What it does not touch is a time that came from somewhere else -- a
//! modification time read off a file, an instant handed in as a parameter,
//! `Duration` arithmetic on either. Those are quantities, and EXPLICIT024 has
//! already asked them to have names.
//!
//! It arrived `Warn` and is denied. What it bit was spread over most of the
//! crates that draw anything, and it came out in two kinds. Where someone at
//! the edge could hold the answer the reading moved out and became a parameter:
//! the virtual keyboard's timestamp base is read by `keyboard` and handed to
//! `Typist::new`, and where the machine is was a memo in the wallpaper's own
//! crate and is now a `Kept` the binary holds and hands an instant to. The rest
//! is the crates that *are* clocks -- the wait, the stopwatch that writes down
//! how long a person waited, the bar's own minute, the runtime's loop, the
//! watcher's cadence -- and each of those carries the allow with a sentence
//! saying that the reading is the answer rather than a decision taken from
//! one.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::config::CrateType;

dylint_linting::declare_late_lint! {
    /// EXPLICIT039: reading the clock inside a library is an input nothing
    /// handed in, and a decision that reads it cannot be asked twice and
    /// answered the same way. The binary reads the clock and passes the
    /// instant inward.
    pub EXPLICIT039_NO_READING_THE_CLOCK,
    Deny,
    "a clock read inside a library; the instant belongs to the caller"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The edge of a program is its binary. A crate compiled as an executable is
// where this desktop meets the machine it is running on, and is the one place
// the ambient answer can be turned into a parameter.
fn is_the_edge(cx: &LateContext<'_>) -> bool {
    cx.tcx
        .crate_types()
        .iter()
        .any(|kind| matches!(kind, CrateType::Executable))
}

// Both clocks, named by resolved path so a `use` of either does not slip past.
// They fail differently and it is worth saying which: the monotonic one cannot
// be asked the same question twice, and the wall one cannot even be relied on
// to go forwards.
fn asks_the_clock(path: &str) -> Option<&'static str> {
    match path {
        "std::time::Instant::now" => {
            Some("this answers differently every time it is asked, and nothing handed it in")
        }
        "std::time::SystemTime::now" => {
            Some("this is the wall clock, which a machine is free to move in either direction")
        }
        _ => None,
    }
}

// What a call resolves to, whether it was written as a path or as a method.
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

impl<'tcx> LateLintPass<'tcx> for Explicit039NoReadingTheClock {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if is_the_edge(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        let Some(harm) = asks_the_clock(named.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT039_NO_READING_THE_CLOCK,
            expr.span,
            format!("`{named}` reads the clock: {harm}"),
            None,
            "take the instant as a parameter and let whoever is at the edge read it, the way \
             `console_cpu_boost`'s `asked(now)` does and its binary does the reading. A decision that \
             reads its own clock is one that cannot be pressed twice and answered the same way, which is \
             the shape `console_input_controller` is written in. Where this crate is the edge -- a \
             binary, or a wait that is measuring itself -- allow this rule here and let the reason say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
