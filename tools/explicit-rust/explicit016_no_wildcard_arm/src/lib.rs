#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, MatchSource, Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT016: a `match` over an enum may not have a wildcard arm. `_ =>`
    /// is a decision made by omission -- add a variant next year and every
    /// catch-all votes on it without the compiler asking. Named variants are
    /// how a new variant becomes a build error at every site that has an
    /// opinion about it.
    ///
    /// A foreign `#[non_exhaustive]` enum was exempt here once, because the
    /// compiler demands a catch-all over one and a rule cannot ask for what
    /// will not compile. It asks for something else instead: do not match on
    /// that enum. The question a catch-all over `io::ErrorKind` is really
    /// asking is whether the kind is one particular kind, and that is an
    /// answer with two named outcomes -- `match fault.kind() == NotFound
    /// { true => …, false => … }`. What is left unwritable is a wide reading
    /// of somebody else's open enum, which is the thing that quietly votes on
    /// a variant added next year.
    ///
    /// A guarded arm was left alone here once, on the argument that the
    /// unguarded arm it falls through to answers for the rest. EXPLICIT019 now
    /// denies the guard itself, so there is nothing left to exempt.
    pub EXPLICIT016_NO_WILDCARD_ARM,
    Deny,
    "a wildcard arm on an enum decides future variants by omission; name every variant"
}

// Tests are exempt. A test that panics is a test that fails, which is what a
// test is for, and `as` in a fixture is arithmetic no one ships. `opts.test`
// is true only for the harness build of a target -- the ordinary build of the
// same library is linted as production, so nothing real is lost by skipping
// this one.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A pattern that matches every variant without naming one: `_`, a bare
// binding, or an or-pattern with either inside it.
fn swallows_every_variant(pat: &Pat<'_>) -> bool {
    match pat.kind {
        PatKind::Wild => true,
        PatKind::Binding(_, _, _, None) => true,
        PatKind::Or(alternatives) => alternatives.iter().any(swallows_every_variant),
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit016NoWildcardArm {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = expr.kind else {
            return;
        };

        if expr.span.from_expansion() {
            return;
        }

        let scrutinee_ty = cx.typeck_results().expr_ty(scrutinee).peel_refs();

        let rustc_middle::ty::Adt(adt, _) = scrutinee_ty.kind() else {
            return;
        };

        if !adt.is_enum() {
            return;
        }

        for arm in arms {
            if swallows_every_variant(arm.pat) {
                span_lint_and_help(
                    cx,
                    EXPLICIT016_NO_WILDCARD_ARM,
                    arm.pat.span,
                    "this arm matches every variant of the enum without naming one",
                    None,
                    "name every variant, so a variant added later is a compile error here rather than a silent vote -- and over somebody else's open enum, ask the question instead: `match value == Wanted { true => …, false => … }`",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
