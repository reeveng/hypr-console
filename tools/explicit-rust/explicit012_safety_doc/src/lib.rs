#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Block, BlockCheckMode};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT012: every `unsafe { … }` must carry a `// SAFETY:` comment on the
    /// line above.
    pub EXPLICIT012_SAFETY_DOC,
    Deny,
    "`unsafe { … }` must be preceded by a `// SAFETY: …` comment naming the invariant"
}

// `declare_late_lint!` already creates the unit struct `Explicit012SafetyDoc`
// and implements `LintPass` on it. We just add the `LateLintPass` impl.

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic nobody ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit012SafetyDoc {
    fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx Block<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        if !matches!(block.rules, BlockCheckMode::UnsafeBlock(_)) {
            return;
        }
        // An `unsafe` block written by a macro is the macro author's to
        // justify, and the comment would have to live in their crate. Only
        // what somebody wrote here is asked for a reason.
        if block.span.from_expansion() {
            return;
        }
        let sm = cx.tcx.sess.source_map();
        let loc = sm.lookup_char_pos(block.span.lo());
        // The whole comment block above, not only the line touching the
        // `unsafe`. A reason worth writing is often two or three lines, and
        // the word only appears on the first of them -- so reading one line
        // would fail exactly the blocks that were explained most carefully.
        //
        // `Loc::line` counts from one and `get_line` counts from zero, so the
        // line above line `n` is `n - 2`. Reading `n - 1` is the block's own
        // line, which inverts the rule: it passes an `unsafe` that says SAFETY
        // on its own line and fails every one that says it above. `checked_sub`
        // is what answers a block at the top of a file, with nothing above it.
        let mut said_safety = false;
        let mut above = loc.line;
        while let Some(index) = above.checked_sub(2)
            && let Some(line) = loc.file.get_line(index)
        {
            let line = line.trim();
            if !line.starts_with("//") {
                break;
            }

            if line.contains("SAFETY:") {
                said_safety = true;
                break;
            }

            above -= 1;
        }
        if !said_safety {
            span_lint_and_help(
                cx,
                EXPLICIT012_SAFETY_DOC,
                block.span,
                "`unsafe { … }` must be preceded by a `// SAFETY: …` comment",
                None,
                "write `// SAFETY: <invariant relied on>` on the line above the block",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}