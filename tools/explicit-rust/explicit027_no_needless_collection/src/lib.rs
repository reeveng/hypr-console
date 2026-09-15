//! EXPLICIT027: a collection made only to be walked again.
//!
//! The first rule here about what a line costs rather than about what it says.
//! `.map(…).collect::<Vec<_>>()` followed by `.iter()` allocates a whole list,
//! copies every element into it, walks it once and drops it -- to do what the
//! iterator it was made from was already doing for nothing. The allocation is
//! the only thing on the line that is not written down, which is why it belongs
//! in this suite rather than in a profiler: the cost is implicit, and the fix
//! is to delete the word that hides it.
//!
//! Two shapes, and both are decidable without asking what a borrow does.
//!
//! The first is chained, and it is the one that can never be right: the
//! collection is a temporary, so nothing else can be looking at it. `.collect()
//! .into_iter()` is the pair of words that cancel; `.collect().len()` is
//! `.count()`; `.collect().contains(&x)` is `.any(…)` and stops at the first
//! match instead of visiting the rest of the list for no reason.
//!
//! The second is bound: a `let` whose value is a collection, used exactly once
//! afterwards, and used as something to walk. That is the same waste with a
//! name on it, and it is what a chain looks like after somebody has broken it
//! over two lines to read better. It is asked narrowly, because the same shape
//! is sometimes the whole point -- a list collected so it can be walked twice,
//! or walked inside a loop, is a list that is saving the work rather than
//! wasting it. So a use inside a loop does not count, more than one use does
//! not count, and a `mut` binding does not count. A closure body counts as a
//! loop for the same reason a loop does: nothing here can say how many times it
//! runs, so a use inside one is not a use that happens once.
//!
//! And a walk whose value still points at the list does not count either, which
//! is the one that had to be learned rather than guessed. `let words:
//! Vec<String> = args().collect();` followed by `words.iter().map(String::as_str)
//! .collect::<Vec<&str>>()` is one use, and it is a walk, and the list cannot go
//! anywhere: every `&str` in the answer is a borrow of it. So the chain the use
//! begins is followed to its end and its type is read, and a reference anywhere
//! in that type means the list is what the answer is made of. That is the shape
//! every `fn main` in this tree has, and a rule that shouted at all of them
//! would have been turned off by the second one.
//!
//! Where a collection is what forces the work to happen -- a borrow that has to
//! end before the next line, an iterator walked backwards that cannot be walked
//! backwards where it came from, a side effect that has to be finished by a
//! certain point -- the site carries the allow and EXPLICIT018 asks its reason
//! to say which of those it is. That sentence is the difference between a list
//! somebody meant and a list somebody typed.

//!
//! It arrived `Warn`, with a short distance in front of it: the chained shape is
//! nearly absent and the bound one is a handful of places where a list was given
//! a name on its way past.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{Visitor, walk_expr};
use rustc_hir::{Block, Expr, ExprKind, HirId, Mutability, Node, PatKind, QPath, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::hir::nested_filter::OnlyBodies;
use rustc_middle::ty::{self, GenericArgKind, TyCtxt};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT027: collecting an iterator and then walking what was
    /// collected allocates a list to do what the iterator was already doing.
    /// `.collect::<Vec<_>>().into_iter()` is two words that cancel;
    /// `.collect::<Vec<_>>().len()` is `.count()`. Drop the collection and
    /// keep the chain.
    pub EXPLICIT027_NO_NEEDLESS_COLLECTION,
    Deny,
    "a collection allocated only so that it can be walked once and dropped"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Everything a collection is asked immediately after being made, where the
// iterator it came from answers the same question without the allocation.
fn cancels_a_collection(named: &str) -> Option<&'static str> {
    match named {
        "iter" | "into_iter" | "iter_mut" => Some("walking it is what the iterator was already doing"),
        "len" => Some("`.count()` answers this without the list"),
        "is_empty" => Some("`.next().is_none()` answers this at the first element"),
        "contains" => Some("`.any(…)` answers this and stops at the first match"),
        "first" | "last" | "get" => Some("`.next()`, `.last()` and `.nth(…)` reach an element directly"),
        "into_keys" | "into_values" => Some("walking it is what the iterator was already doing"),
        _ => None,
    }
}

// The narrower list for a collection with a name: only walking counts. A list
// that is asked its length or searched may well be a list somebody wanted.
fn only_walks_it(named: &str) -> bool {
    matches!(named, "iter" | "into_iter" | "iter_mut")
}

fn is_a_collection(expr: &Expr<'_>) -> bool {
    matches!(expr.kind, ExprKind::MethodCall(made, ..) if made.ident.name.as_str() == "collect")
}

// Every mention of one binding in a stretch of code, and whether the mention
// was somewhere that runs more than once. A closure counts as a loop: nothing
// here can say how often it is called, so a use inside one is not a use that
// happens once. Closure bodies are walked into, which is what `OnlyBodies` is
// for -- a use the visitor could not see would be a use the count denies.
struct Mentions<'tcx> {
    tcx: TyCtxt<'tcx>,
    of: HirId,
    found: Vec<&'tcx Expr<'tcx>>,
    more_than_once: bool,
    depth: usize,
}

