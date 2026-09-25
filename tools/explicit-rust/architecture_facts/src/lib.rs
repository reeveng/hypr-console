//! Not a rule: what a crate runs, listens to and connects to, written down.
//!
//! `docs/architecture/` is a map of how the running desktop is connected, and
//! the half of it that is not in `desktop.conf` or a unit file is in the code:
//! which program a crate runs, which pool topic it subscribes to, whether it
//! asks the compositor, opens a socket, or keeps a clock of its own. A search
//! of the text had been answering those, one test at a time -- the pool's own
//! listener test read the tree for a spelling -- and a search sees a spelling,
//! not a call. `use console_compositor::socket::Socket as S` and a word in a
//! string both read wrong to it. The compiler has already resolved every path
//! to its definition by the time a late pass runs, so this asks the compiler.
//!
//! It is a lint because that is the one door into the compiler this tree
//! already keeps open: the suite's nightly, `clippy_utils`, and `cargo dylint`
//! to drive it. It emits nothing. What it finds goes to one file per compiled
//! crate, under the directory `CONSOLE_ARCHITECTURE_FACTS` names, and without
//! that variable it writes nothing at all, so a run that did not ask for the
//! facts is not slowed by them. The file is written at the end of every crate
//! it is handed, empty or not, which is what lets `just map` say it visited
//! every crate: a cached crate is not handed to it, so the recipe cleans the
//! workspace's own crates first, and `console-architecture` refuses facts that
//! are missing a target cargo says exists.
//!
//! It is named outside the `explicit*` pattern on purpose, so `cargo dylint
//! --all` and the gate never load it.
//!
//! What a fact means is decided here and nowhere else: the definitions below
//! are the vocabulary, and the reader of the facts sees `runs Hyprctl` rather
//! than a path it would have to know how to spell. A use of a definition from
//! the crate being compiled is not a fact about it -- the pool's own client
//! calling its own `connect` is the machinery, not a subscription -- so only
//! a use that crosses into another crate is written.
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
    /// Not a rule. Records what a crate runs, listens to and connects to, for
    /// `just map`, and says nothing.
    pub ARCHITECTURE_FACTS,
    Warn,
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

const ASKING: [&str; 12] = [
    "query",
    "query_at",
    "query_with",
    "request",
    "request_with",
    "told_at",
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
        ("console_events", "subscription::connect" | "subscription::connect_at" | "Subscriber::subscribe" | "Subscriptions::subscribe" | "again::about") => {
            Some(Class::Subscribes)
        }
        ("console_events", "again::layers") => Some(Class::SubscribesTo("Compositor")),
        ("console_panel", "Page::listening") => Some(Class::Subscribes),
        ("console_program_contract", "Subscription::Topic" | "Effect::Subscribe") => Some(Class::Subscribes),
        ("console_program_contract", "Subscription::Timer" | "Timer::new") => Some(Class::Timer("contract")),
        ("console_compositor", "Socket::Events" | "Socket::Requests") => Some(Class::Socket),
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

fn is_topic(cx: &LateContext<'_>, variant: DefId) -> bool {
    cx.tcx.crate_name(variant.krate).as_str() == "console_program_contract"
        && cx.tcx.item_name(cx.tcx.parent(variant)).as_str() == "Topic"
}

// A tuple variant's constructor is a definition of its own under the variant,
// so the variant above it is what carries the name.
fn variant_of(cx: &LateContext<'_>, res: Res) -> Option<(DefId, CtorKind)> {
    match res {
        Res::Def(DefKind::Ctor(_, kind), id) => Some((cx.tcx.parent(id), kind)),
        _ => None,
    }
}

fn topic_named(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    let res = match expr.kind {
        ExprKind::Path(ref qpath) => cx.qpath_res(qpath, expr.hir_id),
        _ => return None,
    };
    let (variant, _) = variant_of(cx, res)?;

    match variant.krate != LOCAL_CRATE && is_topic(cx, variant) {
        true => Some(cx.tcx.item_name(variant).to_string()),
        false => None,
    }
}

// The topics a listen names where it names them literally: `&[Topic::Sound]`,
// `Topic::Path(folder)`. A topic handed in as a variable is `*`, and the
// reader of the facts widens it to every topic the same crate names.
fn topics_in<'tcx>(cx: &LateContext<'tcx>, given: &[&'tcx Expr<'tcx>]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();

    for one in given {
        let _ = for_each_expr(cx, *one, |inner| {
            if let Some(topic) = topic_named(cx, inner) {
                found.insert(topic);
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
    fn record<'tcx>(&mut self, cx: &LateContext<'tcx>, id: DefId, given: &[&'tcx Expr<'tcx>]) {
        if id.krate == LOCAL_CRATE {
            return;
        }

        let krate = cx.tcx.crate_name(id.krate).to_string();
        let path = path_of(cx, id);
        let last = tail(&path, 1);
        let variant = matches!(cx.tcx.def_kind(id), DefKind::Variant);

        // A program is the variant; `Program::command` is the same program
        // asked for its spelling, and was already written where it was named.
        match classify(&krate, &path) {
            Some(Class::Runs) if variant => self.said("runs", last),
            Some(Class::Starts) if variant => self.said("starts", last),
            Some(Class::Runs | Class::Starts) => {}
            Some(Class::Subscribes) => {
                for topic in topics_in(cx, given) {
                    self.said("subscribes", topic);
                }
            }
            Some(Class::SubscribesTo(topic)) => self.said("subscribes", topic.to_string()),
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
        if is_left_out(cx) || expr.span.from_expansion() {
            return;
        }

        match expr.kind {
            ExprKind::Call(called, arguments) => {
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
                let given: Vec<&'tcx Expr<'tcx>> = std::iter::once(receiver).chain(arguments.iter()).collect();

                self.record(cx, id, &given);
            }
            ExprKind::Path(ref qpath) => {
                let res = cx.qpath_res(qpath, expr.hir_id);

                if let Some(topic) = topic_named(cx, expr) {
                    self.said("names", topic);
                }

                if let Some((variant, CtorKind::Const)) = variant_of(cx, res) {
                    self.record(cx, variant, &[]);
                }
            }
            _ => {}
        }
    }

    // The pool's sources are the arms of one match, and an arm that can
    // answer `Subscribed::Yes` is a topic with something behind it.
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

        if !is_topic(cx, variant) {
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
