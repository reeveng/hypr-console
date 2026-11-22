//! EXPLICIT056: a panic is a panic even when it does not look like one.
//!
//! EXPLICIT004 denies the panics that announce themselves -- `unwrap`,
//! `expect`, `panic!` -- and EXPLICIT014 and EXPLICIT015 took the indexing and
//! the arithmetic that panic in disguise. What neither reached is the family
//! of `std` methods whose names say nothing about failure and panic anyway:
//! `Vec::insert`, `Vec::remove` and `Vec::swap` panic on an index that is not
//! in the list, `split_at` and `copy_from_slice` panic on lengths that do not
//! agree, `RefCell::borrow` and `borrow_mut` panic on a conflicting borrow --
//! wherever the cell is reached from, an `Rc` included -- and a `Duration`
//! under `+` or `-`, or one added to or taken from an `Instant`, panics on
//! overflow exactly the way an integer does in a debug build. One `Instant`
//! taken from another does not: it saturates to zero, and the answer is a
//! value. `chunks(0)` and `step_by(0)` panic on a zero that a type could have
//! refused.
//!
//! The spelling this rule asks for is the one whose failure is a value:
//! `split_at_checked`, `try_borrow_mut`, `checked_add` and `checked_sub`, and
//! -- where no form that cannot fail exists -- `get`, which is the door
//! EXPLICIT014 already opened. Where the panic really cannot happen, the site
//! carries the allow and its reason says so, which is this suite's one door.
//! A fixed array can bind two known slots and swap them directly. A shared
//! slice split clamped to its length can name the end-of-slice fallback.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::consts::{ConstEvalCtxt, Constant};
use clippy_utils::diagnostics::{span_lint_and_help, span_lint_and_then};
use clippy_utils::sym;
use rustc_errors::Applicability;
use rustc_hir::{BinOpKind, Expr, ExprKind, def::Res};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};

mod endian_copy;
mod fixes;
mod paired_copy;

dylint_linting::impl_late_lint! {
    /// EXPLICIT056: `Vec::insert`, `Vec::remove`, `Vec::swap`, `split_at`,
    /// `copy_from_slice`, conflicting `RefCell` borrows, zero chunk widths,
    /// wrapping division by zero, and the `Duration` operators can panic. Ask with the form
    /// whose failure is a value, or carry an allow whose reason says why the
    /// panic cannot happen.
    pub EXPLICIT056_NO_HIDDEN_PANIC_API,
    Warn,
    "a std call that panics without looking like it; use the form whose failure is a value",
    Explicit056NoHiddenPanicApi {
        checked_endian_copies: dylint_linting::config_or_default("explicit056_checked_endian_copies"),
        checked_paired_copies: dylint_linting::config_or_default("explicit056_checked_paired_copies"),
    }
}

pub struct Explicit056NoHiddenPanicApi {
    checked_endian_copies: bool,
    checked_paired_copies: bool,
}

// A `Vec`, a slice or an array, by type rather than by spelling, so an alias
// and a reference answer the same as the bare thing.
fn is_a_list(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.kind() {
        ty::Slice(_) | ty::Array(_, _) => true,
        ty::Adt(adt, _) => cx.tcx.is_diagnostic_item(sym::Vec, adt.did()),
        _ => false,
    }
}

// `RefCell`, `Duration` and `Instant`, read off the resolved type so a `use`
// or an alias does not slip past. They arrive through the `std::` re-export
// rather than `core`, so the last two segments are the key.
fn is_named(cx: &LateContext<'_>, ty: Ty<'_>, path: &str) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => cx.tcx.def_path_str(adt.did()).ends_with(path),
        _ => false,
    }
}

fn resolves_into(cx: &LateContext<'_>, expr: &Expr<'_>, path: &str) -> bool {
    cx.typeck_results()
        .type_dependent_def_id(expr.hir_id)
        .is_some_and(|called| cx.tcx.def_path_str(called).contains(path))
}

