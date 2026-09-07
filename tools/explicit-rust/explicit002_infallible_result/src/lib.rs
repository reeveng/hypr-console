#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_abi::ExternAbi;
use rustc_hir::{FnDecl, FnRetTy, TyKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT002: an infallible function still returns `Result<T, Never>`, so
    /// that every call site reads the same and a function that learns how to
    /// fail does not change the shape of its callers.
    ///
    /// Registered `Deny`, and it was the last rule in this suite to get there.
    /// It stood alone in the warned tier for as long as it did because it was
    /// written ahead of the code and had nowhere to point -- there was no
    /// `Never` type in the workspace at all -- and it stayed there, printing
    /// its remaining distance on every run, while `console-core-never` was written
    /// and then carried a crate at a time into every signature that could not
    /// say it succeeded. The last of them was given the words, so the rule
    /// moved, and by the ratchet's one law it never moves back.
    ///
    /// What the warned tier was for is worth keeping written down, because
    /// nothing is standing in it now: a rule may be registered `Warn` while
    /// the tree is still walking towards it, so that the distance is counted
    /// on every run rather than guessed at, and `just explicit` is where that
    /// count is read.
    ///
    /// Four kinds of function are not asked, and each is a signature that was
    /// not chosen here: a method implementing somebody else's trait, an
    /// `extern` function whose shape belongs to the ABI, the program's own
    /// entry point, and anything that answers `!`.
    ///
    /// What a function answers is read after the aliases are resolved, so a
    /// `Result` under another name is a function that already says how it
    /// fails.
    pub EXPLICIT002_INFALLIBLE_RESULT,
    Deny,
    "an infallible function should still return `Result<T, Never>`"
}

// Tests are exempt, as everywhere in this suite: the harness build of a target
// is skipped, and the ordinary build of the same code is linted as production.
fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// A method that implements a trait did not choose its own signature, which is
// the reasoning EXPLICIT007 and EXPLICIT008 already skip an impl for.
// `Drop::drop` answers with nothing and `Default::default` answers with `Self`
// because the traits say so, and a type that wants to be dropped or defaulted
// has no other way to say it. Where the choice was made is the trait, and a
// trait written here is linted where it is written.
fn implements_a_trait(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    matches!(
        cx.tcx.def_kind(cx.tcx.parent(def_id.to_def_id())),
        rustc_hir::def::DefKind::Impl { of_trait: true }
    )
}

// `fn main` answers to nobody. The rule's argument is that a caller should not
// have to change shape when the thing it calls learns how to fail, and the
// entry point has no caller to spare. Asked by the entry point's own def id
// rather than by the name, so a helper somebody called `main` is still asked.
fn is_the_entry_point(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    matches!(cx.tcx.entry_fn(()), Some((entry, _)) if entry == def_id.to_def_id())
}

// An `extern` function's shape belongs to whoever calls it, which here is C:
// a signal handler and GTK's own callbacks. `Result` is not a thing that
// crosses that boundary, and the choice this rule is about was never on offer.
fn speaks_another_language(kind: rustc_hir::intravisit::FnKind<'_>) -> bool {
    let abi = match kind {
        rustc_hir::intravisit::FnKind::ItemFn(_, _, header) => header.abi,
        rustc_hir::intravisit::FnKind::Method(_, sig) => sig.header.abi,
        rustc_hir::intravisit::FnKind::Closure => ExternAbi::Rust,
    };
    abi != ExternAbi::Rust
}

// Asked of the resolved type rather than of what is written, because an alias
// is a name for a `Result` and not a second kind of answer. `console-test-stages`
// spends one -- `Done` is `Result<(), Why>` -- and every check body in the tree
// is written in it. Read syntactically, those were the one thing this rule
// cannot ask for: a function that already says how it fails, told to say it
// again in a way that cannot.
fn returns_result(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    let output = cx.tcx.fn_sig(def_id).skip_binder().output().skip_binder();

    matches!(
        output.kind(),
        rustc_middle::ty::TyKind::Adt(held, _)
            if cx.tcx.is_diagnostic_item(rustc_span::sym::Result, held.did())
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit002InfallibleResult {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        _body: &'tcx rustc_hir::Body<'tcx>,
        _span: rustc_span::Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        if is_test_build(cx) {
            return;
        }
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        if implements_a_trait(cx, def_id) {
            return;
        }
        if speaks_another_language(kind) {
            return;
        }
        if is_the_entry_point(cx, def_id) {
            return;
        }
        let hir_id = cx.tcx.local_def_id_to_hir_id(def_id);
        if cx
            .tcx
            .hir_attrs(hir_id)
            .iter()
            .any(|a| a.has_name(rustc_span::sym::test))
        {
            return;
        }
        if returns_result(cx, def_id) {
            return;
        }
        // A function that answers `!` never answers at all.
        if matches!(decl.output, FnRetTy::Return(ty) if matches!(ty.kind, TyKind::Never)) {
            return;
        }
        let span = match decl.output {
            FnRetTy::Return(ty) => ty.span,
            FnRetTy::DefaultReturn(sp) => sp,
        };
        span_lint_and_help(
            cx,
            EXPLICIT002_INFALLIBLE_RESULT,
            span,
            "this function cannot say it succeeded",
            None,
            "return `Result<T, Never>` so that every call site reads the same whether or not it can fail",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
