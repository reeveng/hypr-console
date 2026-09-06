#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{AmbigArg, GenericArg, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT003: `Result<T, !>` is forbidden. Give the error a name -- one
    /// that says what went wrong, or `Never`, which says that nothing can.
    ///
    /// This is not the opposite of EXPLICIT002, which asks for `Result<T,
    /// Never>` on every function that cannot fail. The two agree about the
    /// shape and disagree only about the spelling, and the spelling is the
    /// whole of what this rule is for: `!` is the compiler's word for a hole in
    /// the type system, and a reader meeting it in a signature cannot tell
    /// whether the author meant *this cannot fail* or had not yet decided what
    /// failing would look like. `Never` is a promise with a name on it.
    pub EXPLICIT003_NO_NEVER_ERROR,
    Deny,
    "`Result<T, !>` is forbidden; name the error, `Never` where nothing can go wrong"
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
                "`Result<T, !>` is forbidden: the compiler has no name for this error",
                None,
                "name it: `Never` where nothing can go wrong, or a type that says what did",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
