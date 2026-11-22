//! EXPLICIT089: text that was not text is not quietly made into some.
//!
//! `to_string_lossy` and `from_utf8_lossy` cannot fail, and that is what is
//! wrong with them. A path on this machine is bytes and a `String` is
//! Unicode; where the bytes are not, each one that does not fit becomes
//! U+FFFD and the call carries on. Two different files now have one name.
//! The name that was checked is not the name that is opened, a comparison
//! made after the call answers for something that is not on the disk, and a
//! list keyed by the result has lost an entry without anybody dropping it.
//! It is EXPLICIT001's fault with the error taken out before it could be
//! swallowed -- and EXPLICIT033's, because what stands in for the part that
//! was missing is a character nobody chose.
//!
//! The honest spellings have an answer in them. `to_str` hands back an
//! `Option` and `from_utf8` a `Result`, and the `None` is a decision the site
//! has to make: refuse the name, skip the entry and say so, or keep the thing
//! as the `OsStr` or the bytes it was and never ask it to be text at all,
//! which is right more often than it looks -- a path handed on to another
//! call was never read by a person. Where it is being shown to one, and only
//! shown, the replacement character is what a person should see, and that is
//! the sentence the allow carries.
//!
//! It arrives `Warn`, over a tree that says it in a great many places, most
//! of them a path on its way into a message.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT089: no `to_string_lossy`, `from_utf8_lossy` or
    /// `from_utf16_lossy`. `to_str` and `from_utf8` answer with a value, and
    /// a path that is only handed on stays the path it was.
    pub EXPLICIT089_NO_LOSSY_TEXT,
    Warn,
    "a lossy conversion gives two different names one spelling and says nothing"
}

fn loses(named: &str) -> bool {
    matches!(
        named,
        "to_string_lossy" | "from_utf8_lossy" | "from_utf8_lossy_owned" | "from_utf16_lossy" | "into_string_lossy"
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit089NoLossyText {
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

        if !matches!(cx.tcx.crate_name(id.krate).as_str(), "core" | "alloc" | "std") {
            return;
        }

        let named = cx.tcx.item_name(id);

        if !loses(named.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT089_NO_LOSSY_TEXT,
            expr.span,
            format!("`{named}` answers what was not text with a character nobody chose"),
            None,
            "ask with `to_str` or `from_utf8` and meet the answer, or keep the path a path and the \
             bytes bytes where nothing reads them as text. Where the result is only ever shown to a \
             person, allow this rule here and let the reason say that nothing decides from it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
