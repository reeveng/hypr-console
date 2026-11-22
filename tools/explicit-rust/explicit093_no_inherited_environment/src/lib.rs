//! EXPLICIT093: a program started by root is told its environment.
//!
//! A child is born with its parent's environment unless somebody says
//! otherwise, and an environment is a list of instructions: `PATH` decides
//! which file a bare name means, `LD_PRELOAD` and `LD_LIBRARY_PATH` decide
//! what code is loaded before `main`, `IFS` and `BASH_ENV` decide what a
//! shell does with a line that looked harmless. For a program started by the
//! person whose environment it is, that is the point. For one started by
//! root it is the other way about -- whatever reached root's environment,
//! from a unit file, a `sudo` that kept more than it should, a session that
//! was somebody's, is now deciding what a privileged program runs. It is
//! EXPLICIT044's argument at the one boundary where it is a door: an input
//! nobody handed in, read by the program with the most to lose from it.
//!
//! So a crate that runs as root starts nothing on what it happened to
//! inherit. `env_clear` first, then the variables the child is meant to have,
//! each named at the site -- which is also where a reader finds out what the
//! child depends on, and is what `login-window` already does for the session
//! it starts.
//!
//! The rule reads one function: a `spawn`, a `status` or an `output` on a
//! `Command` in a body that never says `env_clear`. A command built in one
//! function and started in another is one it cannot follow, and that site
//! says where the clearing was done. It is asked of the crates that run as
//! root, and its case is built as one.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT093: in a crate that runs as root, a `Command` is started
    /// only after `env_clear`, with what the child is meant to have named
    /// beside it.
    pub EXPLICIT093_NO_INHERITED_ENVIRONMENT,
    Warn,
    "a program started by root on an inherited environment takes its instructions from whoever set it"
}

fn runs_as_root(cx: &LateContext<'_>) -> bool {
    let named = cx.tcx.crate_name(LOCAL_CRATE);
    let named = named.as_str();

    named.starts_with("console_manifest_engine") || named.starts_with("console_login")
}

struct Started<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'a TypeckResults<'tcx>,
    cleared: bool,
    starts: Vec<(String, Span)>,
}

impl<'tcx> Visitor<'tcx> for Started<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        intravisit::walk_expr(self, expr);

        let ExprKind::MethodCall(..) = expr.kind else {
            return;
        };

        let Some(id) = self.typeck.type_dependent_def_id(expr.hir_id) else {
            return;
        };

        match self.tcx.def_path_str(id).as_str() {
            "std::process::Command::env_clear" => self.cleared = true,
            named @ ("std::process::Command::spawn"
            | "std::process::Command::status"
            | "std::process::Command::output") => {
                if !expr.span.from_expansion() {
                    self.starts.push((named.to_string(), expr.span));
                }
            }
            _ => {}
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit093NoInheritedEnvironment {
    fn check_body(&mut self, cx: &LateContext<'tcx>, body: &Body<'tcx>) {
        if !runs_as_root(cx) {
            return;
        }

        let mut started =
            Started { tcx: cx.tcx, typeck: cx.tcx.typeck_body(body.id()), cleared: false, starts: Vec::new() };

        started.visit_expr(body.value);

        if started.cleared {
            return;
        }

        for (named, span) in started.starts {
            span_lint_and_help(
                cx,
                EXPLICIT093_NO_INHERITED_ENVIRONMENT,
                span,
                format!("`{named}` starts a program as root on whatever environment this process was handed"),
                None,
                "say `env_clear()` and then name each variable the child is meant to have, `PATH` \
                 among them. Where the command was cleared in the function that built it, allow this \
                 rule here and let the reason say which",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "console_manifest_engine_case");
}
