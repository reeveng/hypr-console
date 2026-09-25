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
use rustc_middle::ty::Ty;
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT038: a fault is a type, not a sentence.
    ///
    /// This is the one place the suite argues against itself, and it stood
    /// warned while that was walked back, a crate at a time, the way 002 went.
    /// EXPLICIT001 makes sure a failure met
    /// is a failure said; EXPLICIT002 puts every function through a `Result` so
    /// a caller's shape does not change when the thing it calls learns how to
    /// fail; EXPLICIT005 makes sure the answer is met. Then what arrives is a
    /// `String`, and a caller that met it can do exactly one thing with it,
    /// which is show it to someone.
    ///
    /// "the socket is not there" and "this desktop may not read it" reach the
    /// same `match` arm. Nothing downstream can retry one and give up on the
    /// other, nothing can carry a fault up through two crates without the
    /// second one guessing at the first one's wording, and a caller that wants
    /// to answer one case has to read prose for it -- which is a comparison
    /// against a sentence someone will rewrite.
    ///
    /// What the tree does instead, where it already does it: an enum that names
    /// the ways this call fails, `Display` on the enum so the words are written
    /// once and in one place, and `From` where a fault crosses a crate boundary
    /// and becomes one of the receiving crate's own cases. `Wire`, `Torn` and
    /// `Why` were the three this was registered against; the rest of the tree
    /// has since walked onto its own.
    ///
    /// A `&str` fault is the same thing with a lifetime on it, and is asked for
    /// the same way. What is not asked: a method implementing someone else's
    /// trait, an `extern` function, and the entry point, which are the three
    /// signatures no one here chose -- and the entry point for a second reason,
    /// that a fault reaching `main` has no caller left to decide anything and
    /// is on its way to being read by a person.
    pub EXPLICIT038_NO_PROSE_FAULT,
    Deny,
    "a fault said in prose is a fault no caller can decide anything about"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// An impl of someone else's trait did not choose its signature, and a trait
// written here is asked once, at the trait, rather than at every impl of it --
// so both are skipped in the pass over functions.
fn belongs_to_a_trait(cx: &LateContext<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    matches!(
        cx.tcx.def_kind(cx.tcx.parent(def_id.to_def_id())),
        rustc_hir::def::DefKind::Impl { of_trait: true } | rustc_hir::def::DefKind::Trait
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

// Read off the resolved type, so an alias for `Result<T, String>` is the same
// question as one spelled out -- which is how EXPLICIT002 reads `Done`.
fn fails_in_prose<'tcx>(cx: &LateContext<'tcx>, answer: Ty<'tcx>) -> bool {
    let rustc_middle::ty::Adt(held, args) = answer.kind() else {
        return false;
    };

    if !cx.tcx.is_diagnostic_item(rustc_span::sym::Result, held.did()) {
        return false;
    }

    let Some(fault) = args.types().nth(1) else {
        return false;
    };

    let fault = fault.peel_refs();

    if fault.is_str() {
        return true;
    }

    matches!(
        fault.kind(),
        rustc_middle::ty::Adt(said, _) if Some(said.did()) == cx.tcx.lang_items().string()
    )
}

fn say(cx: &LateContext<'_>, span: Span) {
    span_lint_and_help(
        cx,
        EXPLICIT038_NO_PROSE_FAULT,
        span,
        "this says how it failed in prose, so every way it can fail arrives as the same type",
        None,
        "name the ways this call fails in an enum, with `Display` on the enum so the words are written \
         once, and `From` where the fault crosses into another crate. A caller can then answer one case \
         and pass the rest on, which is the whole of what a fault is for",
    );
}

impl<'tcx> LateLintPass<'tcx> for Explicit038NoProseFault {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: rustc_hir::intravisit::FnKind<'tcx>,
        decl: &'tcx FnDecl<'tcx>,
        _body: &'tcx rustc_hir::Body<'tcx>,
        _span: Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        if is_test_build(cx) {
            return;
        }
        if matches!(kind, rustc_hir::intravisit::FnKind::Closure) {
            return;
        }
        if belongs_to_a_trait(cx, def_id) {
            return;
        }
        if speaks_another_language(kind) {
            return;
        }
        if is_the_entry_point(cx, def_id) {
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

        let answer = cx.tcx.fn_sig(def_id).skip_binder().output().skip_binder();

        if !fails_in_prose(cx, answer) {
            return;
        }

        let span = match decl.output {
            FnRetTy::Return(ty) => ty.span,
            FnRetTy::DefaultReturn(sp) => sp,
        };

        say(cx, span);
    }

    // A trait's own signature is where the choice was made, so it is asked here
    // rather than at every impl of it -- which is the same reasoning that lets
    // the impls go unasked.
    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx rustc_hir::TraitItem<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        let rustc_hir::TraitItemKind::Fn(sig, _) = item.kind else {
            return;
        };

        let answer = cx
            .tcx
            .fn_sig(item.owner_id.to_def_id())
            .skip_binder()
            .output()
            .skip_binder();

        if !fails_in_prose(cx, answer) {
            return;
        }

        let span = match sig.decl.output {
            FnRetTy::Return(ty) => ty.span,
            FnRetTy::DefaultReturn(sp) => sp,
        };

        say(cx, span);
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
