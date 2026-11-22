//! EXPLICIT090: a descriptor is owned or lent, and is never a number.
//!
//! A `RawFd` is an `i32`, and the kernel hands the same small numbers out
//! again the moment one is closed. So a descriptor kept as a number is a
//! pointer with no borrow checker behind it: closed once by whoever made it
//! and once more by whoever was handed the number, and the second `close`
//! lands on whatever the process opened in between -- the bus, the lock, a
//! file being written. Read or written after its owner let go, it is a read
//! of somebody else's file. Neither fault fails where it is made, and both
//! are the use-after-free this language was chosen not to have, come back
//! through the one type it could not see into.
//!
//! The standard library has had the answer since it named it I/O safety.
//! `OwnedFd` closes when it is dropped and at no other time; `BorrowedFd` is a
//! loan with a lifetime on it; `AsFd` hands one out. `rustix` takes and
//! returns nothing else, which is half of why EXPLICIT052 asks for it. What
//! this rule denies is the way back to the number: `as_raw_fd`, which hands
//! out a copy with no lifetime, `into_raw_fd`, which gives up the owner, and
//! `from_raw_fd`, which declares one out of an integer -- and `RawFd` written
//! as a type, which is where a number of that kind is kept.
//!
//! The door it leaves is the usual one. A descriptor crosses an `extern`
//! signature or a protocol as a number because that is what the other side
//! reads, and the site that does it carries the allow and says who is on the
//! other side.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{AmbigArg, Expr, ExprKind, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT090: no `RawFd`, `as_raw_fd`, `into_raw_fd` or `from_raw_fd`.
    /// A descriptor is an `OwnedFd`, or a `BorrowedFd` handed out by `AsFd`.
    pub EXPLICIT090_NO_RAW_DESCRIPTOR,
    Warn,
    "a descriptor held as a number can be closed twice and used after it is gone"
}

fn becomes_a_number(named: &str) -> Option<&'static str> {
    match named {
        "as_raw_fd" => Some("hands out the number with no lifetime on it, so nothing stops it outliving the descriptor"),
        "into_raw_fd" => Some("gives up the owner, and nothing closes what it held"),
        "from_raw_fd" => Some("declares an owner out of a number, whoever else already owns it"),
        _ => None,
    }
}

const HELP: &str = "hold an `OwnedFd`, lend a `BorrowedFd` through `AsFd`, and hand `rustix` either. Where \
                    the number is what the other side of an `extern` signature or a protocol reads, allow \
                    this rule here and let the reason say who that is";

impl<'tcx> LateLintPass<'tcx> for Explicit090NoRawDescriptor {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let id = match expr.kind {
            ExprKind::Call(called, _) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id)
                else {
                    return;
                };

                id
            }
            ExprKind::MethodCall(..) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                id
            }
            _ => return,
        };

        // The names are the standard library's traits', whoever implements
        // them; a type of this tree's that answers `as_raw_fd` is answering
        // `AsRawFd`.
        let named = cx.tcx.item_name(id);

        let Some(harm) = becomes_a_number(named.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT090_NO_RAW_DESCRIPTOR,
            expr.span,
            format!("`{named}` {harm}"),
            None,
            HELP,
        );
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, written: &'tcx Ty<'tcx, AmbigArg>) {
        if written.span.from_expansion() {
            return;
        }

        let TyKind::Path(QPath::Resolved(_, path)) = written.kind else {
            return;
        };

        let Res::Def(DefKind::TyAlias, id) = path.res else {
            return;
        };

        if !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "std") || cx.tcx.item_name(id).as_str() != "RawFd" {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT090_NO_RAW_DESCRIPTOR,
            written.span,
            "`RawFd` is a descriptor kept as a number, which the kernel hands out again once it is closed",
            None,
            HELP,
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
