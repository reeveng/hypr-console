//! EXPLICIT028: a list is not searched from inside a loop.
//!
//! `for name in wanted { match held.contains(name) { … } }` reads as one line
//! about one name and is two loops. Nothing on the screen says so: `contains`
//! is a word, the loop around it is a word, and the multiplication between them
//! is the thing neither of them mentions. That is the same complaint every rule
//! in this suite makes -- what matters is not written down -- said about time
//! instead of about a type.
//!
//! It is also the fault that cannot be found by using the thing. Two loops over
//! a list of six are free, over a list of sixty are unnoticeable, and over the
//! list somebody's music folder turns out to hold are a desktop that has
//! stopped. So it is worth a compiler saying it while the list is still small,
//! because the day it is not small there is nothing to notice.
//!
//! What is asked is narrow enough to answer off the expression: a linear walk
//! over a slice, an array or a `Vec` -- `contains`, or `any`, `find`,
//! `find_map`, `position` and `rposition` over one of those -- with a loop
//! somewhere above it in the same body. A map or a set is not asked, because
//! looking something up in one is what a map and a set are for, and that is
//! also the fix: build the set once, outside, and ask it inside.
//!
//! A closure is walked into as far as something else promises, and no further.
//! A `contains` inside a closure is the same fault, but a closure on its own
//! has no promise about when it runs and a rule that guessed would be guessing
//! at the only part that matters. What it is handed to can make that promise:
//! `wanted.iter().filter(|one| held.contains(one))` is this rule's loop spelled
//! as a word, multiplying exactly as much as the `for` does, and for a long
//! while it was the spelling that got past. So the climb stops at a closure and
//! then asks what the closure was given to. The iterator words that run a body
//! once per item carry it on, over the list they were called on; everything
//! else ends it.
//!
//! Those words are shared with families that run a body exactly once --
//! `Option::map`, `Result::and_then`, `unwrap_or_else` -- so the word is not
//! what is believed. What is asked is the receiver: an iterator, or a
//! collection, because `retain` and the sorts are written on the collection
//! rather than on an iterator over it. A `map` over an `Option` multiplies by
//! nothing and is not asked about.
//!
//! Where the list is a handful of things known at the time of writing -- the
//! modifier keys, the corners of a rectangle, the four edges a window can be
//! snapped to -- a set would be slower and sillier than the walk. The first
//! reading of this tree said so loudly: most of what the rule found the first
//! time it was run was a `const` table walked inside a loop, and a set of six
//! modifier keys is not a square in any sense worth a compiler's breath. So a
//! list whose length was decided when the code was written is not asked about
//! at all -- a `const`, a `static`, or an array with its length in its type.
//! The multiplication those make is a constant, and a constant is not what this
//! rule is against.
//!
//! The loop over the search is asked the same question, because a square needs
//! both of its sides. `for step in EVERY { settings.iter().find(…) }` runs the
//! walk as many times as somebody typed, which is linear however long the list
//! is. So a loop whose length was fixed when it was written -- an array, a
//! `const`, a `static` -- does not count as standing over anything, and it is a
//! loop that is not fixed, somewhere above, that makes this a rule.
//! EXPLICIT029 asks its loops the same question for the same reason.
//!
//! A list made inside the loop that searches it is the third thing not asked
//! about, and it is the one worth spelling out. `for job in every { let played
//! = bound.filter(…).collect(); played.iter().any(…) }` looks exactly like the
//! fault and is not one: the list is built and walked once per turn, so the
//! two together are a single pass over the same items the build already paid
//! for. What makes a square is a list that outlives the turn, which is why the
//! question is asked of the innermost loop only -- a list made in an outer
//! loop's body and searched in an inner one multiplies just as much as one
//! made outside both.
//!
//! What is left is a list that grows searched inside a loop that grows, and
//! that is a fault rather than a thing to be allowed. The first reading of this
//! tree said "a handful of open menus, a handful of networks in range" at most
//! of the sites and wrote the sentence instead of the fix; the sentence was
//! wrong at every one of them, because a handful is what a list is on the day
//! somebody writes the allow and not on the day it matters. Every one of them
//! is a set or a map now, and the shapes they turned into are worth knowing:
//! a `BTreeSet` beside the list where the order of the list is the answer, an
//! `entry` on a `BTreeMap` where the walk was really grouping, and
//! `Path::ancestors` where the walk was really a prefix test.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::higher::ForLoop;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, HirId, LoopSource, Node};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT028: searching a list from inside a loop is two loops written
    /// as one line. `held.contains(name)` inside `for name in wanted` visits
    /// every held name for every wanted one, and nothing on the screen says
    /// so. Make a set outside the loop and ask it inside.
    pub EXPLICIT028_NO_SEARCH_IN_A_LOOP,
    Deny,
    "a list walked from end to end inside a loop; the work grows with the square"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A slice, an array or a `Vec`: the shapes with no way to find something but
