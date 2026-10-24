//! EXPLICIT081: a value has one owner.
//!
//! An `Arc` is a value nobody owns. Every holder keeps it alive and none of
//! them decides when it ends: it is freed by whichever lets go last, on
//! whatever thread that holder is running on, at a moment no line of the
//! source names, and its `Drop` -- a file closed, a socket shut, a frame's
//! bytes handed back -- runs there too. Two runs of one program can end one
//! value on two threads in two orders, which is the determinism 039 and 077
//! were written to keep, lost in a type rather than in a call. It is the cost
//! argument the rules after 026 make as well: every clone and every drop is an
//! atomic read-modify-write on a count that every holder's core shares, and the
//! price is nowhere on the line that pays it.
//!
//! `Arc<Mutex<T>>` is where the tree spells it most, and that is 068's global
//! handed to everybody instead of to nobody: state any holder may write, so a
//! function holding one decides from an input no call can see the value of. A
//! proof about a value starts from who can write it, and Rust's answer is the
//! owner and the one `&mut`; a count of holders with a lock inside is the
//! answer argued at runtime instead. A cycle of them is never freed, and
//! nothing in the language looks for one.
//!
//! What the rule asks for is one of the three spellings that keep the owner.
//! Move the value to the one place that uses it. Lend it, where
//! `std::thread::scope` lends across threads for as long as the scope and no
//! longer. Or keep it behind an actor (`console-actor`) that owns it and
//! answers requests, which is the shape a panel's handlers are already moving
//! toward (`docs/programs.md`): a button that sends its machine a request
//! rather than holding a closure everybody holds. `Rc` is the same question on
//! one thread, and the `Weak` of either is a handle to a value nobody owns that
//! may already be gone, so the four are asked together -- an `Arc` denied alone
//! is an `Rc` written instead. JPL's coding standard for flight software says
//! the same as its eighth rule, about C: a data object in shared memory has a
//! single owning task, and ownership passes between tasks explicitly,
//! preferably by message.
//!
//! What is read is the type as it is written -- a field, a signature, an
//! alias, a binding, the `Arc` in `Arc::new` and `Arc::clone` -- and a
//! `.clone()` of one, which makes another owner with no type at the call to
//! say so. The type is known by the standard library's own name for it rather
//! than by its spelling, so a type of this tree's that happens to be called
//! `Arc` is left alone. A `use` is not a site, and an alias is read where it is
//! declared rather than at every use, because that is where it was decided.
//!
//! It arrives `Warn`, across the panel's handlers and the threads that share a
//! `Mutex`, and `just explicit` prints the distance.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::Res;
use rustc_hir::{AmbigArg, Expr, ExprKind, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::def_id::DefId;
use rustc_span::sym;

dylint_linting::declare_late_lint! {
    /// EXPLICIT081: no `Arc`, no `Rc`, and no `Weak` of either. A value has
    /// one owner: move it, lend it, or keep it behind an actor that answers.
    pub EXPLICIT081_NO_SHARED_OWNERSHIP,
    Warn,
    "a value nobody owns, freed by whichever holder lets go last; give it one owner"
}

const HELP_TEXT: &str = "give it one owner: move it to the one place that uses it, lend it with a borrow \
     (`std::thread::scope` lends across threads), or keep it behind an actor (`console-actor`) \
     that owns it and answers requests";

fn owned_by_nobody(cx: &LateContext<'_>, definition: DefId) -> Option<&'static str> {
    match cx.tcx.get_diagnostic_name(definition)? {
        sym::Arc => Some(
            "an `Arc` is a value nobody owns: whichever holder lets go last frees it, on whatever \
             thread that holder is on",
        ),
        sym::Rc => Some("an `Rc` is a value nobody owns: whichever holder lets go last frees it"),
        sym::ArcWeak | sym::RcWeak => {
            Some("a `Weak` is a handle to a value nobody owns, which may already be gone when it is asked")
        }
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit081NoSharedOwnership {
    fn check_ty(&mut self, cx: &LateContext<'tcx>, written: &'tcx Ty<'tcx, AmbigArg>) {
        let written: &Ty<'tcx> = written.as_unambig_ty();

        if written.span.from_expansion() {
            return;
        }

        let TyKind::Path(QPath::Resolved(_, path)) = written.kind else {
            return;
        };

        let Res::Def(_, definition) = path.res else {
            return;
        };

        let Some(what) = owned_by_nobody(cx, definition) else {
            return;
        };

        span_lint_and_help(cx, EXPLICIT081_NO_SHARED_OWNERSHIP, written.span, what, None, HELP_TEXT);
    }

    // A `.clone()` names no type, so the owner it makes is found by what the
    // call returns rather than by what it is spelled.
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::MethodCall(segment, _, [], _) = expr.kind else {
            return;
        };

        if segment.ident.name != sym::clone {
            return;
        }

        let ty::Adt(made, _) = cx.typeck_results().expr_ty(expr).kind() else {
            return;
        };

        if owned_by_nobody(cx, made.did()).is_none() {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT081_NO_SHARED_OWNERSHIP,
            expr.span,
            "this `.clone()` makes another owner of a value nobody owns, and nothing at the call says so",
            None,
            HELP_TEXT,
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
