//! EXPLICIT105: a machine handler describes external work and returns.
//!
//! A handler owns the machine's state while it runs. Waiting for another
//! thread, a device, or a deadline keeps it from handling the next request.
//! Offer an effect instead and have the driver deliver the outcome as a request.
//! The pass follows statically resolved calls within the crate from
//! `Machine::initialize` and `Machine::handle`. It reports the call in the
//! machine and points to the blocking call when it is reached through a helper.
//! Closures can be handed to an effect runner, so the pass leaves their bodies
//! to the code that invokes them. The migration needs a decision about the
//! reply and what state to keep while waiting.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashSet;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, FnDecl, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT105: a machine handler submits effects and returns to its driver.
    pub EXPLICIT105_NO_BLOCKING_IN_HANDLER,
    Warn,
    "blocking call in an event handler"
}

fn migration(path: &str) -> Option<&'static str> {
    let (module, method) = path.rsplit_once("::")?;
    let module = module.split_once("::<").map_or(module, |(name, _)| name);
    let guidance = match (module, method) {
        ("std::thread", "sleep" | "sleep_until") => {
            "schedule `Subscription::Once` and continue on its `Event::Tick`"
        }
        ("std::process::Command", "status" | "output") => {
            "offer `Effect::Run` or `Effect::Fork` and handle the result in `Event::Replied`"
        }
        ("std::fs", "read" | "read_to_string" | "read_dir" | "metadata")
        | ("std::fs::File", "open") => {
            "offer a read effect and deliver the data or error as another request"
        }
        _ if blocking(module, method) => {
            "offer an effect for the driver to perform and handle its outcome as another request"
        }
        _ => return None,
    };

    Some(guidance)
}

fn blocking(module: &str, method: &str) -> bool {
    match module {
        "std::thread" => matches!(method, "park" | "park_timeout" | "scope"),
        "std::thread::JoinHandle" | "std::thread::ScopedJoinHandle" => method == "join",
        "std::sync::mpsc::Receiver" => matches!(method, "recv" | "recv_timeout" | "recv_deadline"),
        "std::sync::mpsc::SyncSender" => method == "send",
        "std::sync::Mutex" => method == "lock",
        "std::sync::RwLock" => matches!(method, "read" | "write"),
        "std::sync::Condvar" => matches!(
            method,
            "wait" | "wait_while" | "wait_timeout" | "wait_timeout_while"
        ),
        "std::sync::Barrier" => method == "wait",
        "std::fs" => method == "write",
        "std::fs::File" => matches!(method, "create" | "sync_all" | "sync_data"),
        "std::process::Child" => matches!(method, "wait" | "wait_with_output"),
        "console_actor::Actor" => matches!(method, "send" | "get"),
        "console_actor::Scope" => method == "close",
        "console_actor::Completions" | "console_actor::events::Completions" => {
            method == "recv_timeout"
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct Call {
    function: DefId,
    node: HirId,
    span: Span,
}

struct Calls<'tcx> {
    tcx: TyCtxt<'tcx>,
    types: &'tcx TypeckResults<'tcx>,
    found: Vec<Call>,
}

impl<'tcx> Visitor<'tcx> for Calls<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if expr.span.in_external_macro(self.tcx.sess.source_map()) {
            intravisit::walk_expr(self, expr);
            return;
        }

        let function = match expr.kind {
            ExprKind::MethodCall(..) => self.types.type_dependent_def_id(expr.hir_id),
            ExprKind::Call(callee, _) => match callee.kind {
                ExprKind::Path(ref path) => match self.types.qpath_res(path, callee.hir_id) {
                    Res::Def(DefKind::Fn | DefKind::AssocFn, function) => Some(function),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };

        if let Some(function) = function {
            self.found.push(Call { function, node: expr.hir_id, span: expr.span });
        }

        intravisit::walk_expr(self, expr);
    }
}

fn calls<'tcx>(tcx: TyCtxt<'tcx>, body: &'tcx Body<'tcx>) -> Vec<Call> {
    let mut calls = Calls { tcx, types: tcx.typeck_body(body.id()), found: Vec::new() };
    calls.visit_expr(body.value);
    calls.found
}

fn machine_method(cx: &LateContext<'_>, item: LocalDefId) -> Option<&'static str> {
    let implementation = cx.tcx.trait_impl_of_assoc(item.to_def_id())?;
    let machine = cx.tcx.impl_trait_ref(implementation).skip_binder().def_id;
    if cx.tcx.def_path_str(machine) != "console_core_state_machine::Machine" {
        return None;
    }

    match cx.tcx.item_name(item.to_def_id()).as_str() {
        "handle" => Some("an event handler"),
        "initialize" => Some("machine initialization"),
        _ => None,
    }
}

fn reached(tcx: TyCtxt<'_>, origin: Call, method: LocalDefId) -> Option<(Call, &'static str)> {
    let mut pending = vec![origin];
    let mut visited = HashSet::from([method]);

    while let Some(call) = pending.pop() {
        if let Some(help) = migration(&tcx.def_path_str(call.function)) {
            return Some((call, help));
        }

        if let Some(local) = call.function.as_local()
            && visited.insert(local)
            && let Some(body) = tcx.hir_maybe_body_owned_by(local)
        {
            pending.extend(calls(tcx, body).into_iter().rev());
        }
    }

    None
}

impl<'tcx> LateLintPass<'tcx> for Explicit105NoBlockingInHandler {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _kind: FnKind<'tcx>,
        _decl: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _span: Span,
        item: LocalDefId,
    ) {
        let Some(method) = machine_method(cx, item) else {
            return;
        };

        for origin in calls(cx.tcx, body) {
            let Some((blocking, help)) = reached(cx.tcx, origin, item) else {
                continue;
            };
            span_lint_hir_and_then(
                cx,
                EXPLICIT105_NO_BLOCKING_IN_HANDLER,
                origin.node,
                origin.span,
                format!("blocking call in {method}"),
                |diagnostic| {
                    if blocking.node != origin.node {
                        diagnostic.span_note(blocking.span, "this helper reaches a blocking operation");
                    }
                    diagnostic.help(help);
                },
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "handlers");
}