// to look at everything. A map and a set are deliberately absent -- asking one
// is the fix rather than the fault.
fn is_a_list<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    let held = cx.typeck_results().expr_ty(expr).peel_refs();

    match held.kind() {
        ty::Slice(_) => true,
        ty::Adt(held, _) => {
            let named = cx.tcx.def_path_str(held.did());

            matches!(
                named.as_str(),
                "std::vec::Vec" | "alloc::vec::Vec" | "std::collections::VecDeque" | "alloc::collections::VecDeque"
            )
        }
        _ => false,
    }
}

// A list whose length was decided when this was written. An array carries its
// length in its type; a `const` or a `static` is a table somebody typed out.
// Neither is a list that grows, and neither is what a square is made of.
fn was_fixed_when_written<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> bool {
    match cx.typeck_results().expr_ty(expr).peel_refs().kind() {
        ty::Array(..) => return true,
        _ => {}
    }

    let ExprKind::Path(ref qpath) = expr.kind else {
        return false;
    };

    matches!(
        cx.qpath_res(qpath, expr.hir_id),
        Res::Def(DefKind::Const { .. } | DefKind::Static { .. }, _)
    )
}

fn walks_the_whole_list(named: &str) -> bool {
    matches!(named, "any" | "find" | "find_map" | "position" | "rposition")
}

// Two spellings of one walk: `list.contains(&x)`, and `list.iter().any(…)` and
// its family. Anything else asking those questions is asking a map or a set,
// which is the answer rather than the complaint.
fn searches_a_list<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(String, &'tcx Expr<'tcx>)> {
    let ExprKind::MethodCall(asked, held, ..) = expr.kind else {
        return None;
    };

    let named = asked.ident.name.as_str().to_string();

    match named.as_str() {
        "contains" => match is_a_list(cx, held) && !was_fixed_when_written(cx, held) {
            true => return Some((named, held)),
            false => return None,
        },
        _ => {}
    }

    if !walks_the_whole_list(named.as_str()) {
        return None;
    }

    let ExprKind::MethodCall(walked, held, ..) = held.kind else {
        return None;
    };

    if !matches!(walked.ident.name.as_str(), "iter" | "into_iter" | "iter_mut") {
        return None;
    }

    match is_a_list(cx, held) && !was_fixed_when_written(cx, held) {
        true => Some((named, held)),
        false => None,
    }
}

// The `let` a list came from, when it came from one in this body. A field, a
// parameter reached through `self`, or anything computed in place answers
// `None`, and rightly: those outlive the turn of the loop.
fn made_by_a_let<'tcx>(cx: &LateContext<'tcx>, held: &Expr<'tcx>) -> Option<HirId> {
    let ExprKind::Path(ref qpath) = held.kind else {
        return None;
    };

    match cx.qpath_res(qpath, held.hir_id) {
        Res::Local(made) => Some(made),
        _ => None,
    }
}

// Whether that `let` is inside this loop. Climbing from the binding up to the
// loop is exact where comparing spans is not: a `for` is a desugar and its
// spans overlap things it does not contain.
fn made_inside(cx: &LateContext<'_>, made: HirId, loop_at: HirId) -> bool {
    let mut at = made;
    let mut left = 64usize;

    while left > 0 {
        left = left.saturating_sub(1);

        match at == loop_at {
            true => return true,
            false => {}
        }

        let above = cx.tcx.parent_hir_id(at);

        match above == at {
            true => return false,
            false => at = above,
        }
    }

    false
}

// The root of what a loop walks, under whatever was chained onto it.
fn what_is_walked<'tcx>(from: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    let mut at = from;
    let mut left = 16usize;

    while left > 0 {
        left = left.saturating_sub(1);

        let ExprKind::MethodCall(_, taken, ..) = at.kind else {
            return at;
        };

        at = taken;
    }

    at
}

// A closure is a body of its own and nothing here can say when it runs --
// unless it was handed to something that says so itself. `map` over a list
// runs its closure once per item, which is this rule's multiplication wearing
// a different spelling, and the list is the receiver it was called on. The
// families that run a closure exactly once are not that: `Option::map`,
// `Result::and_then`, `unwrap_or_else` and their kin share the words and
// multiply by nothing, so what is walked is asked whether it is many things
// before the word is believed.
fn walks_a_list<'tcx>(cx: &LateContext<'tcx>, closure: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
    let Node::Expr(above) = cx.tcx.parent_hir_node(closure.hir_id) else {
        return None;
    };

    let ExprKind::MethodCall(named, walked, ..) = above.kind else {
        return None;
    };

    let per_item = [
        "all", "any", "filter", "filter_map", "find", "find_map", "flat_map", "fold", "for_each",
        "inspect", "map", "map_while", "max_by_key", "min_by_key", "partition", "position",
        "retain", "rposition", "scan", "skip_while", "sort_by", "sort_by_key", "sort_unstable_by",
        "sort_unstable_by_key", "take_while", "try_fold", "try_for_each",
    ];

    match per_item.contains(&named.ident.as_str()) {
        true => {}
        false => return None,
    }

    match is_many(cx, walked) {
        true => Some(walked),
        false => None,
    }
}

