//! EXPLICIT092: root is given up in one order, and all of it at once.
//!
//! A process that stops being root has three things to put down, and the
//! kernel only lets it put them down while it is still root. The
//! supplementary groups go first, then the group, then the user. Written the
//! other way round, `setuid` succeeds, the process is no longer root, and the
//! `setgid` and the `setgroups` after it are refused -- so the session that
//! starts is the person's own, still in root's group and still a member of
//! every group root was in, which is `disk` and `wheel` and whatever else the
//! machine has. Left out, the same. Nothing fails that anybody sees: the
//! login works, the desktop comes up, and the fault is a set of permissions
//! on a process nobody thinks to list.
//!
//! EXPLICIT005 already makes each of the three answers be met, which is the
//! other half of the old fault -- a `setuid` that failed and was not looked
//! at. This rule holds the order, in the one place it can be read: the
//! function that drops. A `setuid` in a body with no `setgid` before it, or a
//! `setgid` with no `setgroups` or `initgroups` before it, is the sequence
//! with a step missing or out of turn. It reads the names whoever declares
//! them, because `console-login-window` declares glibc's three itself and
//! `rustix` calls them `set_thread_*`.
//!
//! A type would say this better -- root, then without its groups, then
//! without its group, each made only from the one before, which is
//! EXPLICIT076's shape -- and the day one is written the rule points at it.
//! Until then it is the order, checked.
//!
//! It arrived `Warn` expecting to find nothing, found nothing, and is denied.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT092: in the function that gives root up, the groups go, then
    /// the group, then the user. A `setuid` with no `setgid` before it, or a
    /// `setgid` with no `setgroups` before it, keeps what it skipped.
    pub EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER,
    Deny,
    "root given up out of order keeps the group and the groups it meant to put down"
}

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Groups,
    Group,
    User,
}

fn which_step(named: &str) -> Option<Step> {
    match named {
        "setgroups" | "initgroups" | "set_thread_groups" => Some(Step::Groups),
        "setgid" | "setegid" | "setresgid" | "set_thread_gid" | "set_thread_res_gid" => Some(Step::Group),
        "setuid" | "seteuid" | "setresuid" | "set_thread_uid" | "set_thread_res_uid" => Some(Step::User),
        _ => None,
    }
}

struct Steps<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'a TypeckResults<'tcx>,
    taken: Vec<(Step, String, Span)>,
}

impl<'tcx> Visitor<'tcx> for Steps<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        // The arguments are walked first: a step taken inside another call's
        // argument is taken before that call is.
        intravisit::walk_expr(self, expr);

        let id = match expr.kind {
            ExprKind::Call(called, _) => match called.kind {
                ExprKind::Path(ref qpath) => match self.typeck.qpath_res(qpath, called.hir_id) {
                    Res::Def(DefKind::Fn | DefKind::AssocFn, id) => Some(id),
                    _ => None,
                },
                _ => None,
            },
            ExprKind::MethodCall(..) => self.typeck.type_dependent_def_id(expr.hir_id),
            _ => None,
        };

        let Some(id) = id else {
            return;
        };

        let named = self.tcx.item_name(id);

        if let Some(step) = which_step(named.as_str()) {
            self.taken.push((step, named.to_string(), expr.span));
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit092PrivilegeDroppedInOrder {
    fn check_body(&mut self, cx: &LateContext<'tcx>, body: &Body<'tcx>) {
        let mut steps = Steps { tcx: cx.tcx, typeck: cx.tcx.typeck_body(body.id()), taken: Vec::new() };

        steps.visit_expr(body.value);

        for (at, (step, named, span)) in steps.taken.iter().enumerate() {
            let before = steps.taken.get(..at).unwrap_or_default();
            let came = |wanted: Step| before.iter().any(|(earlier, _, _)| *earlier == wanted);

            let missing = match step {
                Step::Groups => continue,
                Step::Group if came(Step::Groups) => continue,
                Step::Group => "the supplementary groups are still root's: nothing before it calls `setgroups` or `initgroups`",
                Step::User if came(Step::Groups) && came(Step::Group) => continue,
                Step::User if came(Step::Groups) => "the group is still root's: nothing before it calls `setgid`, and after it nothing may",
                Step::User => "the group and the groups are still root's: nothing before it puts them down, and after it nothing may",
            };

            span_lint_and_help(
                cx,
                EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER,
                *span,
                format!("`{named}` here, and {missing}"),
                None,
                "give root up in the one order the kernel allows, in one function: the groups, the \
                 group, then the user, each answer met. Where a step really was taken somewhere \
                 else first, allow this rule here and let the reason say where",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
