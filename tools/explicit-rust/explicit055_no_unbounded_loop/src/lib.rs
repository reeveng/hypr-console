//! EXPLICIT055: every loop has a bound a reader can see.
//!
//! EXPLICIT053 took the `while` and EXPLICIT054 the bare `loop`, so every loop
//! left in the tree is a `for` -- but a `for` is only as bounded as what it
//! walks, and the standard library sells iterators with no end in them:
//! `repeat` never runs out, `cycle` starts again, `from_fn` stops only when
//! the closure says, as does `repeat_with`, `successors` stops only when the
//! step says, and a channel's `Receiver` hands events over until the other end
//! dies -- which is a machine's lifetime, not a bound the code holds. That is
//! true of `iter`, of `into_iter`, and of a `for` handed the receiver itself.
//! `try_iter` is not on the list: it hands over what is already waiting and
//! ends, which is a bound the channel holds.
//!
//! This rule is the kernel's argument applied to a place the desktop never
//! reaches: the code that flies cannot be counted on to come back, so what it
//! runs has to end when its fuel does. But it is asked of every crate, because
//! an endless walk is no more readable on a desktop than in the air -- the
//! difference is only who notices first.
//!
//! What it asks is not that the iterators be unwritten but that they be
//! *placed*: `repeat`, `cycle`, `from_fn`, `successors` and an endless
//! receiver belong at the one top-level site that owns the source they walk --
//! the wait, the runtime's event loop -- and a bound inside them is either
//! visible (`take`, a fuel counter, a `const` limit) or the site carries the
//! allow and its reason says why the end is named where it is. The rule reads
//! the resolved path, so a `use` does not slip past.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::higher::ForLoop;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};

dylint_linting::declare_late_lint! {
    /// EXPLICIT055: an iterator with no end in it -- `repeat`, `repeat_with`,
    /// `cycle`, `from_fn`, `successors`, a channel's `Receiver` -- is a loop whose bound
    /// is written nowhere. Walk it at the site that owns it, with the bound in
    /// sight (`take`, a fuel counter, a `const` limit), or carry an allow whose
    /// reason names where the end really is.
    pub EXPLICIT055_NO_UNBOUNDED_LOOP,
    Warn,
    "an iterator with no end in it is a loop with no bound; take one, count fuel, or name the site that owns the end"
}

// The resolved path, so a `use` or a re-export does not slip past. The
// receiver is caught by the end of its path rather than its start, because a
// `for event in events.iter()` resolves to `Receiver::iter` from however far
// away the channel was built.
fn is_endless(path: &str) -> Option<&'static str> {
    // The trait's methods resolve through the `std::` re-export rather than
    // through `core`, and the receiver's through its own module, so the last
    // of the path is what matches and the crate before it is not assumed.
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "repeat" if path.contains("iter::") => {
            Some("`repeat` hands the same thing over forever; nobody wrote where it stops")
        }
        "repeat_with" if path.contains("iter::") => {
            Some("`repeat_with` asks the closure forever; nobody wrote where it stops")
        }
        "cycle" if path.contains("iter::") => {
            Some("`cycle` starts the walk again when it ends; the end it offers is not one")
        }
        "from_fn" if path.contains("iter::") => {
            Some("`from_fn` runs until the closure says no, and the closure is not here")
        }
        "successors" if path.contains("iter::") => {
            Some("`successors` runs until the step says no, and the step is not here")
        }
        "iter" if path.contains("mpsc::Receiver") => Some(FOREVER),
        _ => None,
    }
}

const FOREVER: &str =
    "a channel hands events over until the other end dies, which is the machine's lifetime and not a bound this code holds";

// A receiver walked whole: a `for` handed it, or an `into_iter` asked of it.
// Neither resolves to anything of the receiver's own -- the one is the
// lowering's call and the other the trait's -- so the type is what is read.
fn is_a_receiver(cx: &LateContext<'_>, held: Ty<'_>) -> bool {
    match held.peel_refs().kind() {
        ty::Adt(adt, _) => cx.tcx.def_path_str(adt.did()).ends_with("mpsc::Receiver"),
        _ => false,
    }
}

// What a call resolves to, whether it was written as a path or as a method.
fn what_is_called(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<String> {
    match expr.kind {
        ExprKind::Call(called, _) => {
            let ExprKind::Path(ref qpath) = called.kind else {
                return None;
            };

            let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id)
            else {
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

impl<'tcx> LateLintPass<'tcx> for Explicit055NoUnboundedLoop {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if let Some(walked) = ForLoop::hir(expr) {
            if is_a_receiver(cx, cx.typeck_results().expr_ty(walked.arg)) {
                lint(cx, walked.arg, "a `for` over a receiver", FOREVER);
            }

            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        if let ExprKind::MethodCall(segment, base, [], _) = expr.kind
            && segment.ident.as_str() == "into_iter"
            && is_a_receiver(cx, cx.typeck_results().expr_ty(base))
        {
            lint(cx, expr, "`into_iter` over a receiver", FOREVER);

            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        let Some(harm) = is_endless(named.as_str()) else {
            return;
        };

        lint(cx, expr, format!("`{named}`").as_str(), harm);
    }
}

fn lint(cx: &LateContext<'_>, at: &Expr<'_>, named: &str, harm: &str) {
    span_lint_and_help(
        cx,
        EXPLICIT055_NO_UNBOUNDED_LOOP,
        at.span,
        format!("{named} walks with no end in it: {harm}"),
        None,
        "bound it where it is walked -- `take(N)`, a fuel counter, a `const` limit -- or walk \
         it at the one site that owns the source, carrying an allow whose reason says where \
         the end is named",
    );
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