fn proven_nonzero(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let expr = clippy_utils::expr_or_init(cx, expr);
    if let Some(Constant::Int(value)) = ConstEvalCtxt::new(cx).eval(expr) {
        return value != 0;
    }
    if integer_with_nonzero_bound(cx, expr) {
        return true;
    }

    let (called, receiver) = match expr.kind {
        ExprKind::MethodCall(_, receiver, [], _) => (
            cx.typeck_results().type_dependent_def_id(expr.hir_id),
            receiver,
        ),
        ExprKind::Call(callee, [receiver]) => {
            let ExprKind::Path(ref path) = callee.kind else {
                return false;
            };
            (cx.qpath_res(path, callee.hir_id).opt_def_id(), receiver)
        }
        _ => return false,
    };
    if !called.is_some_and(|id| {
        cx.tcx.item_name(id).as_str() == "get" && cx.tcx.crate_name(id.krate).as_str() == "core"
    }) {
        return false;
    }

    matches!(cx.typeck_results().expr_ty(receiver).peel_refs().kind(), ty::Adt(adt, _)
        if cx.tcx.is_diagnostic_item(sym::NonZero, adt.did()))
}

fn integer_with_nonzero_bound(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let types = cx.typeck_results();
    let integer = types.expr_ty(expr);
    if !integer.is_integral() {
        return false;
    }

    let (called, left, right) = match expr.kind {
        ExprKind::MethodCall(_, left, [right], _) => {
            (types.type_dependent_def_id(expr.hir_id), left, right)
        }
        ExprKind::Call(callee, [left, right]) => match types.expr_ty(callee).kind() {
            ty::FnDef(id, _) => (Some(*id), left, right),
            _ => return false,
        },
        _ => return false,
    };
    let Some(called) = called else { return false };
    let inherent_integer_method = cx.tcx.crate_name(called.krate).as_str() == "core"
        && cx.tcx.inherent_impl_of_assoc(called).is_some_and(|id|
            cx.tcx.type_of(id).instantiate_identity().skip_norm_wip() == integer);
    let constants = ConstEvalCtxt::new(cx);
    let method = cx.tcx.item_name(called);
    if inherent_integer_method && method.as_str() == "wrapping_shl" {
        // The shift is reduced modulo the bit width. The low bit of an odd
        // input therefore lands somewhere in the result for every shift.
        return matches!(constants.eval(left), Some(Constant::Int(value)) if value & 1 == 1);
    }
    if !matches!(integer.kind(), ty::Uint(_)) {
        return false;
    }
    let maximum = cx.tcx.is_diagnostic_item(sym::cmp_max, called)
        || (method == sym::max
            && cx
                .tcx
                .trait_of_assoc(called)
                .is_some_and(|which| cx.tcx.is_diagnostic_item(sym::Ord, which)));
    let nonzero = |operand| matches!(constants.eval(operand), Some(Constant::Int(value)) if value != 0);
    if inherent_integer_method && method.as_str() == "saturating_mul" {
        return nonzero(left) && nonzero(right);
    }
    let saturating_sum = inherent_integer_method && method.as_str() == "saturating_add";
    (maximum || saturating_sum) && (nonzero(left) || nonzero(right))
}

fn fixed_array_copy(
    cx: &LateContext<'_>,
    call: &Expr<'_>,
    into: &Expr<'_>,
    from: &Expr<'_>,
) -> Option<String> {
    let types = cx.typeck_results();
    let (into_array, dereference_into) = match types.expr_ty(into).kind() {
        ty::Ref(_, array, ty::Mutability::Mut) => (*array, "*"),
        _ => (types.expr_ty(into), ""),
    };
    let ty::Ref(_, from_array, _) = types.expr_ty(from).kind() else {
        return None;
    };
    let ty::Array(element, _) = into_array.kind() else {
        return None;
    };
    // Assignment need not preserve padding that a slice copy can carry.
    if into_array != *from_array || !element.is_primitive() {
        return None;
    }
    let method = types.type_dependent_def_id(call.hir_id)?;
    let implementation = cx.tcx.inherent_impl_of_assoc(method)?;
    if cx.tcx.crate_name(method.krate).as_str() != "core"
        || !cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .skip_norm_wip()
            .is_slice()
    {
        return None;
    }

    // Use only locals and explicit borrows of locals. Assignment reads the source first, whereas
    // the method reads the destination first. These reads have no effects.
    let map = cx.sess().source_map();
    let local = |expr: &Expr<'_>| {
        if !expr.span.from_expansion()
            && matches!(expr.kind, ExprKind::Path(ref path) if matches!(cx.qpath_res(path, expr.hir_id), Res::Local(_)))
        {
            map.span_to_snippet(expr.span).ok()
        } else {
            None
        }
    };
    let text = map.span_to_snippet(call.span).ok()?;
    if call.span.from_expansion() || text.contains("//") || text.contains("/*") {
        return None;
    }
    let into = local(into)?;
    let from = match from.kind {
        ExprKind::AddrOf(rustc_hir::BorrowKind::Ref, _, place) => local(place)?,
        _ => format!("*{}", local(from)?),
    };
    let assignment = format!("{dereference_into}{into} = {from}");
    Some(
        if matches!(
            cx.tcx.parent_hir_node(call.hir_id),
            rustc_hir::Node::Stmt(_)
        ) {
            assignment
        } else {
            format!("({assignment})")
        },
    )
}

