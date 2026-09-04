#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{AmbigArg, GenericArg, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT003: `Result<T, !>` is forbidden. A `Result` whose error is the
    /// never type is a `Result` that cannot fail, written as though it could.
    /// It costs every caller a `match` arm that can never be taken. Return `T`,
    /// or give the error a name that says what went wrong.
    pub EXPLICIT003_NO_NEVER_ERROR,
    Deny,
    "`Result<T, !>` is forbidden; return the value, or name a real error type"
}

// Tests are exempt, as everywhere in this suite: the harness build of a target
// is skipped, and the ordinary build of the same code is linted as production.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The last segment of the written path, so that `Result<..>`, `std::result::Result<..>`
// and a `use`d alias all answer the same way.
fn is_written_result(ty: &Ty<'_>) -> Option<&'static str> {
    let TyKind::Path(QPath::Resolved(_, path)) = ty.kind else {
        return None;
    };
    let seg = path.segments.last()?;
    if seg.ident.name.as_str() == "Result" { Some("Result") } else { None }
}

impl<'tcx> LateLintPass<'tcx> for Explicit003NoNeverError {
    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        if is_test_build(cx) {
            return;
        }
        let ty: &Ty<'tcx> = ty.as_unambig_ty();
        if is_written_result(ty).is_none() {
            return;
        }
        let TyKind::Path(QPath::Resolved(_, path)) = ty.kind else {
            return;
        };
        let Some(seg) = path.segments.last() else {
            return;
        };
        let Some(args) = seg.args else {
            return;
        };
        // The error is the second type argument. A `Result` written with one
        // argument is an alias that has already chosen its error.
        let types: Vec<_> = args
            .args
            .iter()
            .filter_map(|a| match a {
                GenericArg::Type(t) => Some(t),
                _ => None,
            })
            .collect();
        let Some(err) = types.get(1) else {
            return;
        };
        if matches!(err.kind, TyKind::Never) {
            span_lint_and_help(
                cx,
                EXPLICIT003_NO_NEVER_ERROR,
                err.span,
                "`Result<T, !>` is forbidden: an error that cannot happen is not an error",
                None,
                "return `T` on its own, or name an error type that says what went wrong",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
