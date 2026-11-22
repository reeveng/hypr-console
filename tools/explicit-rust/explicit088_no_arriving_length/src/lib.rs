//! EXPLICIT088: a buffer is not sized by a number that arrived.
//!
//! `Vec::with_capacity(length)` and `vec![0; length]` ask the allocator for
//! whatever `length` says, at once, before a byte of what is meant to fill it
//! has been seen. Where `length` is the length of something already held,
//! that is a courtesy. Where it was read out of a header -- four bytes on a
//! bus, a field in an archive's directory, a count in front of a list -- it
//! is somebody else's number, and four bytes are enough to ask for four
//! gigabytes. The process either dies of it or, on a machine that overcommits,
//! is killed later by something that cannot say why. EXPLICIT087 is the same
//! fault met a byte at a time; this is the one that needs no patience.
//!
//! What the rule can read is where the number came from, as far as the
//! function it is in. A literal and a `const` were written. A standard container's
//! `len()` or `capacity()` measures something already in memory. Counting
//! a standard slice iterator after `filter` is bounded by that slice,
//! and arithmetic over those is still a measurement. A number passed through
//! `min` or `clamp` beside one of those has a ceiling somebody wrote, which
//! is the fix: `declared.min(THE_MOST_A_MESSAGE_IS)`, or better a refusal
//! when the declared length is over it, and then a buffer that grows as the
//! bytes really arrive. Anything else -- a parameter, a field, a value
//! decoded a line above -- is a length the rule cannot find the end of.
//!
//! It is the cost argument the rules after 026 make, with an adversary in it:
//! a price nobody wrote down, set by whoever sent the message.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, Mutability, Node, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use clippy_utils::sym;

dylint_linting::declare_late_lint! {
    /// EXPLICIT088: allocation, reservation and resizing calls
    /// take a length that was written, measured off something already held,
    /// or passed under a ceiling. Not one that arrived.
    pub EXPLICIT088_NO_ARRIVING_LENGTH,
    Warn,
    "a buffer sized by a number that arrived is a heap the size of somebody else's choosing"
}

fn in_memory_measure(cx: &LateContext<'_>, receiver: &Expr<'_>, method: &str) -> bool {
    match cx.typeck_results().expr_ty_adjusted(receiver).peel_refs().kind() {
        ty::Slice(_) | ty::Array(..) | ty::Str => method == "len",
        ty::Adt(adt, _) => {
            matches!(method, "len" | "capacity")
                && (cx.tcx.is_diagnostic_item(sym::Vec, adt.did())
                    || cx.tcx.is_diagnostic_item(sym::String, adt.did()))
        }
        _ => false,
    }
}

fn filtered_slice(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    let ExprKind::MethodCall(_, input, [_], _) = receiver.kind else {
        return false;
    };
    let types = cx.typeck_results();
    let ty::Adt(iterator, _) = types.expr_ty(input).kind() else {
        return false;
    };
    cx.tcx.is_diagnostic_item(sym::SliceIter, iterator.did())
        && types.type_dependent_def_id(receiver.hir_id).is_some_and(|filter| {
            cx.tcx.item_name(filter).as_str() == "filter"
                && cx.tcx.trait_of_assoc(filter).is_some_and(|item| cx.tcx.is_diagnostic_item(sym::Iterator, item))
        })
}

// How far a binding is followed back to what it was made from. A length that
// passes through more names than this before it is used is one a reader has
// lost as well.
const AS_FAR_AS_A_READER_FOLLOWS: u8 = 4;