// Whether what is being walked is many things or one. The iterator half is
// asked of the trait; the collection half is asked of the type, because
// `retain` and the sorts are written on the collection rather than on an
// iterator over it.
fn is_many<'tcx>(cx: &LateContext<'tcx>, walked: &'tcx Expr<'tcx>) -> bool {
    let held = cx.typeck_results().expr_ty_adjusted(walked).peel_refs();

    match cx.tcx.get_diagnostic_item(sym::Iterator) {
        Some(iterating) => match implements_trait(cx, held, iterating, &[]) {
            true => return true,
            false => {}
        },
        None => {}
    }

    match held.kind() {
        ty::Slice(..) | ty::Array(..) => true,
        ty::Adt(held, _) => {
            let named = cx.tcx.def_path_str(held.did());

            [
                "std::vec::Vec",
                "std::collections::VecDeque",
                "std::collections::HashMap",
                "std::collections::HashSet",
                "std::collections::BTreeMap",
                "std::collections::BTreeSet",
            ]
            .contains(&named.as_str())
        }
        _ => false,
    }
}

// A loop that runs as many times as somebody typed multiplies by a number
// decided when the code was written, which is not a square.
fn a_fixed_loop<'tcx>(cx: &LateContext<'tcx>, walked: &'tcx Expr<'tcx>) -> bool {
    let root = what_is_walked(walked);

    match root.kind {
        ExprKind::Array(..) | ExprKind::Repeat(..) => return true,
        _ => {}
    }

    was_fixed_when_written(cx, root)
}

// Up the tree from the search until the body it is in runs out. A closure is
// where the walk stops: it is a body of its own and there is no saying from
// here when it runs.
fn a_loop_stands_over_it(cx: &LateContext<'_>, expr: &Expr<'_>, made: Option<HirId>) -> bool {
    let mut at = expr.hir_id;
    let mut left = 64usize;
    let mut innermost = true;

    while left > 0 {
        left = left.saturating_sub(1);

        let above = cx.tcx.parent_hir_node(at);

        match above {
            Node::Expr(above) => {
                // Only of the loop the search is actually in. A list made in an
                // outer loop's body and searched in an inner one multiplies
                // exactly as much as one made outside every loop.
                match matches!(above.kind, ExprKind::Loop(..)) {
                    true => {
                        match innermost && made.is_some_and(|made| made_inside(cx, made, above.hir_id)) {
                            true => return false,
                            false => {}
                        }

                        innermost = false;
                    }
                    false => {}
                }

                // A `for` is a `match` over `into_iter` with a `Loop` inside it,
                // so the loop is met first and the head of it one node later.
                match ForLoop::hir(above) {
                    Some(walking) => match a_fixed_loop(cx, walking.arg) {
                        true => {}
                        false => return true,
                    },
                    None => match above.kind {
                        ExprKind::Loop(_, _, LoopSource::ForLoop, _) => {}
                        ExprKind::Loop(_, _, LoopSource::Loop | LoopSource::While, _) => {
                            return true;
                        }
                        ExprKind::Closure(..) => match walks_a_list(cx, above) {
                            Some(walked) => match a_fixed_loop(cx, walked) {
                                true => {}
                                false => return true,
                            },
                            None => return false,
                        },
                        _ => {}
                    },
                }

                at = above.hir_id;
            }
            Node::Block(above) => at = above.hir_id,
            Node::Stmt(above) => at = above.hir_id,
            Node::LetStmt(above) => at = above.hir_id,
            Node::Arm(above) => at = above.hir_id,
            _ => return false,
        }
    }

    false
}

impl<'tcx> LateLintPass<'tcx> for Explicit028NoSearchInALoop {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let Some((named, held)) = searches_a_list(cx, expr) else {
            return;
        };

        if !a_loop_stands_over_it(cx, expr, made_by_a_let(cx, held)) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT028_NO_SEARCH_IN_A_LOOP,
            expr.span,
            format!("`{named}` walks the whole list, and a loop stands over it: the work here grows with the square"),
            None,
            "make the list into a `BTreeSet` or a `HashSet` once, before the loop, and ask that inside \
             it -- or, where the walk is really gathering what belongs together, fill a map with `entry` \
             in one pass and drop the search altogether",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
