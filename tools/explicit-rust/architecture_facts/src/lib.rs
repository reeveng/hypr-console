//! Not a rule: what a crate runs, listens to and connects to, written down.
//!
//! `docs/architecture/` is a map of how the running desktop is connected, and
//! the half of it that is not in `desktop.conf` or a unit file is in the code:
//! which program a crate runs, which pool event group it subscribes to, whether
//! it asks the compositor, opens a socket, or keeps a clock of its own. A
//! search of the text had been answering those, one test at a time -- the
//! pool's own listener test read the tree for a spelling -- and a search sees a
//! spelling, not a call. `use console_compositor::socket::Socket as S` and a
//! word in a string both read wrong to it. The compiler has already resolved
//! every path to its definition by the time a late pass runs, so this asks the
//! compiler.
//!
//! It is a lint because that is the one door into the compiler this tree
//! already keeps open: the suite's nightly, `clippy_utils`, and `cargo dylint`
//! to drive it. It emits nothing. What it finds goes to one file per compiled
//! crate, under the directory `CONSOLE_ARCHITECTURE_FACTS` names, and without
//! that variable it writes nothing at all, so a run that did not ask for the
//! facts is not slowed by them. The file is written at the end of every crate
//! it is handed, empty or not, which is what lets `just map` say it visited
//! every crate: a cached crate is not handed to it, so the file it wrote the
//! last time stands until cargo builds the crate again, the recipe cleans the
//! workspace's own crates only when there are no facts at all, and
//! `console-architecture` refuses facts that are missing a target cargo says
//! exists.
//!
//! It is named outside the `explicit*` pattern on purpose, so `cargo dylint
//! --all` and the gate never load it.
//!
//! What a fact means is decided here and nowhere else: the definitions below
//! are the vocabulary, and the reader of the facts sees `runs Hyprctl` rather
//! than a path it would have to know how to spell. Effects are usually
//! recorded when a call crosses into another crate. The Linux adapter also
//! records subscription calls because its runtime shares a crate with the
//! event client.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::BTreeSet;
use std::ops::ControlFlow;
use std::path::PathBuf;

