//! EXPLICIT050: a function fits on a screen.
//!
//! TigerStyle keeps a function to what can be seen at once, for the reason
//! EXPLICIT013 asks for blank lines: the shape of a function should be visible
//! before it is read. 013 is the one rule here that looks at the screen rather
//! than at types, and this is its other half -- a body whose two ends cannot
//! both be seen has no shape to show, and the reader holds the top of it in
//! their head while scrolling to the bottom.
//!
//! What is counted is the lines of the body that hold something, outside the
//! arms of a `match`. A blank line is the room 013 asked for and is not charged
//! for twice. An arm is one case of a decision spelled once per case, which is
//! what 016 and 019 asked for, so a `match` over an enum with many variants is
//! as long as the enum is and says nothing about the function around it. That
//! is a ceiling as well as an exemption: a body written as one long arm is not
//! seen by this rule, and that shape is one EXPLICIT019 already makes awkward.
//!
//! The limit is TigerStyle's own. A long function is nearly always a sequence
//! rather than a decision -- an apply, a deploy, a surface being built -- and
//! what comes out of the rule is named stages, not smaller pieces of the same
//! walk. A closure is part of the function it is written in and is counted
//! there.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use std::collections::BTreeSet;
use std::ops::ControlFlow;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::visitors::for_each_expr;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, ExprKind, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

dylint_linting::declare_late_lint! {
    /// EXPLICIT050: a function longer than a screen has no shape a reader can
    /// see at once. Name its stages as functions of their own.
    pub EXPLICIT050_NO_LONG_FUNCTION,
    Deny,
    "a function longer than a screen; name its stages"
}

const SCREEN: usize = 70;

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Every line a `match` arm stands on, the pattern with the body, so that a
// decision is charged for the lines around it and not for its cases. A `?`
// and a `for` are a `match` once they are lowered, and neither is an arm
// anyone wrote.
fn lines_in_arms<'tcx>(cx: &LateContext<'tcx>, body: &Body<'tcx>) -> BTreeSet<usize> {
    let source = cx.sess().source_map();
    let mut lines = BTreeSet::new();

    let _ = for_each_expr(cx, body.value, |expr| {
        if let ExprKind::Match(_, arms, _) = expr.kind {
            for arm in arms.iter().filter(|arm| !arm.span.from_expansion()) {
                let first = source.lookup_char_pos(arm.span.lo()).line;
                let last = source.lookup_char_pos(arm.span.hi()).line;

                lines.extend(first..=last);
            }
        }

        ControlFlow::<()>::Continue(())
    });

    lines
}

fn lines_written<'tcx>(cx: &LateContext<'tcx>, body: &Body<'tcx>) -> Option<usize> {
    let source = cx.sess().source_map();
    let span = body.value.span;
    let written = source.span_to_snippet(span).ok()?;
    let first = source.lookup_char_pos(span.lo()).line;
    let in_arms = lines_in_arms(cx, body);

    let counted = written
        .lines()
        .enumerate()
        .filter(|(offset, line)| !line.trim().is_empty() && !in_arms.contains(&(first + offset)))
        .count();

    Some(counted)
}

impl<'tcx> LateLintPass<'tcx> for Explicit050NoLongFunction {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        if is_test_build(cx) {
            return;
        }

        if span.from_expansion() {
            return;
        }

        if let FnKind::Closure = kind {
            return;
        }

        let Some(counted) = lines_written(cx, body) else {
            return;
        };

        if counted <= SCREEN {
            return;
        }

        let name = cx.tcx.item_name(def_id.to_def_id());

        span_lint_and_help(
            cx,
            EXPLICIT050_NO_LONG_FUNCTION,
            cx.tcx.def_span(def_id),
            format!("`{name}` is {counted} lines outside its `match` arms, and a screen holds {SCREEN}"),
            None,
            "name its stages. A body whose two ends cannot both be seen is read by holding the top of it \
             in mind while scrolling to the bottom, and a long one is nearly always a sequence rather \
             than a decision: each step as a function of its own, called in order from here, is a list \
             a reader can take in at once",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