fn is_bounded(cx: &LateContext<'_>, length: &Expr<'_>, left: u8) -> bool {
    match length.kind {
        ExprKind::Lit(_) | ExprKind::ConstBlock(_) => true,
        ExprKind::AddrOf(_, _, inner) | ExprKind::Cast(inner, _) | ExprKind::Unary(_, inner) => {
            is_bounded(cx, inner, left)
        }
        ExprKind::Binary(_, one, other) => is_bounded(cx, one, left) && is_bounded(cx, other, left),
        ExprKind::Path(ref qpath) => match cx.qpath_res(qpath, length.hir_id) {
            Res::Def(
                DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::ConstParam,
                _,
            ) => true,
            Res::Local(binding) if left > 0 => match cx.tcx.parent_hir_node(binding) {
                Node::LetStmt(made) if matches!(made.pat.kind, PatKind::Binding(mode, ..) if mode.1 == Mutability::Not) => {
                    made.init.is_some_and(|from| is_bounded(cx, from, left - 1))
                }
                _ => false,
            },
            _ => false,
        },
        ExprKind::MethodCall(_, receiver, given, _) => {
            let Some(id) = cx.typeck_results().type_dependent_def_id(length.hir_id) else {
                return false;
            };
            if !matches!(
                cx.tcx.crate_name(id.krate).as_str(),
                "core" | "alloc" | "std"
            ) {
                return false;
            }

            let method = cx.tcx.item_name(id);
            if in_memory_measure(cx, receiver, method.as_str())
                || (method.as_str() == "count"
                    && given.is_empty()
                    && cx.tcx.trait_of_assoc(id).is_some_and(|item| cx.tcx.is_diagnostic_item(sym::Iterator, item))
                    && filtered_slice(cx, receiver))
            {
                return true;
            }
            if !cx.typeck_results().expr_ty(receiver).is_integral() {
                return false;
            }

            match (method.as_str(), given) {
                ("min", [other]) => is_bounded(cx, receiver, left) || is_bounded(cx, other, left),
                ("clamp", [_, upper]) => is_bounded(cx, upper, left),
                (
                    "saturating_add" | "saturating_sub" | "saturating_mul" | "saturating_div"
                    | "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "wrapping_div"
                    | "wrapping_rem" | "div_ceil",
                    [other],
                ) => is_bounded(cx, receiver, left) && is_bounded(cx, other, left),
                _ => false,
            }
        }
        ExprKind::Field(..) | ExprKind::Tup(..) => false,
        _ => false,
    }
}

// The receiver takes the first argument slot in associated-function syntax.
fn the_length<'tcx>(
    cx: &LateContext<'_>,
    expr: &'tcx Expr<'tcx>,
) -> Option<(&'static str, &'tcx Expr<'tcx>)> {
    let (id, arguments, receiver_slots) = match expr.kind {
        ExprKind::Call(callee, arguments) => {
            let ExprKind::Path(ref path) = callee.kind else {
                return None;
            };
            let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(path, callee.hir_id)
            else {
                return None;
            };
            (id, arguments, 1)
        }
        ExprKind::MethodCall(_, _, arguments, _) => (
            cx.typeck_results().type_dependent_def_id(expr.hir_id)?,
            arguments,
            0,
        ),
        _ => return None,
    };
    if !matches!(
        cx.tcx.crate_name(id.krate).as_str(),
        "core" | "alloc" | "std"
    ) {
        return None;
    }
    let (name, index) = match cx.tcx.item_name(id).as_str() {
        "with_capacity" => ("with_capacity", 0),
        "from_elem" => ("vec![_; n]", 1),
        "reserve" => ("reserve", receiver_slots),
        "reserve_exact" => ("reserve_exact", receiver_slots),
        "try_reserve" => ("try_reserve", receiver_slots),
        "try_reserve_exact" => ("try_reserve_exact", receiver_slots),
        "resize" => ("resize", receiver_slots),
        "resize_with" => ("resize_with", receiver_slots),
        _ => return None,
    };
    Some((name, arguments.get(index)?))
}

impl<'tcx> LateLintPass<'tcx> for Explicit088NoArrivingLength {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let Some((named, length)) = the_length(cx, expr) else {
            return;
        };

        if is_bounded(cx, length, AS_FAR_AS_A_READER_FOLLOWS) {
            return;
        }

        // `vec![x; n]` is a call a macro wrote around a length a person did.
        let shown = match expr.span.from_expansion() {
            true => expr.span.source_callsite(),
            false => expr.span,
        };

        span_lint_and_help(
            cx,
            EXPLICIT088_NO_ARRIVING_LENGTH,
            shown,
            format!(
                "`{named}` is handed a length nothing here wrote, measured or put a ceiling on"
            ),
            None,
            "refuse a declared length over a `const` that says what the most is, or pass it through \
             `min` beside one, and let the buffer grow as the bytes really arrive. Where the number \
             was bounded before it was handed in, allow this rule here and let the reason say where",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
