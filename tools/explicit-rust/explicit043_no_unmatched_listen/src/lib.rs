//! EXPLICIT043: a topic listened to is a topic the program stops listening to.
//!
//! This is the one thing in the contract's list of effects that is half of a
//! pair. `Ask` is over when the answer comes back, `Start` is answered by
//! `console-program-lifetime` saying how long the child lives, `Write` ends
//! with the file on the disk. `Subscribe` is none of those: it adds a topic to a
//! connection and then nothing happens, for as long as the program runs. What
//! it costs is not the subscription, it is every event on that topic
//! afterwards -- woken, decoded, walked past, and thrown away, by a program
//! that stopped caring about it three screens ago.
//!
//! `console-events` is built for the pair and says so: the pool has `subscribe`
//! and `unsubscribe`, the wire has both words, and a connection dropped takes its
//! topics with it. What has no name is the shape in between -- a topic added
//! part way through a program's life, by a program that goes on living. That
//! is `Subscriber::subscribe`, and the only thing that ever takes one back is
//! `Subscriber::unsubscribe`.
//!
//! The rule is the one obligation in this suite that a single line cannot
//! answer, because the two halves are in two places on purpose: the listen is
//! where the program starts caring and the stop listening is where it stops, and if
//! those were the same line there would be nothing to subscribe to. So the
//! question is asked of the crate rather than of the call. A crate that says
//! `Listen` and never says `StopListening` is not a crate with a bug at a line, it is
//! a crate that has decided -- without writing it down -- that everything it
//! ever listens to, it listens to forever.
//!
//! That is a weaker question than the one a type would ask, and it is the
//! honest one to ask here. A linear type could hold the pair properly: a
//! subscription that must be given back, a `Subscriber` that will not drop with
//! topics on it. Rust can express that and this tree has the place for it --
//! it is `console-program-lifetime`'s trick, where the two ways a child can
//! end are the two variants and there is no third. What stops it being written
//! today is that `Effect` is a value handed across a loop that does not own the
//! connection, so there is nothing for the obligation to be attached to.
//! The backlog is where that belongs rather than a rule pretending to it.
//!
//! It arrives green, which is worth saying plainly: nothing in the tree
//! constructs `Effect::Subscribe` at all today, and the two crates that call
//! `subscribe` both call `unsubscribe`. It is a ratchet rather than a sweep --
//! the third after EXPLICIT034 and EXPLICIT037 -- and what it is for is the
//! program someone writes next, which will reach for `Subscribe` because it is
//! on the list and will have no reason to think about the other half until
//! something is slow and no one knows why.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

dylint_linting::impl_late_lint! {
    /// EXPLICIT043: `Effect::Subscribe` adds a topic and nothing takes it away.
    /// A crate that never says `Effect::Unsubscribe` has decided that everything it
    /// listens to it listens to for as long as it runs, and has written that
    /// nowhere.
    pub EXPLICIT043_NO_UNMATCHED_LISTEN,
    Deny,
    "a topic listened to in a crate that never stops_listening one",
    Explicit043NoUnmatchedListen::new()
}

// The spellings of the pair. `Effect::Subscribe` is what a program written to
// the contract hands back; `Subscriber::subscribe` is what the runtime underneath it
// calls, and `Subscriptions::subscribe` is the same call once the receiver has
// been split off to a thread of its own. All are asked, because a program can be
// written at any of those levels.
const PAIRS: [(&str, &str); 3] = [
    ("Effect::Subscribe", "Effect::Unsubscribe"),
    ("Subscriber::subscribe", "Subscriber::unsubscribe"),
    ("Subscriptions::subscribe", "Subscriptions::unsubscribe"),
];

// The listen is kept with the node it was written at as well as its span,
// because the question is answered after the walk is over. By then the pass has
// left every `allow` behind it: a level is read off the node it was written on,
// and in `check_crate_post` the node the pass is standing at is the crate.
pub struct Explicit043NoUnmatchedListen {
    listened: Vec<(usize, HirId, Span)>,
    stopped_listening: [bool; 3],
}

impl Explicit043NoUnmatchedListen {
    pub fn new() -> Self {
        Self { listened: Vec::new(), stopped_listening: [false; 3] }
    }
}

impl Default for Explicit043NoUnmatchedListen {
    fn default() -> Self {
        Self::new()
    }
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// What a call resolves to, whether it was written as a variant, a path or a
// method. A variant in a `match` arm is a pattern and is never an expression,
// so what this sees is a `Subscribe` someone is making rather than one someone
// is handling -- which is the difference the rule is about.
fn what_is_called(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    match expr.kind {
        ExprKind::Call(called, _) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(kind @ (DefKind::Fn | DefKind::AssocFn | DefKind::Ctor(..)), id) =
                cx.qpath_res(qpath, called.hir_id)
            else {
                return None;
            };

            // A tuple variant's constructor is a definition of its own and
            // spells itself `{{constructor}}` at the end of the path, so the
            // variant above it is what carries the word.
            match kind {
                DefKind::Ctor(..) => Some(cx.tcx.def_path_str(cx.tcx.parent(id))),
                _ => Some(cx.tcx.def_path_str(id)),
            }
        }
        ExprKind::MethodCall(..) => {
            let id = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;

            Some(cx.tcx.def_path_str(id))
        }
        _ => None,
    }
}

// A constructor's own path ends in the variant it makes, under whatever module
// the enum is in, so the tail is what is asked about rather than the whole of
// it. The tail has to start at a path separator: `CompareEffect::Subscribe` is a
// program's own variant and ends in the letters of `Effect::Subscribe` without
// being one.
fn is_the_word(named: &str, word: &str) -> bool {
    match named.strip_suffix(word) {
        Some(before) => before.is_empty() || before.ends_with("::"),
        None => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit043NoUnmatchedListen {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        for (which, (listens, stops_listening)) in PAIRS.into_iter().enumerate() {
            match is_the_word(named.as_str(), listens) {
                true => self.listened.push((which, expr.hir_id, expr.span)),
                false => {}
            }

            match is_the_word(named.as_str(), stops_listening) {
                true => match self.stopped_listening.get_mut(which) {
                    Some(said) => *said = true,
                    None => {}
                },
                false => {}
            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        for (which, node, at) in self.listened.iter() {
            let Some(false) = self.stopped_listening.get(*which) else {
                continue;
            };

            let Some((listens, stops_listening)) = PAIRS.get(*which) else {
                continue;
            };

            span_lint_hir_and_then(
                cx,
                EXPLICIT043_NO_UNMATCHED_LISTEN,
                *node,
                *at,
                format!("`{listens}` is said in this crate and `{stops_listening}` is not: the topic is listened to for as long as the program runs"),
                |said| {
                    said.help(
                        "say where it stops. Every event on a topic no one is reading any more is still a \
                         wake, a decode and a walk past, and the cost of it is nowhere near the line that \
                         caused it. If the answer really is for the life of the program -- a program that \
                         is about the topic and ends when it stops caring -- allow this rule at the listen \
                         and let the reason say so",
                    );
                },
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
