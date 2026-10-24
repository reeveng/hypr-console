//! EXPLICIT056: a panic is a panic even when it does not look like one.
//!
//! EXPLICIT004 denies the panics that announce themselves -- `unwrap`,
//! `expect`, `panic!` -- and EXPLICIT014 and EXPLICIT015 took the indexing and
//! the arithmetic that panic in disguise. What neither reached is the family
//! of `std` methods whose names say nothing about failure and panic anyway:
//! `Vec::insert`, `Vec::remove` and `Vec::swap` panic on an index that is not
//! in the list, `split_at` and `copy_from_slice` panic on lengths that do not
//! agree, `RefCell::borrow_mut` panics when the value is already held --
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
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::LitKind;
use rustc_hir::{BinOpKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::sym;

dylint_linting::declare_late_lint! {
    /// EXPLICIT056: `Vec::insert`, `Vec::remove`, `Vec::swap`, `split_at`,
    /// `copy_from_slice`, `RefCell::borrow_mut`, `chunks(0)`, `step_by(0)` and
    /// the `Duration` operators panic without saying so. Ask with the form
    /// whose failure is a value, or carry an allow whose reason says why the
    /// panic cannot happen.
    pub EXPLICIT056_NO_HIDDEN_PANIC_API,
    Warn,
    "a std call that panics without looking like it; use the form whose failure is a value"
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

// The zero this rule is named for: a literal `0` handed to a method that
// panics on one. A computed zero is not caught, and that is the honest half of
// the rule -- it is the spelling a person would have noticed.
fn is_a_zero(expr: &Expr<'_>) -> bool {
    match expr.kind {
        ExprKind::Lit(lit) => match lit.node {
            LitKind::Int(held, _) => held.get() == 0,
            _ => false,
        },
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit056NoHiddenPanicApi {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        match expr.kind {
            ExprKind::MethodCall(path, base, args, _) => {
                let name = path.ident.as_str();
                let held = cx.typeck_results().expr_ty(base).peel_refs();

                let harm = match name {
                    "insert" | "remove" | "swap" | "swap_remove" if is_a_list(cx, held) => {
                        Some((
                            format!("`{name}` answers an index that is not in the list with a panic"),
                            "ask with `get`, or reorder so the index is one the list itself handed \
                             over, rather than one a caller could be wrong about",
                        ))
                    }
                    "split_at" | "copy_from_slice" | "swap_with_slice" if is_a_list(cx, held) => {
                        Some((
                            format!("`{name}` answers two lengths that do not agree with a panic"),
                            "`split_at_checked` returns the halves as a value, and a length is \
                             checked once where it is read rather than trusted twice here",
                        ))
                    }
                    "chunks" | "chunks_exact"
                        if is_a_list(cx, held) && args.first().is_some_and(is_a_zero) =>
                    {
                        Some((
                            format!("`{name}` with a zero panics, and the zero is written here"),
                            "the count is a quantity that cannot be zero -- a `NonZeroUsize`, the \
                             same proof division already asks for -- rather than a panic at \
                             runtime",
                        ))
                    }
                    // `step_by` walks what it is given -- an iterator, over a
                    // list or over anything -- so the zero is caught wherever
                    // it is handed one.
                    "step_by" if args.first().is_some_and(is_a_zero) => {
                        Some((
                            "`step_by` with a zero panics, and the zero is written here".to_owned(),
                            "the count is a quantity that cannot be zero -- a `NonZeroUsize`, the \
                             same proof division already asks for -- rather than a panic at \
                             runtime",
                        ))
                    }
                    // The cell is read off the method the call resolved to
                    // rather than off what it was asked of, so a cell behind
                    // an `Rc` or a `Box` answers the same as a bare one.
                    "borrow_mut" if resolves_into(cx, expr, "cell::RefCell") => {
                        Some((
                            "`borrow_mut` answers a value already held with a panic".to_owned(),
                            "`try_borrow_mut` returns the absence instead, which is a thing the \
                             caller can meet rather than a fault they cannot",
                        ))
                    }
                    _ => None,
                };

                if let Some((harm, help)) = harm {
                    span_lint_and_help(
                        cx,
                        EXPLICIT056_NO_HIDDEN_PANIC_API,
                        expr.span,
                        harm,
                        None,
                        help,
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

                let between_instants = is_named(cx, left, "time::Instant")
                    && is_named(cx, right, "time::Instant");

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
