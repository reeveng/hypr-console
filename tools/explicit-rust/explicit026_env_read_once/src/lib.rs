//! EXPLICIT026: an environment variable is read in one place, and the place is
//! the crate that understands what it means.
//!
//! `console-core-places` exists because two crates read `HOME` and both
//! answered `/root` when it was unset, which succeeds against the wrong
//! person's dotfiles rather than failing. That was fixed once, in one place,
//! for one variable, and the rule it implies was never written down.
//!
//! It is the format rule this workspace already keeps, said about ambient
//! state rather than about file syntax. A `.desktop` file is read in
//! `console-applications` and `desktop.conf` in `console-manifest-engine`
//! because a second reading of one thing drifts quietly, and drift surfaces as
//! a feature behaving differently in two places rather than as an argument in
//! the code. A name read from the environment is the same shape with less
//! ceremony around it: nothing declares it, nothing types it, and the second
//! caller is always the one that is guessing.
//!
//! So the call is denied everywhere and the crate that owns the answer carries
//! the allow, with EXPLICIT018 asking it for a sentence. The set of owning
//! crates is then greppable, which a list inside this lint would not be -- and
//! a second crate wanting the same name has to come and take the allow off the
//! first one, which is the argument this rule exists to force.
//!
//! What it stops at is what is read from outside the process. `env!` and
//! `option_env!` are compile-time and are not ambient at all: what they read is
//! this tree at the moment it was compiled. Process globals are left alone for
//! a different reason -- a `thread_local!` in a panel is there because glib's
//! main context is per-thread and what is held is not `Send`, and a rule whose
//! every violation carries the same sentence is paperwork rather than a rule.
//!
//! One level up it is `os_design.md`'s I1: a context can name only what it was
//! handed, and no ambient name resolves to authority. A tree that reads `HOME`
//! in a dozen crates is practising the opposite of what the machine is meant to
//! promise, and this is the cheapest place to find out what that costs.

//!
//! It arrived `Warn`. What it bites is not small -- a name from the environment
//! is read in most of the crates that draw anything, and some names are read in
//! more than one of them. Most of those readings are probably right and merely
//! homeless: a session variable the compositor set, `CONSOLE_HOST`, an XDG
//! path. Each one is answered by moving it to a crate that can say what an
//! absent one means, or by leaving it where it is under an allow that says this
//! is the place. `just explicit` prints what is left.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT026: reading an environment variable is denied outside the
    /// crate that owns what the name means. Two crates reading `HOME` is two
    /// answers to one question, and the day they differ nothing says so. The
    /// owning crate carries the allow and its reason names the variable.
    pub EXPLICIT026_ENV_READ_ONCE,
    Deny,
    "an environment variable read outside the crate that owns what it means"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The four ways the process asks its environment what it holds. Read off the
// resolved path rather than the spelling, so a `use std::env::var` does not
// slip past.
fn asks_the_environment(path: &str) -> Option<&'static str> {
    match path {
        "std::env::var" | "std::env::var_os" => {
            Some("one name, answered here and possibly answered differently somewhere else")
        }
        "std::env::vars" | "std::env::vars_os" => {
            Some("every name at once, which is every one of them read in a place that owns none")
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit026EnvReadOnce {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };

        let ExprKind::Path(QPath::Resolved(_, path)) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = path.res else {
            return;
        };

        let named = cx.tcx.def_path_str(id);

        let Some(harm) = asks_the_environment(named.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT026_ENV_READ_ONCE,
            expr.span,
            format!("`{named}` reads {harm}"),
            None,
            "a name from the environment is read in one crate -- the one that knows what the name means \
             and what an absent one implies -- and handed on from there. `console_core_places` is that \
             crate for `HOME`, and its `home` is an `Option` because the alternative was two crates \
             answering `/root`. Where this crate is the owner, allow this rule here and let the reason \
             name the variable",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
