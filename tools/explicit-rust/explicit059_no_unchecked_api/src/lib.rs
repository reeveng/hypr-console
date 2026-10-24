//! EXPLICIT059: the unchecked spellings are not shortcuts, they are promises.
//!
//! Every function this rule denies answers a question the safe spelling asks
//! and returns a value that is only right if the answer was yes:
//! `transmute` is a bit-pattern change with no type in between,
//! `get_unchecked` is indexing with the `None` thrown away, `unwrap_unchecked`
//! is an `unwrap` the compiler is told not to check, `from_utf8_unchecked` is
//! utf8 validation by assertion, `mem::forget` drops a destructor on the
//! floor, `Box::leak` gives up the owner, and `assume_init` declares a value
//! written that nothing wrote. Each of them compiles to the same few
//! instructions the safe form does and removes the one branch that made the
//! safe form honest -- which is a trade nobody can see in the source.
//!
//! EXPLICIT058 asks why an `unsafe` is written at all; this rule is the list
//! it is most often excused with, denied on their own names so the excuse has
//! to be about the site and not about the call. `try_into` and `TryFrom` answer
//! a bit-pattern change with a value; `get` and `from_utf8` answer with an
//! `Option` and a `Result`; the drop happens where the value's lifetime ends
//! or is handed to `console-program-lifetime` to say.
//!
//! It arrives `Deny`: nothing in the tree spells any of these, which makes it
//! a ratchet like 034 and 037 -- the first unsafe that wants one of these
//! names has to carry an allow whose reason says why the check it skips was
//! already done, in writing, by something the reader can find.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT059: `transmute`, `get_unchecked`, `unwrap_unchecked`,
    /// `from_utf8_unchecked`, `mem::forget`, `Box::leak` and `assume_init`
    /// answer a question by promising the answer was yes. Ask with the form
    /// whose answer is a value.
    pub EXPLICIT059_NO_UNCHECKED_API,
    Deny,
    "an unchecked API promises what the safe form checks; use the spelling whose answer is a value"
}

fn promises(path: &str) -> Option<&'static str> {
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "transmute" | "transmute_unchecked" => {
            Some("a bit-pattern change with no type in between; `TryFrom` says what it becomes")
        }
        "get_unchecked" | "get_unchecked_mut" => {
            Some("indexing with the `None` thrown away; `get` returns the absence instead")
        }
        "unwrap_unchecked" | "expect_unchecked" => {
            Some("an `unwrap` the compiler is told not to check; meet the absence rather than promising it cannot happen")
        }
        "from_utf8_unchecked" | "from_utf8_unchecked_mut" => {
            Some("utf8 validation by assertion; `std::str::from_utf8` returns a `Result`")
        }
        "forget" => Some("a destructor dropped on the floor; say the lifetime where the value is made instead"),
        "leak" => Some("an owner given up on purpose; the value wants an owner whose end is said"),
        "assume_init" => Some("a value declared written that nothing wrote; the `MaybeUninit` wants a write that is checked"),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit059NoUncheckedApi {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        let id = match expr.kind {
            ExprKind::Call(called, _) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                let Res::Def(DefKind::Fn | DefKind::AssocFn, id) =
                    cx.qpath_res(qpath, called.hir_id)
                else {
                    return;
                };

                id
            }
            // The slice and the `str` spellings are methods, resolved the same
            // way; `mem::forget` and `Box::leak` arrive as calls.
            ExprKind::MethodCall(..) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                id
            }
            _ => return,
        };

        // The names are the standard library's. A `forget` this tree wrote
        // for itself -- a home screen forgetting a tile -- promises nothing.
        if !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "alloc" | "std") {
            return;
        }

        let path = cx.tcx.def_path_str(id);

        let Some(harm) = promises(path.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT059_NO_UNCHECKED_API,
            expr.span,
            format!("`{path}` checks nothing: {harm}"),
            None,
            "use the spelling whose answer is a value. Where the check really was done beside the \
             call, allow this rule here and let the reason point at where",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
