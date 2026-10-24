//! EXPLICIT066: the kernel does not allocate after it is up.
//!
//! The desktop's crates run where a heap is always there and somebody is
//! always watching it; the kernel does not. What flies has an init and then
//! has a job, and the heap is the difference between the two: a `Box` after
//! init is a claim on memory nobody budgeted, a `Vec` is a length the machine
//! decides, a `format!` is a scratch space for a sentence that could have
//! been written into a fixed buffer, and a `HashMap` is a claim on all three
//! at once. The flight crates -- `console-kernel` and its family -- are the
//! ones that boot and then answer, so this rule is asked of them alone: the
//! desktop's crates allocate by design, and the argument does not reach them.
//!
//! What the rule asks for is the shape the kernel's `allocator` module is:
//! the heap exists, one place holds it, and everything after init runs on
//! fixed-capacity types and borrows of what init made. A `no_std` that stops
//! using `alloc` entirely is the same argument at one step further, and the
//! kernel is walking that way; the rule is here so that walking it is a
//! decision the source shows rather than a coincidence nobody notices going
//! backwards.
//!
//! It arrives `Warn` over a kernel that today keeps a `Vec` for its log and a
//! `String` for each line -- which is exactly the distance this rule is meant
//! to print: every one of those is a buffer whose size somebody chose, and
//! the flight wants to have chosen them before it boots.
//!
//! Its ui case is built as a crate named like the kernel's, because the rule
//! is asked only of `console-kernel*` and the case is the one way to press it
//! here on a desktop.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{Expr, ExprKind, LangItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::sym;

dylint_linting::declare_late_lint! {
    /// EXPLICIT066: in the flight crates, no `Box`, `Vec`, `String`, `format!`,
    /// `to_string` or `HashMap` after init. Allocate in init, run on fixed-capacity
    /// types, and let a buffer's size be a decision someone made before the
    /// machine booted.
    pub EXPLICIT066_NO_HEAP_AFTER_INIT,
    Warn,
    "the heap reached for after init in a crate that flies; fixed capacity is the flight's budget"
}

// The flight crates: the kernel and its family. Everything else in this tree
// runs on a desktop, where the heap is a tool and not a budget.
fn is_flight(cx: &LateContext<'_>) -> bool {
    cx.tcx
        .crate_name(LOCAL_CRATE)
        .as_str()
        .starts_with("console_kernel")
}

// What reaching for the heap resolves to. `vec!` and `format!` arrive as
// calls inside macro expansions, so they are caught by the paths the
// expansions resolve to and reported at the call site a person wrote.
// The resolved path, so a re-export does not slip past. The methods arrive
// through `std::` with generic markers in them, so the last two segments are
// what matches.
fn reaches_for_the_heap(path: &str) -> Option<&'static str> {
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "new" if path.contains("boxed::Box") => {
            Some("`Box::new` claims a piece of the heap nobody budgeted")
        }
        "new" if path.contains("hash_map::HashMap") => {
            Some("`HashMap::new` claims a heap, a hasher and a growth policy at once")
        }
        "new" if path.contains("vec::Vec") => {
            Some("`Vec::new` opens a length the machine will decide")
        }
        "new" if path.contains("string::String") => {
            Some("`String::new` opens a length the machine will decide")
        }
        "with_capacity" if path.contains("vec::Vec") => {
            Some("`Vec::with_capacity` reserves the heap in advance of needing it")
        }
        "with_capacity" if path.contains("string::String") => {
            Some("`String::with_capacity` reserves the heap in advance of needing it")
        }
        "format" if path.contains("fmt::") => {
            Some("`format!` spends a scratch heap on a sentence")
        }
        "to_string" if path.contains("ToString") => {
            Some("`to_string` spends the heap on a copy of what was already held")
        }
        // `vec!` is sugar whose expansion ends in an `into_vec` nobody wrote
        // by hand, however the nightly lowers it -- or, for `vec![x; n]`, in a
        // `from_elem` that fills a heap of the size asked.
        "from_elem" if path.contains("vec::") => {
            Some("`vec!` spends the heap on a list whose size could have been written")
        }
        _ if path.contains("into_vec") => {
            Some("`vec!` spends the heap on a list whose size could have been written")
        }
        _ => None,
    }
}

// `String::from` and `Vec::from` resolve to the `From` trait rather than to
// anything of `String`'s, so whether they reach for the heap is asked of the
// type the call makes.
fn made_on_the_heap(cx: &LateContext<'_>, expr: &Expr<'_>, path: &str) -> Option<&'static str> {
    let last = path.rsplit("::").next().unwrap_or_default();

    let ty::Adt(made, _) = cx.typeck_results().expr_ty(expr).kind() else {
        return None;
    };

    let heap = cx.tcx.is_lang_item(made.did(), LangItem::String)
        || cx.tcx.is_diagnostic_item(sym::Vec, made.did())
        || cx.tcx.is_diagnostic_item(sym::HashMap, made.did())
        || made.is_box();

    match last {
        "from" if heap && path.contains("convert::From") => {
            Some("this spends the heap on a copy of what was already held")
        }
        _ => None,
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

impl<'tcx> LateLintPass<'tcx> for Explicit066NoHeapAfterInit {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if !is_flight(cx) {
            return;
        }

        let Some(named) = what_is_called(cx, expr) else {
            return;
        };

        let Some(harm) = reaches_for_the_heap(named.as_str())
            .or_else(|| made_on_the_heap(cx, expr, named.as_str()))
        else {
            return;
        };

        // A macro wrote the call; the person wrote the macro. The span to
        // read is the one a hand touched.
        let shown = match expr.span.from_expansion() {
            true => expr.span.source_callsite(),
            false => expr.span,
        };

        span_lint_and_help(
            cx,
            EXPLICIT066_NO_HEAP_AFTER_INIT,
            shown,
            harm,
            None,
            "allocate in init and hand out fixed-capacity borrows, so a buffer's size is a decision \
             made before the machine booted. Where a flight crate genuinely must grow one, allow \
             this rule here and let the reason say what budgets it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "console_kernel_case");
}
