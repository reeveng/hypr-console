//! EXPLICIT075: a decision fits in a head.
//!
//! EXPLICIT050 holds a function to a screen: its two ends have to be visible
//! together. This rule holds its *inside* to a head: nesting past a handful
//! of decisions is a function whose shape nobody can hold -- every level of
//! it is a promise about what is true once you are inside, and past a few
//! the promises are the only thing a reader is tracking, not the code. The
//! same is true of a signature with a crowd of parameters: each parameter is
//! a thing the call site must get right in one order, and a crowd is a
//! call site nobody can write without counting on their fingers -- which is
//! EXPLICIT024's fault, arrived at by the width instead of by the pair.
//!
//! The answer in both directions is the one the tree already gives: a nested
//! decision names its levels -- each `match` inside becomes a function whose
//! name says which case it is -- and a crowd of parameters is one value with
//! many parts (`console-core-geometry` again) or a struct of the things that
//! travel together, which is also the door EXPLICIT048 asks a state to go
//! through.
//!
//! What the rule counts is what a reader counts: a `match`, a loop, a block,
//! each one a level deeper than the last. A `for` is one level, its braces
//! included, the way a `loop`'s are: the compiler lowers it into a `match`, a
//! `loop` and a second `match` around the body, and none of those three is a
//! thing anybody wrote or reads. The `match` a `?` lowers into is the same --
//! a level nobody climbs down -- and so is anything a macro wrote. Six levels is where the sixth is
//! no longer a thing a head holds, so that is the wall; eight parameters is
//! where the order stops being visible at the call site, so that is the
//! other. Both numbers are arguments rather than laws, and the allow at a
//! site is the disagreement, in writing.
//!
//! It arrives `Warn`, printing the deepest and the widest the tree has.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::higher::ForLoop;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, ExprKind, FnDecl, MatchSource};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT075: past six nested decisions or eight parameters, a function
    /// is a shape no reader holds. Name the levels as functions; make the
    /// crowd one value with parts.
    pub EXPLICIT075_BOUNDED_COMPLEXITY,
    Warn,
    "nesting past six levels or parameters past eight; the shape a head cannot hold"
}

const DEPTH_HELD: u32 = 6;
const PARAMETERS_HELD: usize = 8;

// Each of these is a level a reader goes down, and every one of them is a
// promise about what holds inside it.
struct Depth {
    now: u32,
    deepest: u32,
    where_deepest: Option<Span>,
}

impl Depth {
    fn down(&mut self, at: Span) {
        self.now = self.now.saturating_add(1);

        if self.now > self.deepest {
            self.deepest = self.now;
            self.where_deepest = Some(at);
        }
    }

    fn up(&mut self) {
        self.now = self.now.saturating_sub(1);
    }
}

impl<'tcx> Visitor<'tcx> for Depth {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        // A `for` is read as the one loop it is written as. What it walks is
        // asked before the loop is entered, and its body's braces are the
        // loop's own, as a `loop`'s block is.
        if let Some(walked) = ForLoop::hir(expr) {
            self.visit_expr(walked.arg);
            self.down(walked.span);

            match walked.body.kind {
                ExprKind::Block(block, _) => intravisit::walk_block(self, block),
                _ => self.visit_expr(walked.body),
            }

            self.up();
            return;
        }

        let a_level = match expr.kind {
            ExprKind::Match(_, _, MatchSource::Normal) | ExprKind::Block(..) => {
                !expr.span.from_expansion()
            }
            ExprKind::Loop(..) => true,
            _ => false,
        };

        match a_level {
            true => {
                self.down(expr.span);
                intravisit::walk_expr(self, expr);
                self.up();
            }
            false => intravisit::walk_expr(self, expr),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit075BoundedComplexity {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: rustc_span::Span,
        _def_id: LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) {
            return;
        }

        if span.from_expansion() {
            return;
        }

        // A receiver is not a parameter anybody passes.
        let passed = match decl.implicit_self() {
            rustc_hir::ImplicitSelfKind::None => decl.inputs.len(),
            _ => decl.inputs.len().saturating_sub(1),
        };

        if passed > PARAMETERS_HELD {
            span_lint_and_help(
                cx,
                EXPLICIT075_BOUNDED_COMPLEXITY,
                span,
                format!(
                    "this function takes {passed} parameters, past the {PARAMETERS_HELD} a call \
                     site can keep in order"
                ),
                None,
                "make the crowd one value with parts -- a struct of the things that travel \
                 together, the shape `console-core-geometry` gives a place and a size -- or allow \
                 this rule here and let the reason say why the order is visible at the call site",
            );
        }

        let mut depth = Depth {
            now: 0,
            deepest: 0,
            where_deepest: None,
        };

        depth.visit_expr(body.value);

        if depth.deepest > DEPTH_HELD {
            if let Some(where_deepest) = depth.where_deepest {
                span_lint_and_help(
                    cx,
                    EXPLICIT075_BOUNDED_COMPLEXITY,
                    where_deepest,
                    format!(
                        "this function nests {deepest} levels of decisions, past the \
                         {DEPTH_HELD} a head holds",
                        deepest = depth.deepest,
                    ),
                    None,
                    "name the levels: each `match` inside becomes a function whose name says \
                     which case it is, so a reader takes the cases one at a time instead of \
                     holding six promises",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