impl<'tcx> Visitor<'tcx> for Mentions<'tcx> {
    type MaybeTyCtxt = TyCtxt<'tcx>;
    type NestedFilter = OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            ExprKind::Loop(..) | ExprKind::Closure(..) => {
                self.depth = self.depth.saturating_add(1);
                walk_expr(self, expr);
                self.depth = self.depth.saturating_sub(1);
                return;
            }
            ExprKind::Path(QPath::Resolved(None, path)) => match path.res {
                Res::Local(named) => match named == self.of {
                    true => {
                        self.found.push(expr);

                        match self.depth {
                            0 => {}
                            _ => self.more_than_once = true,
                        }
                    }
                    false => {}
                },
                _ => {}
            },
            _ => {}
        }

        walk_expr(self, expr);
    }
}

// The far end of the chain a use begins: `held` in `held.iter().map(…).collect()`
// walks up to the `collect`, which is the value the whole line comes to.
fn what_the_walk_comes_to<'tcx>(cx: &LateContext<'tcx>, from: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    let mut at = from;
    let mut left = 32usize;

    while left > 0 {
        left = left.saturating_sub(1);

        let Node::Expr(above) = cx.tcx.parent_hir_node(at.hir_id) else {
            return at;
        };

        let ExprKind::MethodCall(_, taken, ..) = above.kind else {
            return at;
        };

        match taken.hir_id == at.hir_id {
            true => at = above,
            false => return at,
        }
    }

    at
}

// Whether what the walk came to is still made of the list. A reference anywhere
// in the type means the answer borrows, and a list something is borrowing from
// is a list that has to exist.
fn still_points_at_it<'tcx>(cx: &LateContext<'tcx>, comes_to: &'tcx Expr<'tcx>) -> bool {
    cx.typeck_results()
        .expr_ty(comes_to)
        .walk()
        .any(|part| match part.kind() {
            GenericArgKind::Type(part) => matches!(part.kind(), ty::Ref(..)),
            GenericArgKind::Lifetime(_) => true,
            GenericArgKind::Const(_) => false,
        })
}

// Said against the node rather than only against the span, so that an `#[allow]`
// on the statement is the level this is read at. A block is what the second half
// of this rule walks, and a block is not where anybody would write the allow.
fn say(cx: &LateContext<'_>, of: HirId, at: Span, what: &str, instead: &str) {
    span_lint_hir_and_then(
        cx,
        EXPLICIT027_NO_NEEDLESS_COLLECTION,
        of,
        at,
        format!("this collection is allocated and then {what}"),
        |said| {
            said.help(format!(
                "{instead}. Where the collection is what forces the work to happen -- a borrow that has \
                 to end, an order that has to be settled, a side effect that has to be finished by here \
                 -- allow this rule at the site and let the reason say which"
            ));
        },
    );
}

impl<'tcx> LateLintPass<'tcx> for Explicit027NoNeedlessCollection {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::MethodCall(asked, taken, ..) = expr.kind else {
            return;
        };

        if !is_a_collection(taken) {
            return;
        }

        let Some(instead) = cancels_a_collection(asked.ident.name.as_str()) else {
            return;
        };

        let asked = asked.ident.name;

        say(cx, expr.hir_id, expr.span, &format!("`{asked}` is asked of it straight away"), instead);
    }

    fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx Block<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        for (at, stmt) in block.stmts.iter().enumerate() {
            let StmtKind::Let(binding) = stmt.kind else {
                continue;
            };

            if stmt.span.from_expansion() {
                continue;
            }

            let PatKind::Binding(mode, named, _, None) = binding.pat.kind else {
                continue;
            };

            if mode.1 != Mutability::Not {
                continue;
            }

            let Some(made) = binding.init else {
                continue;
            };

            if !is_a_collection(made) {
                continue;
            }

            let mut mentions = Mentions {
                tcx: cx.tcx,
                of: named,
                found: Vec::new(),
                more_than_once: false,
                depth: 0,
            };

            let Some(after) = block.stmts.get(at.saturating_add(1)..) else {
                continue;
            };

            for later in after {
                mentions.visit_stmt(later);
            }

            match block.expr {
                Some(tail) => mentions.visit_expr(tail),
                None => {}
            }

            if mentions.more_than_once {
                continue;
            }

            let [only] = mentions.found.as_slice() else {
                continue;
            };

            let Node::Expr(walked) = cx.tcx.parent_hir_node(only.hir_id) else {
                continue;
            };

            let ExprKind::MethodCall(asked, taken, ..) = walked.kind else {
                continue;
            };

            if taken.hir_id != only.hir_id {
                continue;
            }

            if !only_walks_it(asked.ident.name.as_str()) {
                continue;
            }

            if still_points_at_it(cx, what_the_walk_comes_to(cx, only)) {
                continue;
            }

            say(
                cx,
                made.hir_id,
                made.span,
                "it is walked once and dropped, and nothing else ever looks at it",
                "hand the iterator on rather than the list: the chain that made this can be the chain \
                 that walks it",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
