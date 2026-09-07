#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT022: a check waits for the thing, not for a number of seconds.
    /// EXPLICIT021 denies `thread::sleep`, and the tree answered it -- except
    /// where the sleep is behind a public function. `Device::settle` is one
    /// `thread::sleep` carrying one allow, and the allow's reason is true of
    /// exactly one caller: `Device::until`, which is the gap between two
    /// questions to the handheld. Every other caller is a check that named a
    /// number of seconds and asked nothing, and EXPLICIT021 cannot see any of
    /// them, because from where it stands there is one sleep in the tree and
    /// it is already excused.
    ///
    /// That is the shape this rule is really about: an allow excuses a site,
    /// and a `pub fn` around it turns one excused site into as many as anybody
    /// cares to write. So the rule is asked at the call, where the decision is
    /// actually made.
    ///
    /// What a call becomes instead is written in `docs/checks.md`: `drawn()`
    /// and `gone()` for a menu, and `until(what)` with the question spelled
    /// out for everything else. The last of them to cross were the home
    /// screen's, and what they were waiting on was a repaint: the check reads
    /// that by colour, and `lit()` answers `Result<_, String>` where `until`
    /// carried a question that could not fail. So `until` carries the fault
    /// its question carries now, and a reading of the screen is a wait like
    /// any other.
    ///
    /// What is left is the calls whose reason says the elapsing was itself
    /// what was asked for -- the gap between two questions, a d-pad held down
    /// so the highlight walks, and the two checks whose whole assertion is
    /// that nothing happened. They are why `settle` is still `pub`: all but
    /// the gap are written outside the crate that holds it, and each says at
    /// its own site why a number is the thing being named. That is the same
    /// place `console_waiting::between` stands.
    ///
    /// Tests are not exempt, unlike the rest of the suite. The checks that
    /// break this rule are written in test targets, and a rule that skipped
    /// them would be counting the wrong tree.
    pub EXPLICIT022_NO_SETTLING,
    Deny,
    "settling for a number of seconds on the device instead of asking it what happened"
}

// The path is read off the resolved method rather than off the spelling,
// because `settle` is a word four other types in this workspace use for four
// other things -- laying files down, a turn of the emulated daemon, a boost
// that has expired, a screen that has redrawn -- and none of those is a wait
// on a clock. It is matched by its tail so that the rule can be pressed in a
// `ui/` case, which has no console-test-stages to link against; the head of it
// is `console_test_stages`, and nothing else in this tree ends this way.
const A_NUMBER_OF_SECONDS: &str = "device::Device::settle";

impl<'tcx> LateLintPass<'tcx> for Explicit022NoSettling {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }
        let ExprKind::MethodCall(..) = expr.kind else {
            return;
        };
        let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
            return;
        };
        let named = cx.tcx.def_path_str(id);
        if !named.ends_with(A_NUMBER_OF_SECONDS) {
            return;
        }
        span_lint_and_help(
            cx,
            EXPLICIT022_NO_SETTLING,
            expr.span,
            format!("`{named}` waits out a number of seconds on the handheld and asks it nothing"),
            None,
            "the number is a guess about how long this device takes, made on the day the check was \
             written and relied on by every run since. Say what the check is actually waiting for: \
             `drawn()` for a menu that comes up, `gone()` for one that goes away, or `until(what)` \
             with the question written out. `settle` is the gap those put between two questions, \
             and it is theirs",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
