//! EXPLICIT031: a list is walked by its own iterator, not by counting.
//!
//! `for at in 0..held.len()` is a loop about a number that is standing in for a
//! loop about a list. It says the wrong thing -- the subject is the index and
//! not the item -- and it can be wrong in a way `for item in held.iter()`
//! cannot: the bound is written by hand, so it can be `len()` of the wrong
//! list, or the same list before something was taken out of it, and every one
//! of those is a fault the compiler has nothing to say about.
//!
//! Stock clippy has `needless_range_loop` and it does not reach this. It fires
//! where the counter is used to index, and EXPLICIT014 has already denied
//! indexing everywhere in this tree -- so what the range loop turns into here
//! is `held.get(at)` and a `None` arm, which clippy reads as a counter used for
//! something and leaves alone. The `None` arm is the tell: it is a case that
//! cannot happen, written out to satisfy a rule, standing where the loop's own
//! bound should have made it unaskable.
//!
//! `iter()` says the same walk with the item as its subject, cannot run off the
//! end, and takes `enumerate` when the position is genuinely wanted alongside
//! the item. Where two lists are walked together, `zip` is the pair; where a
//! window over the list is wanted, `windows` and `chunks` are the shapes.
//!
//! A range over anything else is left alone. A range is a fine thing to walk
//! when the numbers are the subject -- a row of buttons to be laid out, a
//! countdown, a retry -- and this rule only asks about the one whose end is a
//! list's length, which is a range that was never about numbers at all.

//!
//! It arrived `Warn` with a short distance in front of it, which is what
//! EXPLICIT014 has already done to this shape: with indexing denied, counting to
//! a length costs a `get` and a `None` arm at every site, and most of the tree
//! had already decided that was not worth it.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::higher::ForLoop;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT031: `for at in 0..held.len()` walks a list by counting to its
    /// length. The subject is the number rather than the item, the bound is
    /// written by hand and can be the wrong list's, and EXPLICIT014 then asks
    /// for a `None` arm that cannot happen. `held.iter()` says the walk, and
    /// `enumerate` adds the position where it is really wanted.
    pub EXPLICIT031_NO_WALKING_BY_COUNT,
    Deny,
    "a list walked by counting to its length; walk it with `iter()`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A range whose far end is somebody's length. Both spellings, since `..=` is
// written by a different struct and is the one that is off by one as well.
fn ends_at_a_length<'tcx>(walked: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    let ends = match walked.kind {
        ExprKind::Struct(_, fields, _) => fields
            .iter()
            .find(|field| field.ident.name.as_str() == "end")
            .map(|field| field.expr),
        ExprKind::Call(_, made) => made.get(1),
        _ => None,
    }?;

    let ExprKind::MethodCall(asked, held, taken, _) = ends.kind else {
        return None;
    };

    if !taken.is_empty() {
        return None;
    }

    match asked.ident.name.as_str() {
        "len" => Some(held),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit031NoWalkingByCount {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        // A `for` loop is a `match` over `IntoIterator::into_iter` by the time
        // it reaches here, and this is what reads it back as the loop it was
        // written as.
        let Some(ForLoop { arg: walked, .. }) = ForLoop::hir(expr) else {
            return;
        };

        let Some(held) = ends_at_a_length(walked) else {
            return;
        };

        if held.span.from_expansion() {
            return;
        }

        // The loop's head carries the mark of the `for` desugaring, and a
        // diagnostic left on a desugared span is a diagnostic nobody is shown.
        // `source_callsite` is the range as somebody wrote it.
        span_lint_and_help(
            cx,
            EXPLICIT031_NO_WALKING_BY_COUNT,
            walked.span.source_callsite(),
            "this loop counts to a list's length instead of walking the list",
            None,
            "walk it with `iter()`, and take `enumerate()` where the position is wanted alongside the \
             item. A hand-written bound can be the wrong list's length or a length from before something \
             was taken out, and EXPLICIT014 then asks for a `None` arm that cannot happen",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