fn panicking_division(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let (id, divisor) = match expr.kind {
        ExprKind::MethodCall(_, _, [divisor], _) => (
            cx.typeck_results().type_dependent_def_id(expr.hir_id),
            divisor,
        ),
        ExprKind::Call(callee, [_, divisor]) => {
            let ExprKind::Path(ref path) = callee.kind else {
                return false;
            };
            (cx.qpath_res(path, callee.hir_id).opt_def_id(), divisor)
        }
        _ => return false,
    };
    let Some(id) = id else { return false };
    matches!(
        cx.tcx.item_name(id).as_str(),
        "wrapping_div" | "wrapping_rem" | "wrapping_div_euclid" | "wrapping_rem_euclid"
    ) && cx.tcx.crate_name(id.krate).as_str() == "core"
        && !proven_nonzero(cx, divisor)
}

impl<'tcx> LateLintPass<'tcx> for Explicit056NoHiddenPanicApi {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        if panicking_division(cx, expr) {
            span_lint_and_help(
                cx,
                EXPLICIT056_NO_HIDDEN_PANIC_API,
                expr.span,
                "wrapping division or remainder still panics on a zero divisor",
                None,
                "use the checked method, or prove that the divisor cannot be zero",
            );
            return;
        }

        match expr.kind {
            ExprKind::MethodCall(path, base, args, _) => {
                let name = path.ident.as_str();
                let held = cx.typeck_results().expr_ty(base).peel_refs();

                let harm = match name {
                    // The insertion point at the front exists even in an empty Vec.
                    "insert" if matches!(held.kind(), ty::Adt(adt, _) if cx.tcx.is_diagnostic_item(sym::Vec, adt.did()))
                        && args.first().is_some_and(|index| matches!(ConstEvalCtxt::new(cx).eval(index), Some(Constant::Int(0))))
                        && cx.typeck_results().type_dependent_def_id(expr.hir_id)
                            .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
                            .is_some_and(|id| matches!(cx.tcx.type_of(id).instantiate_identity().skip_norm_wip().kind(),
                                ty::Adt(adt, _) if cx.tcx.is_diagnostic_item(sym::Vec, adt.did()))) => None,
                    "insert" | "remove" | "swap" | "swap_remove" if is_a_list(cx, held) => Some((
                        format!("`{name}` answers an index that is not in the list with a panic"),
                        "ask with `get`, or reorder so the index is one the list itself handed \
                             over, rather than one a caller could be wrong about",
                    )),
                    "split_at" | "copy_from_slice" | "swap_with_slice" if is_a_list(cx, held) => {
                        Some((
                            format!("`{name}` answers two lengths that do not agree with a panic"),
                            "`split_at_checked` returns the halves as a value, and a length is \
                             checked once where it is read rather than trusted twice here",
                        ))
                    }
                    "split_at" | "split_at_mut" if held.is_str() => Some((
                        format!("`{name}` panics on an invalid UTF-8 split point"),
                        "use the checked string split method and handle an invalid byte index",
                    )),
                    "chunks" | "chunks_exact" | "chunks_mut" | "chunks_exact_mut" | "rchunks"
                    | "rchunks_mut" | "rchunks_exact" | "rchunks_exact_mut" | "windows"
                        if is_a_list(cx, held)
                            && args.first().is_some_and(|count| !proven_nonzero(cx, count)) =>
                    {
                        Some((
                            format!("`{name}` panics on zero; this count is not proven nonzero"),
                            "the count is a quantity that cannot be zero -- a `NonZeroUsize`, the \
                             same proof division already asks for -- rather than a panic at \
                             runtime",
                        ))
                    }
                    // `step_by` walks what it is given -- an iterator, over a
                    // list or over anything -- so the zero is caught wherever
                    // it is handed one.
                    "step_by"
                        if cx
                            .typeck_results()
                            .type_dependent_def_id(expr.hir_id)
                            .and_then(|id| cx.tcx.trait_of_assoc(id))
                            .is_some_and(|id| cx.tcx.is_diagnostic_item(sym::Iterator, id))
                            && args.first().is_some_and(|count| !proven_nonzero(cx, count)) =>
                    {
                        Some((
                            "`step_by` panics on zero; this count is not proven nonzero".to_owned(),
                            "the count is a quantity that cannot be zero -- a `NonZeroUsize`, the \
                             same proof division already asks for -- rather than a panic at \
                             runtime",
                        ))
                    }
                    // The cell is read off the method the call resolved to
                    // rather than off what it was asked of, so a cell behind
                    // an `Rc` or a `Box` answers the same as a bare one.
                    "borrow" | "borrow_mut" if resolves_into(cx, expr, "cell::RefCell") => Some((
                        format!("`{name}` panics when the cell has a conflicting borrow"),
                        "use `try_borrow` or `try_borrow_mut` to handle a conflicting borrow",
                    )),
                    _ => None,
                };

                if let Some((harm, help)) = harm {
                    span_lint_and_then(
                        cx,
                        EXPLICIT056_NO_HIDDEN_PANIC_API,
                        expr.span,
                        harm,
                        |diagnostic| {
                            let suggestion = match (name, args) {
                                ("copy_from_slice", [from]) => fixed_array_copy(cx, expr, base, from)
                                    .map(|replacement| ("copy the arrays directly because their lengths agree", replacement))
                                    .or_else(|| {
                                        if self.checked_endian_copies {
                                            endian_copy::replacement(cx, expr, base, from)
                                                .map(|replacement| ("check the destination as a fixed byte array before copying", replacement))
                                        } else {
                                            None
                                        }
                                    })
                                    .or_else(|| {
                                        if self.checked_paired_copies {
                                            paired_copy::replacement(cx, expr, base, from)
                                                .map(|replacement| ("copy between ranges whose checks establish the same length", replacement))
                                        } else {
                                            None
                                        }
                                    }),
                                ("swap", [left, right]) => fixes::fixed_array_swap(cx, expr, base, left, right)
                                    .map(|replacement| ("swap the two slots proven to exist", replacement)),
                                ("split_at", [index]) => fixes::clamped_slice_split(cx, expr, base, index)
                                    .map(|replacement| ("handle a split past the end as a split at the end", replacement)),
                                _ => None,
                            };
                            if let Some((message, replacement)) = suggestion {
                                diagnostic.span_suggestion(
                                    expr.span,
                                    message,
                                    replacement,
                                    Applicability::MachineApplicable,
                                );
                            } else {
                                diagnostic.help(help);
                            }
                        },
                    );
                }
            }

            // A `Duration` under `+` or `-` is EXPLICIT015's fault wearing a
            // type that makes it look safe: the overflow is answered by a
            // panic in every profile, not only in a debug build. The one
            // operator over these types that cannot panic is an `Instant`
            // taken from an `Instant`, which saturates to a zero `Duration`.
            ExprKind::Binary(op, lhs, rhs)
                if matches!(op.node, BinOpKind::Add | BinOpKind::Sub) =>
            {
                let left = cx.typeck_results().expr_ty(lhs).peel_refs();
                let right = cx.typeck_results().expr_ty(rhs).peel_refs();

                let timed = is_named(cx, left, "time::Duration")
                    || is_named(cx, left, "time::Instant")
                    || is_named(cx, right, "time::Duration")
                    || is_named(cx, right, "time::Instant");

                let between_instants =
                    is_named(cx, left, "time::Instant") && is_named(cx, right, "time::Instant");

                if timed && !between_instants {
                    span_lint_and_help(
                        cx,
                        EXPLICIT056_NO_HIDDEN_PANIC_API,
                        expr.span,
                        "adding or subtracting a `Duration` or an `Instant` panics on overflow",
                        None,
                        "`checked_add` and `checked_sub` return the absence instead, which is the \
                         answer every other quantity in this tree already gives",
                    );
                }
            }

            _ => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}

#[test]
fn fixed_lengths() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn checked_endian_copies() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_endian")
        .dylint_toml("explicit056_checked_endian_copies = true")
        .run();
}

#[test]
fn checked_paired_copies() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_paired")
        .dylint_toml("explicit056_checked_paired_copies = true")
        .run();
}
