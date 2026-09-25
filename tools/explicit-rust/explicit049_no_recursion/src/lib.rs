//! EXPLICIT049: a function does not reach itself; the depth is a loop's to count.
//!
//! TigerStyle takes this one from the Power of Ten: no recursion, because the
//! depth is an input and the stack is a limit nobody wrote down. A walk over a
//! directory or a widget tree goes as deep as whatever it is handed, and what it
//! runs out of is something the process owns and no line of it names.
//!
//! The answer is one of two spellings. A depth handed in and counted down says
//! the limit where the walk starts; a stack of its own, pushed and popped until
//! it is empty, cannot run out of anything but memory the program asked for by
//! name. The second is the one to reach for, because it is also the one a
//! reader can bound without holding the call chain in their head.
//!
//! The question is asked of the crate rather than of the function, because the
//! recursion worth finding is the one that goes through a second function: `a`
//! calls `b` and `b` calls `a`, and neither body says so. Every place one of
//! this crate's functions is named -- called, or handed on as a value to be
//! called later -- is an edge, and a call site is reported when what it names
//! can find its way back to where it stands. A trait method is followed to the
//! impl the call resolves to where the types say which, so a `Display` that
//! formats its own children is found by the same walk. A closure is part of
//! the function it is written in, except a `move` one, which is taken away to
//! be run from a stack that is not this one.
//!
//! It asks the source rather than the code the compiler emits. A recursion that
//! is turned into a loop on the way out is still a recursion the reader has to
//! bound, and the reader is who the rule is for.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{CaptureBy, Expr, ExprKind, HirId, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Instance;
use rustc_span::Span;

dylint_linting::impl_late_lint! {
    /// EXPLICIT049: a function that reaches itself, directly or through
    /// another, goes as deep as its input and spends a stack nobody bounded.
    /// Hand the depth in and count it down, or keep a stack of its own.
    pub EXPLICIT049_NO_RECURSION,
    Deny,
    "a function that reaches itself; the depth is a stack nobody bounded",
    Explicit049NoRecursion::new()
}

// Every edge is kept with the node it was written at, because the question is
// answered once the walk is over and a level is read off the node an `allow`
// was written on rather than off the crate the pass is standing at by then.
pub struct Explicit049NoRecursion {
    names: HashMap<LocalDefId, Vec<LocalDefId>>,
    written: Vec<Named>,
}

struct Named {
    caller: LocalDefId,
    what: LocalDefId,
    node: HirId,
    at: Span,
}

impl Explicit049NoRecursion {
    pub fn new() -> Self {
        Self { names: HashMap::new(), written: Vec::new() }
    }

    fn reaches(&self, from: LocalDefId, to: LocalDefId) -> bool {
        let mut seen = HashSet::new();
        let mut waiting = vec![from];

        while let Some(here) = waiting.pop() {
            if here == to {
                return true;
            }

            if !seen.insert(here) {
                continue;
            }

            waiting.extend(self.names.get(&here).into_iter().flatten());
        }

        false
    }
}

impl Default for Explicit049NoRecursion {
    fn default() -> Self {
        Self::new()
    }
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A trait method named through the trait is followed to the impl the types
// choose, where they choose one. Where they do not -- a generic caller -- the
// trait's own method is the edge, which finds nothing, and that is the honest
// answer: which body runs is decided by a caller the crate may not have.
fn named_function(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<DefId> {
    let id = match expr.kind {
        ExprKind::Path(ref qpath) => match cx.qpath_res(qpath, expr.hir_id) {
            Res::Def(DefKind::Fn | DefKind::AssocFn, id) => id,
            _ => return None,
        },
        ExprKind::MethodCall(..) => cx.typeck_results().type_dependent_def_id(expr.hir_id)?,
        _ => return None,
    };

    let args = cx.typeck_results().node_args(expr.hir_id);

    match Instance::try_resolve(cx.tcx, cx.typing_env(), id, args) {
        Ok(Some(instance)) => Some(instance.def_id()),
        _ => Some(id),
    }
}

// A closure is part of the function it is written in: what it calls, the
// function calls, while the function is still on the stack. A `move` closure is
// the exception, because what it takes it takes in order to be run later -- a
// callback handed to a toolkit, a thread -- from a stack that is not this one,
// so a function that registers itself again from a callback is a loop of
// events rather than a descent. That is a reading of the spelling rather than a
// proof, and it is the ceiling of the rule: a `move` closure called on the spot
// is not seen.
fn is_carried_away(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    for (_id, node) in cx.tcx.hir_parent_iter(expr.hir_id) {
        match node {
            Node::Expr(Expr { kind: ExprKind::Closure(closure), .. }) => match closure.capture_clause {
                CaptureBy::Value { .. } => return true,
                _ => {}
            },
            Node::Item(_) | Node::ImplItem(_) | Node::TraitItem(_) => return false,
            _ => {}
        }
    }

    false
}

fn standing_in(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<LocalDefId> {
    if is_carried_away(cx, expr) {
        return None;
    }

    let body = cx.tcx.hir_enclosing_body_owner(expr.hir_id);

    cx.tcx.typeck_root_def_id(body.to_def_id()).as_local()
}

impl<'tcx> LateLintPass<'tcx> for Explicit049NoRecursion {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let Some(what) = named_function(cx, expr).and_then(DefId::as_local) else {
            return;
        };

        let Some(caller) = standing_in(cx, expr) else {
            return;
        };

        let at = match expr.kind {
            ExprKind::MethodCall(segment, ..) => segment.ident.span,
            _ => expr.span,
        };

        self.names.entry(caller).or_default().push(what);
        self.written.push(Named { caller, what, node: expr.hir_id, at });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        for one in self.written.iter() {
            let caller = &one.caller;

            if !self.reaches(one.what, *caller) {
                continue;
            }

            let caller_name = cx.tcx.item_name(caller.to_def_id());
            let named_name = cx.tcx.item_name(one.what.to_def_id());

            let message = match one.what == *caller {
                true => format!("`{caller_name}` calls itself, so its depth is whatever it is handed"),
                false => format!(
                    "`{caller_name}` calls `{named_name}`, which finds its way back to `{caller_name}`, \
                     so the depth of the two is whatever they are handed"
                ),
            };

            span_lint_hir_and_then(cx, EXPLICIT049_NO_RECURSION, one.node, one.at, message, |said| {
                said.help(
                    "keep a stack of its own and walk until it is empty, or hand the depth in and count \
                     it down. Either one says the limit where the walk starts; a call that reaches itself \
                     spends a stack the process owns and no line of it names, and a recursion the \
                     compiler turns into a loop is still one the reader has to bound",
                );
            });
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
