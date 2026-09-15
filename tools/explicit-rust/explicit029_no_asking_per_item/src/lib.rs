//! EXPLICIT029: a program is not run once per item.
//!
//! Running a program is a fork, an exec, a link, a socket and a wait, and it
//! costs milliseconds however small the program is. It is written as one short
//! call, and a short call inside a loop is where a desktop goes to sit down:
//! `hyprctl` asked once per window is a launcher that opens in half a second on
//! the day somebody has thirty of them open, and there is no line anywhere
//! saying that is what will happen.
//!
//! This is EXPLICIT028's argument about a different multiplication, and on this
//! device it is the more likely of the two. Almost nothing here holds a list
//! long enough for a quadratic walk to matter; a great deal of it asks the
//! compositor or another program, and that answer costs about the same each
//! whether the list is one long or forty. `console-compositor` exists so that
//! the questions are in one place, and one place is also where they can be
//! asked once: `hyprctl` will answer about every window in a single reply, and
//! the loop belongs on the far side of that reply rather than around it.
//!
//! It asked about files too when it was written, and the first reading of this
//! tree took that half back out. A syscall is microseconds where a process is
//! milliseconds, and nearly every file read in a loop here is a read that has
//! to be per item: a directory walked one folder at a time, a staging copy made
//! one file at a time, sysfs holding one file per core and one per battery.
//! A rule whose answer is nearly always an allow is not a rule, it is
//! paperwork, and this one was going to be four fifths paperwork. So the file
//! half went, and what is left is the one that is worth stopping for.
//!
//! What it asks for is the shape rather than the count. Hoist the question out
//! of the loop and walk its answer, or ask the one question that answers for
//! every item at once. Where neither is possible -- a program that has to be
//! run per file because that is what the program takes, a directory that has to
//! be looked at entry by entry -- the site carries the allow and EXPLICIT018
//! asks the reason to say why the question could not be asked once.
//!
//! A loop whose length was fixed when it was written is not asked about, which
//! is EXPLICIT028's rule said about the other multiplication. `for at in
//! [INTO_IT, "0"]` is two attempts at one film; `for argv in [cloning,
//! configuring, compiling]` is three steps that are three different programs
//! and could never have been one. Neither is a fan-out and there is nothing to
//! hoist out of either -- what the rule is against is a program run once per
//! thing in a list nobody has counted.
//!
//! A closure stops the walk upward unless something above it says when it runs,
//! which is EXPLICIT028's sentence and the same one here: the iterator words
//! that run a body once per item carry the climb over the list they were called
//! on. `named.iter().map(|one| Command::new(one))` is a fork and an exec per
//! item however it is spelled, and for a long while this was the spelling that
//! got past. The words are shared with the families that run a body exactly
//! once, so what is asked is the receiver rather than the word: a `map` over an
//! `Option` multiplies by nothing.

//!
//! It arrived `Warn`, and the first reading of the tree is what narrowed it to
//! programs. What is left is a handful, and they are of two kinds: a question
//! that could have been asked once and was not, and a program that really does
//! have to be run per item -- one recording handed to one model, one session
//! started per thing being resumed. The second kind wants the allow and a
//! sentence saying so.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::higher::ForLoop;
use clippy_utils::sym;
use clippy_utils::ty::implements_trait;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, LoopSource, Node};
use rustc_middle::ty;
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT029: running a program from inside a loop pays for a fork and
    /// an exec once per item, and the price is the same however small the
    /// program is. Ask once outside the loop and walk the answer, or hand the
    /// one program the whole list.
    pub EXPLICIT029_NO_ASKING_PER_ITEM,
    Deny,
    "a program run from inside a loop; a fork and an exec once per item"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The four words a process is made of. Named by resolved path, so a `use` of
// any of them does not slip past.
fn runs_a_program(path: &str) -> bool {
    let program = [
        "std::process::Command::new",
        "std::process::Command::output",
        "std::process::Command::status",
        "std::process::Command::spawn",
    ];

    program.contains(&path)
}

// What a call resolves to, whether it was written as a path or as a method.
fn what_is_called(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    match expr.kind {
        ExprKind::Call(called, _) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id) else {
                return None;
            };

            Some(cx.tcx.def_path_str(id))
        }
        ExprKind::MethodCall(..) => {
            let id = cx.typeck_results().type_dependent_def_id(expr.hir_id)?;

            Some(cx.tcx.def_path_str(id))
        }
        _ => None,
    }
}

// The root of what a loop walks, under whatever was chained onto it:
// `[Some(stop), instead].into_iter().flatten()` is an array with two words
// after it.
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

// A loop that runs as many times as somebody typed. An array literal, an array
// type, a `const` or a `static`: each of those multiplies by a number that was
// decided when the code was written, which is not what this rule is against.
fn was_fixed_when_written<'tcx>(cx: &LateContext<'tcx>, walked: &'tcx Expr<'tcx>) -> bool {
    let root = what_is_walked(walked);

    match root.kind {
        ExprKind::Array(..) | ExprKind::Repeat(..) => return true,
        _ => {}
    }

    match cx.typeck_results().expr_ty(root).peel_refs().kind() {
        ty::Array(..) => return true,
        _ => {}
    }

    let ExprKind::Path(ref qpath) = root.kind else {
        return false;
    };

    matches!(
        cx.qpath_res(qpath, root.hir_id),
        Res::Def(DefKind::Const { .. } | DefKind::Static { .. }, _)
    )
}

fn a_loop_stands_over_it(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let mut at = expr.hir_id;
    let mut left = 64usize;

    while left > 0 {
        left = left.saturating_sub(1);

        match cx.tcx.parent_hir_node(at) {
            Node::Expr(above) => {
                // A `for` is a `match` over `into_iter` with a `Loop` inside it,
                // so the loop is met first and the head of it one node later.
                // The `Loop` of a `for` is stepped over and the `match` above it
                // is where the question is asked. A fixed loop inside one that
                // is not fixed is still once per item, so a fixed one keeps the
                // climb going rather than ending it.
                match ForLoop::hir(above) {
                    Some(walking) => match was_fixed_when_written(cx, walking.arg) {
                        true => {}
                        false => return true,
                    },
                    None => match above.kind {
                        ExprKind::Loop(_, _, LoopSource::ForLoop, _) => {}
                        ExprKind::Loop(_, _, LoopSource::Loop | LoopSource::While, _) => {
                            return true;
                        }
                        ExprKind::Closure(..) => match walks_a_list(cx, above) {
                            Some(walked) => match was_fixed_when_written(cx, walked) {
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

impl<'tcx> LateLintPass<'tcx> for Explicit029NoAskingPerItem {
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

        if !runs_a_program(named.as_str()) {
            return;
        }

        if !a_loop_stands_over_it(cx, expr) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT029_NO_ASKING_PER_ITEM,
            expr.span,
            format!(
                "`{named}` is called with a loop over it: a program is a fork and an exec, and costs \
                 milliseconds whatever the program is"
            ),
            None,
            "ask once and walk the answer. `console_compositor` will say what every window is in one \
             reply, and one program handed a list is one process. Where the program really has to be \
             run per item, allow this rule at the site and let the reason say why",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