use clippy_utils::visitors::for_each_expr;
use rustc_hir::def::{CtorKind, DefKind, Res};
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_hir::{Arm, Expr, ExprKind, PatExprKind, PatKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::print::with_no_trimmed_paths;

dylint_linting::impl_late_lint! {
    /// Records architecture and host access facts for `just map`.
    pub ARCHITECTURE_FACTS,
    // Keep collecting in crates that allow warnings. This pass emits facts.
    Deny,
    "records what a crate runs, listens to and connects to; never emitted",
    ArchitectureFacts::default()
}

const WHERE: &str = "CONSOLE_ARCHITECTURE_FACTS";

#[derive(Default)]
pub struct ArchitectureFacts {
    facts: BTreeSet<(&'static str, String)>,
}

enum Class {
    Runs,
    Starts,
    Subscribes,
    SubscribesTo(&'static str),
    Asks,
    Socket,
    SocketEvents,
    Connects,
    Binds,
    Timer(&'static str),
}

const ASKING: [&str; 13] = [
    "ask_recorded",
    "query",
    "query_at",
    "query_with",
    "request",
    "request_with",
    "dispatch_at",
    "switch_layout",
    "switch_layout_with",
    "set_layouts",
    "set_layouts_with",
    "asked",
    "asked_at",
];

fn classify(krate: &str, path: &str) -> Option<Class> {
    let two = tail(path, 2);
    let last = tail(path, 1);

    match (krate, two.as_str()) {
        ("console_core_external_programs", _) if two.starts_with("Program::") => Some(Class::Runs),
        ("console_core_internal_programs", _) if two.starts_with("InternalProgram::") => Some(Class::Starts),
        ("console_operating_system_linux", "subscription::connect" | "subscription::connect_at" | "Subscriber::subscribe" | "Subscriptions::subscribe")
        | ("console_events", "again::about") => Some(Class::Subscribes),
        ("console_events", "again::layers") => Some(Class::SubscribesTo("Compositor")),
        ("console_panel", "Page::with_subscription") => Some(Class::Subscribes),
        ("console_program_contract", "Subscription::EventGroup") => Some(Class::Subscribes),
        ("console_program_contract", "Subscription::Timer" | "Timer::new") => Some(Class::Timer("contract")),
        ("console_compositor", "Socket::Events" | "Socket::Requests") => Some(Class::Socket),
        ("console_compositor", "Action::dispatch") => Some(Class::Asks),
        ("console_compositor", _) if ASKING.contains(&last.as_str()) => Some(Class::Asks),
        ("console_onscreen", "console_onscreen::events") => Some(Class::SocketEvents),
        ("std", "UnixStream::connect" | "UnixStream::connect_addr" | "UnixDatagram::connect") => Some(Class::Connects),
        ("std", "UnixListener::bind" | "UnixListener::bind_addr" | "UnixDatagram::bind") => Some(Class::Binds),
        ("std", "Receiver::recv_timeout") => Some(Class::Timer("recv_timeout")),
        ("std", "Instant::checked_add") => Some(Class::Timer("deadline")),
        ("glib", _) if last.starts_with("timeout_add") => Some(Class::Timer("glib")),
        _ => None,
    }
}

// A generic type's own method spells its parameters as a segment of the path
// -- `Receiver::<T>::recv_timeout` -- which is not a name anyone asks for.
fn tail(path: &str, count: usize) -> String {
    let parts: Vec<&str> = path.split("::").filter(|part| !part.starts_with('<')).collect();
    let from = parts.len().saturating_sub(count);

    parts[from..].join("::")
}

fn path_of(cx: &LateContext<'_>, id: DefId) -> String {
    with_no_trimmed_paths!(cx.tcx.def_path_str(id))
}

fn host_access(krate: &str, path: &str) -> Option<&'static str> {
    if krate == "libc" {
        return Some("system");
    }

    let mut parts = path.split("::").filter(|part| !part.starts_with('<'));
    let _ = parts.next();
    let module = parts.next()?;
    let operation = parts.last()?;

    match (krate, module, operation) {
        ("std", "fs", _) | ("rustix", "fs", _) => Some("filesystem"),
        ("std", "path", "exists" | "try_exists" | "is_file" | "is_dir" | "is_symlink"
            | "metadata" | "symlink_metadata" | "read_link" | "read_dir" | "canonicalize") => Some("filesystem"),
        ("std", "process", "spawn" | "status" | "output" | "exit" | "abort" | "id" | "kill" | "wait" | "try_wait" | "wait_with_output")
        | ("rustix", "process", _) => Some("process"),
        ("std", "time", "now" | "elapsed") | ("rustix", "time", _) => Some("clock"),
        ("std", "env", "var" | "var_os" | "vars" | "vars_os" | "args" | "args_os" | "current_dir" | "current_exe"
            | "set_current_dir" | "set_var" | "remove_var" | "temp_dir" | "home_dir") => Some("environment"),
        ("std", "net" | "os", "bind" | "connect" | "connect_addr" | "bind_addr" | "connect_timeout" | "pair")
        | ("rustix", "net", _) => Some("network"),
        ("std", "thread", "spawn" | "spawn_scoped" | "available_parallelism" | "sleep" | "sleep_until") => Some("threads"),
        ("rustix", _, _) => Some("system"),
        _ => None,
    }
}

fn is_event_group(cx: &LateContext<'_>, variant: DefId) -> bool {
    cx.tcx.crate_name(variant.krate).as_str() == "console_program_contract"
        && cx.tcx.item_name(cx.tcx.parent(variant)).as_str() == "EventGroup"
}

// A tuple variant's constructor is a definition of its own under the variant,
// so the variant above it is what carries the name.
fn variant_of(cx: &LateContext<'_>, res: Res) -> Option<(DefId, CtorKind)> {
    match res {
        Res::Def(DefKind::Ctor(_, kind), id) => Some((cx.tcx.parent(id), kind)),
        _ => None,
    }
}

fn event_group_named(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let res = match expr.kind {
        ExprKind::Path(ref qpath) => cx.qpath_res(qpath, expr.hir_id),
        _ => return None,
    };
    let (variant, _) = variant_of(cx, res)?;

    match variant.krate != LOCAL_CRATE && is_event_group(cx, variant) {
        true => Some(cx.tcx.item_name(variant).to_string()),
        false => None,
    }
}

// The event groups a listen names where it names them literally:
// `&[EventGroup::Sound]`, `EventGroup::Path(folder)`. An event group handed in
// as a variable is `*`, and the reader of the facts widens it to every event
// group the same crate names.
fn event_groups_in<'tcx>(cx: &LateContext<'tcx>, given: &[&'tcx Expr<'tcx>]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();

    for one in given {
        let _ = for_each_expr(cx, *one, |inner| {
            if let Some(event_group) = event_group_named(cx, inner) {
                found.insert(event_group);
            }

            ControlFlow::<()>::Continue(())
        });
    }

    if found.is_empty() {
        found.insert("*".to_string());
    }

    found
}

impl ArchitectureFacts {
    fn record_host_access(&mut self, cx: &LateContext<'_>, id: DefId) {
        let krate = cx.tcx.crate_name(id.krate);

        if matches!(krate.as_str(), "std" | "rustix" | "libc") {
            let path = path_of(cx, id);

            if let Some(capability) = host_access(krate.as_str(), &path) {
                self.said("host-access", format!("{capability} {path}"));
            }
        }
    }

    fn record<'tcx>(&mut self, cx: &LateContext<'tcx>, id: DefId, given: &[&'tcx Expr<'tcx>]) {
        let krate = cx.tcx.crate_name(id.krate).to_string();
        let path = path_of(cx, id);
        let class = classify(&krate, &path);

        // The Linux runtime and its event client share a crate. The runtime
        // still subscribes to the groups handed in by its programs.
        let linux_subscription = krate == "console_operating_system_linux" && matches!(class, Some(Class::Subscribes));

        if id.krate == LOCAL_CRATE && !linux_subscription {
            return;
        }

        let last = tail(&path, 1);
        let variant = matches!(cx.tcx.def_kind(id), DefKind::Variant);

        // A program is the variant; `Program::command` is the same program
        // asked for its spelling, and was already written where it was named.
        match class {
            Some(Class::Runs) if variant => self.said("runs", last),
            Some(Class::Starts) if variant => self.said("starts", last),
            Some(Class::Runs | Class::Starts) => {}
            Some(Class::Subscribes) => {
                for event_group in event_groups_in(cx, given) {
                    self.said("subscribes", event_group);
                }
            }
            Some(Class::SubscribesTo(event_group)) => self.said("subscribes", event_group.to_string()),
            Some(Class::Asks) => self.said("asks", "compositor".to_string()),
            Some(Class::Socket) => self.said("socket", last),
            Some(Class::SocketEvents) => self.said("socket", "Events".to_string()),
            Some(Class::Connects) => self.said("connects", "unix".to_string()),
            Some(Class::Binds) => self.said("binds", "unix".to_string()),
            Some(Class::Timer(how)) => self.said("timer", how.to_string()),
            None => {}
        }
    }

    fn said(&mut self, kind: &'static str, what: String) {
        self.facts.insert((kind, what));
    }
}

fn is_left_out(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test || cx.tcx.crate_name(LOCAL_CRATE).as_str() == "build_script_build"
}

fn in_function(cx: &LateContext<'_>, arm: &Arm<'_>) -> String {
    let owner = cx.tcx.hir_enclosing_body_owner(arm.hir_id);

    path_of(cx, owner.to_def_id())
}

fn pattern_path<'a>(arm: &'a Arm<'a>) -> Option<(&'a QPath<'a>, rustc_hir::HirId)> {
    match arm.pat.kind {
        PatKind::TupleStruct(ref qpath, ..) => Some((qpath, arm.pat.hir_id)),
        PatKind::Expr(expr) => match expr.kind {
            PatExprKind::Path(ref qpath) => Some((qpath, expr.hir_id)),
            _ => None,
        },
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for ArchitectureFacts {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_left_out(cx) {
            return;
        }

        let expanded = expr.span.from_expansion();
        match expr.kind {
            ExprKind::Call(called, arguments) if !expanded => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };
                let given: Vec<&'tcx Expr<'tcx>> = arguments.iter().collect();

                match cx.qpath_res(qpath, called.hir_id) {
                    Res::Def(DefKind::Fn | DefKind::AssocFn, id) => self.record(cx, id, &given),
                    Res::Def(DefKind::Ctor(_, CtorKind::Fn), id) => self.record(cx, cx.tcx.parent(id), &given),
                    _ => {}
                }
            }
            ExprKind::MethodCall(_, receiver, arguments, _) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                self.record_host_access(cx, id);
                if expanded {
                    return;
                }

                let given: Vec<&'tcx Expr<'tcx>> = std::iter::once(receiver).chain(arguments.iter()).collect();

                self.record(cx, id, &given);
            }
            ExprKind::Path(ref qpath) => {
                let res = cx.qpath_res(qpath, expr.hir_id);

                if let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = res {
                    self.record_host_access(cx, id);
                }
                if expanded {
                    return;
                }

                if let Some(event_group) = event_group_named(cx, expr) {
                    self.said("names", event_group);
                }

                if let Some((variant, CtorKind::Const)) = variant_of(cx, res) {
                    self.record(cx, variant, &[]);
                }
            }
            _ => {}
        }
    }

    // The pool's sources are the arms of one match, and an arm that can
    // answer `Subscribed::Yes` is an event group with something behind it.
    fn check_arm(&mut self, cx: &LateContext<'tcx>, arm: &'tcx Arm<'tcx>) {
        if is_left_out(cx) || cx.tcx.crate_name(LOCAL_CRATE).as_str() != "console_events" {
            return;
        }

        if tail(&in_function(cx, arm), 2) != "sources::hold" {
            return;
        }

        let Some((qpath, id)) = pattern_path(arm) else {
            return;
        };
        let Some((variant, _)) = variant_of(cx, cx.qpath_res(qpath, id)) else {
            return;
        };

        if !is_event_group(cx, variant) {
            return;
        }

        self.said("handles", cx.tcx.item_name(variant).to_string());

        let answers_yes = for_each_expr(cx, arm.body, |inner| {
            let ExprKind::Path(ref inner_path) = inner.kind else {
                return ControlFlow::Continue(());
            };

            match variant_of(cx, cx.qpath_res(inner_path, inner.hir_id)) {
                Some((said, _)) if tail(&path_of(cx, said), 2) == "Subscribed::Yes" => ControlFlow::Break(()),
                _ => ControlFlow::Continue(()),
            }
        })
        .is_some();

        if answers_yes {
            self.said("sources", cx.tcx.item_name(variant).to_string());
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        if is_left_out(cx) {
            return;
        }

        let Some(into) = std::env::var_os(WHERE).map(PathBuf::from) else {
            return;
        };
        let package = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| cx.tcx.crate_name(LOCAL_CRATE).to_string());
        let target = std::env::var("CARGO_BIN_NAME").unwrap_or_else(|_| "lib".to_string());

        self.said("builds", cx.tcx.crate_name(LOCAL_CRATE).to_string());

        for &number in cx.tcx.crates(()) {
            let name = cx.tcx.crate_name(number).to_string();

            if name.starts_with("console_") {
                self.said("links", name);
            }
        }

        let mut written = String::new();

        for (kind, what) in &self.facts {
            written.push_str(&format!(
                "{{\"package\":\"{package}\",\"target\":\"{target}\",\"kind\":\"{kind}\",\"what\":\"{what}\"}}\n"
            ));
        }

        let _ = std::fs::create_dir_all(&into);
        let file = into.join(format!("{package}.{target}.jsonl"));
        let beside = into.join(format!(".{package}.{target}.jsonl.part"));

        if std::fs::write(&beside, written).is_ok() {
            let _ = std::fs::rename(&beside, &file);
        }
    }
}
