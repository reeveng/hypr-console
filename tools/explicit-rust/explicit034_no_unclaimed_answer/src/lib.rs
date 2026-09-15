#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_abi::ExternAbi;
use rustc_hir::{FnDecl, FnRetTy};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT034: a function that computes an answer says that the answer is
    /// the point of calling it.
    ///
    /// EXPLICIT009 already asks that a discarded `#[must_use]` value be written
    /// `let _ = ...`, so that throwing one away is a line somebody wrote rather
    /// than a line somebody did not. It can only ask that where the attribute
    /// is: a call whose answer nothing marked is a call that can be dropped
    /// in the middle of a block and read as a thing being done.
    ///
    /// This is the other half, asked at the declaration instead of the call. A
    /// function that hands back a value and mutates nothing is a question, and
    /// a question whose answer is dropped was not asked for any reason.
    ///
    /// It arrives with almost nothing to say here, and the reason is worth
    /// writing down rather than reading as a rule that does no work.
    /// EXPLICIT002 already sent every function that could not say it succeeded
    /// through `Result<T, Never>`, and `Result` carries the attribute itself --
    /// so the answers this rule would have found were claimed by the rule
    /// before it. What is left is the day somebody writes a getter that hands
    /// back a `String`, and the ratchet is the whole reason to have it written
    /// before that day rather than after.
    ///
    /// A function that takes anything by `&mut` is not asked. There the call is
    /// a doing as well as an answer -- a cursor stepped, a buffer filled -- and
    /// a caller that wanted only the doing is not throwing anything away.
    /// Neither is a method implementing somebody else's trait, for the reason
    /// EXPLICIT002, 007 and 008 all skip one: the signature was the trait's to
    /// choose, and a trait written here is linted where it is written.
    pub EXPLICIT034_NO_UNCLAIMED_ANSWER,
    Deny,
    "a function whose answer is the point of it should say so with `#[must_use]`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn implements_a_trait(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    matches!(
        cx.tcx.def_kind(cx.tcx.parent(def_id.to_def_id())),
        rustc_hir::def::DefKind::Impl { of_trait: true }
    )
}

fn is_the_entry_point(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    matches!(cx.tcx.entry_fn(()), Some((entry, _)) if entry == def_id.to_def_id())
}

fn speaks_another_language(kind: rustc_hir::intravisit::FnKind<'_>) -> bool {
    let abi = match kind {
        rustc_hir::intravisit::FnKind::ItemFn(_, _, header) => header.abi,
        rustc_hir::intravisit::FnKind::Method(_, sig) => sig.header.abi,
        rustc_hir::intravisit::FnKind::Closure => ExternAbi::Rust,
    };
    abi != ExternAbi::Rust
}

// A call that can change something it was lent is a doing as well as an answer,
// and a caller is entitled to want only the doing. Asked of the resolved
// signature rather than of what is written, so an alias for a `&mut` is read
// the same as one spelled out.
fn changes_what_it_was_lent(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    cx.tcx
        .fn_sig(def_id)
        .skip_binder()
        .inputs()
        .skip_binder()
        .iter()
        .any(|held| matches!(held.kind(), rustc_middle::ty::Ref(_, _, rustc_middle::ty::Mutability::Mut)))
}

// A macro's entry point answers to the compiler, which is the same reason an
// `extern` function is not asked: `TokenStream` in and `TokenStream` out is the
// shape the language requires, and nothing about it was chosen here.
fn is_a_macro(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    rustc_hir::find_attr!(
        cx.tcx,
        def_id.to_def_id(),
        ProcMacro | ProcMacroAttribute | ProcMacroDerive { .. }
    )
}

fn already_says_so(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    rustc_hir::find_attr!(cx.tcx, def_id.to_def_id(), MustUse { .. })
}

// The answer's own type may already carry the attribute, which is how nearly
// every function in this tree is answered for: EXPLICIT002 sends them all
// through `Result`, and `Result` says it itself. Asked of the type rather than
// through `clippy_utils::ty::is_must_use_ty`, which reads a `Result` as a
// question about what is inside it -- `Result<(), Never>` would come back as an
// answer nobody has to take, and every signature in the tree is one of those.
fn the_answer_says_so<'tcx>(cx: &LateContext<'tcx>, answer: rustc_middle::ty::Ty<'tcx>) -> bool {
    matches!(
        answer.kind(),
        rustc_middle::ty::Adt(held, _) if rustc_hir::find_attr!(cx.tcx, held.did(), MustUse { .. })
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit034NoUnclaimedAnswer {
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
        if is_a_macro(cx, def_id) {
            return;
        }
        if cx
            .tcx
            .hir_attrs(cx.tcx.local_def_id_to_hir_id(def_id))
            .iter()
            .any(|held| held.has_name(rustc_span::sym::test))
        {
            return;
        }
        if already_says_so(cx, def_id) {
            return;
        }
        if changes_what_it_was_lent(cx, def_id) {
            return;
        }

        let answer = cx.tcx.fn_sig(def_id).skip_binder().output().skip_binder();

        if answer.is_unit() || answer.is_never() {
            return;
        }
        if the_answer_says_so(cx, answer) {
            return;
        }

        let span = match decl.output {
            FnRetTy::Return(ty) => ty.span,
            FnRetTy::DefaultReturn(sp) => sp,
        };

        span_lint_and_help(
            cx,
            EXPLICIT034_NO_UNCLAIMED_ANSWER,
            span,
            "this answer can be dropped in the middle of a block and read as a thing being done",
            None,
            "a function that hands back a value and changes nothing is a question. Say `#[must_use]`, so \
             that a caller who throws the answer away writes `let _ = ...` and EXPLICIT009 can see it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
