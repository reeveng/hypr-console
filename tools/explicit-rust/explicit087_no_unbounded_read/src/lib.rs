//! EXPLICIT087: a read says how much it is willing to be handed.
//!
//! `read_to_end` and `read_to_string` read until whoever is writing stops,
//! and the buffer grows for as long as that takes. Nothing in the line says
//! how much that is, because the line does not decide it: the other end does.
//! A socket somebody else holds, the output of a program, an entry in an
//! archive that says it is small and inflates to whatever its maker liked --
//! each is a heap the size of somebody else's choosing, on a handheld with
//! one user and no swap to speak of. EXPLICIT055 and EXPLICIT057 ask a loop
//! and a wait for their end; this is the same question asked of a buffer.
//!
//! EXPLICIT064 asks it of a serde type, at the door. This rule asks it one
//! step earlier, where the bytes come in, and the spelling is the standard
//! library's own: `reader.take(limit)` is a reader that ends, so a
//! `read_to_end` over one is a read whose size is written at the site, and
//! what the limit is becomes a `const` with a name somebody had to choose.
//!
//! The whole-file forms -- `std::fs::read` and `std::fs::read_to_string` --
//! are not asked about, for the reason EXPLICIT029 gave up asking about files
//! in a loop: almost everything this tree reads that way is a line of sysfs
//! or a file it wrote itself, and a rule that is four-fifths allow is
//! paperwork. A file that arrived from somewhere is opened and read through a
//! `take` like anything else, and the day that distinction can be read off a
//! type this rule should grow to hold it.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT087: `read_to_end` and `read_to_string` on a reader are asked
    /// through a `take`, so the most they can be handed is written where
    /// they are called.
    pub EXPLICIT087_NO_UNBOUNDED_READ,
    Warn,
    "a read to the end of a stream is a buffer the size of whatever the other end sends"
}

fn reads_to_the_end(path: &str) -> bool {
    matches!(
        path,
        "std::io::Read::read_to_end" | "std::io::Read::read_to_string" | "std::io::read_to_string"
    )
}

fn ends(cx: &LateContext<'_>, reader: &Expr<'_>) -> bool {
    match cx.typeck_results().expr_ty_adjusted(reader).peel_refs().kind() {
        ty::Adt(made, _) => cx.tcx.def_path_str(made.did()).ends_with("io::Take"),
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit087NoUnboundedRead {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let (id, reader) = match expr.kind {
            ExprKind::MethodCall(_, receiver, _, _) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                (id, receiver)
            }
            ExprKind::Call(called, [reader, ..]) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id)
                else {
                    return;
                };

                (id, reader)
            }
            _ => return,
        };

        let named = cx.tcx.def_path_str(id);

        if !reads_to_the_end(named.as_str()) || ends(cx, reader) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT087_NO_UNBOUNDED_READ,
            expr.span,
            format!("`{named}` reads for as long as the other end goes on writing"),
            None,
            "read through `take(LIMIT)`, with the limit a `const` whose name says what it is the \
             most of, and decide what a stream that reached it means. Where the other end is this \
             desktop's own and what it sends is already bounded, allow this rule here and let the \
             reason say by what",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
