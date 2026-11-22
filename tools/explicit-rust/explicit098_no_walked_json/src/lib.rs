//! EXPLICIT098: what somebody else wrote is decoded into a type, once.
//!
//! `serde_json::from_str::<Value>` succeeds for anything that is JSON, which
//! is to say it checks nothing about what this program was expecting. The
//! checking is then done by whoever reads the value, a field at a time, with
//! `.get("windows").and_then(Value::as_i64)` -- a chain in which a field that
//! is missing, a field that is a string where a number was meant and a reply
//! that is not this kind of reply at all are one `None`. Each reader makes
//! its own guess about the shape, the guesses are written in as many places
//! as there are readers, and the day the other program changes its answer
//! nothing fails: a count reads as absent and the desktop draws nothing
//! where a window is. EXPLICIT006 calls that an error turned into an
//! absence, and CLAUDE.md calls it two readings of one file.
//!
//! EXPLICIT064 already says what the door is -- a type with every field
//! known and unknown ones refused -- and asks it of the types that derive
//! `Deserialize`. This rule closes the way round it. A decode whose answer is
//! a `Value` is a decode that put the door off until later, and later is
//! everywhere. Decoded into a struct, the shape is written once, in the crate
//! that owns the format; a reply that does not fit is a fault with a field
//! name in it, at the moment it arrived; and what the rest of the program
//! holds is a value the compiler knows the fields of.
//!
//! It reads the decode and not the walk, so it counts readings rather than
//! lines. Where the shape really is not known -- a document passed through,
//! a tree shown to a person as it came -- the site says so.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::sym;

dylint_linting::declare_late_lint! {
    /// EXPLICIT098: `serde_json::from_str`, `from_slice` and `from_reader`
    /// decode into a type of this tree's, not into a `Value` to be walked.
    pub EXPLICIT098_NO_WALKED_JSON,
    Warn,
    "JSON decoded into a `Value` is a shape every reader guesses at separately"
}

fn decodes(named: &str) -> bool {
    matches!(named, "from_str" | "from_slice" | "from_reader")
}

fn is_a_value<'tcx>(cx: &LateContext<'tcx>, made: Ty<'tcx>) -> bool {
    match made.kind() {
        ty::Adt(of, _) => {
            cx.tcx.crate_name(of.did().krate).as_str() == "serde_json" && cx.tcx.item_name(of.did()).as_str() == "Value"
        }
        _ => false,
    }
}

// A decode answers with a `Result`, and what it decoded into is the first of
// the two types that names.
fn decoded_into<'tcx>(cx: &LateContext<'tcx>, expr: &Expr<'tcx>) -> Option<Ty<'tcx>> {
    match cx.typeck_results().expr_ty(expr).kind() {
        ty::Adt(of, given) if cx.tcx.is_diagnostic_item(sym::Result, of.did()) => given.types().next(),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit098NoWalkedJson {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };

        let ExprKind::Path(ref qpath) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = cx.qpath_res(qpath, called.hir_id) else {
            return;
        };

        if cx.tcx.crate_name(id.krate).as_str() != "serde_json" || !decodes(cx.tcx.item_name(id).as_str()) {
            return;
        }

        if !decoded_into(cx, expr).is_some_and(|made| is_a_value(cx, made)) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT098_NO_WALKED_JSON,
            expr.span,
            "this decodes into a `Value`, which accepts any JSON and leaves the shape to whoever reads it",
            None,
            "decode into a struct in the crate that owns the format, every field named and unknown \
             ones refused, so a reply that does not fit is a fault where it arrived. Where the shape \
             really is not known here, allow this rule and let the reason say why",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
