#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT036: a program this desktop runs is named by a variant, not by a
    /// string spelled at the call.
    ///
    /// `console-core-external-programs` holds one variant per program this
    /// desktop runs and did not write, and an `Origin` beside each saying where
    /// it comes from -- which is what makes the list worth holding rather than
    /// a tidier way to spell the same words. A test crosses that list with the
    /// packages the manifest installs, so a program nobody arranged to be on
    /// the device fails here rather than on somebody's handheld.
    ///
    /// `Command::new("hyprctl")` is outside all of that. It runs a program the
    /// list has never heard of, on a machine nothing checked, and the fault it
    /// makes is the quietest kind: the desktop comes up, the feature is dead,
    /// and what is missing is a package nobody wrote down.
    ///
    /// A program of this tree's own is the same question asked the other way.
    /// It exists on the device only if the manifest names its binary, so a
    /// string here is a claim about the manifest that nothing checks -- and the
    /// four sites in this tree that spelled one were all of that kind.
    ///
    /// The rule is about a literal and nothing else. A name handed in, read
    /// from a file or taken off a variant is a name something else already
    /// answered for; this is the one spelling where the answer is nowhere.
    pub EXPLICIT036_NO_PROGRAM_BY_NAME,
    Deny,
    "a program named by a string literal is a program no list of what this desktop runs can see"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Asked of the resolved path rather than the spelling, so a `use
// std::process::Command as Run` does not slip past.
fn makes_a_command(cx: &LateContext<'_>, called: &Expr<'_>) -> bool {
    let ExprKind::Path(ref qpath) = called.kind else {
        return false;
    };

    let Res::Def(DefKind::AssocFn, id) = cx.typeck_results().qpath_res(qpath, called.hir_id) else {
        return false;
    };

    cx.tcx.def_path_str(id) == "std::process::Command::new"
}

fn is_a_spelled_name(expr: &Expr<'_>) -> Option<String> {
    let ExprKind::Lit(said) = expr.kind else {
        return None;
    };

    match said.node {
        rustc_ast::LitKind::Str(named, _) => Some(named.to_string()),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit036NoProgramByName {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        if expr.span.from_expansion() {
            return;
        }
        let ExprKind::Call(called, args) = expr.kind else {
            return;
        };
        if !makes_a_command(cx, called) {
            return;
        }
        let Some(named) = args.first().and_then(is_a_spelled_name) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT036_NO_PROGRAM_BY_NAME,
            expr.span,
            format!("`{named}` is spelled here, so nothing holds this desktop to having it"),
            None,
            "a program this desktop runs and did not write is a variant of \
             `console_core_external_programs::Program`, with the `Origin` that says where it comes from; \
             a program of our own is on the device only because the manifest names it. Either way the \
             name is answered for somewhere, and a literal here is answered for nowhere",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
