//! EXPLICIT072: the flight's call graph is a thing you can hold.
//!
//! On the desktop, indirection is a tool: a trait object lets a panel hold
//! "whatever draws" without saying what, and a function pointer lets a table
//! be built at runtime. In the air, the same spellings are costs nobody can
//! see: a `dyn Trait` is a vtable the machine follows at a moment the stack
//! already committed to, and a function pointer is a call the call graph does
//! not contain. Neither one is in the source in a way a static reader -- or a
//! stack-size analysis, or a timing analysis -- can follow, and the flight
//! crates run where the answer to "how deep can this go" and "when does this
//! run" is the whole design.
//!
//! What the rule asks for is the shape the rest of this tree reaches for
//! anyway: an `enum` with a variant per behaviour, matched where the
//! behaviour is chosen. The match is in the source, the call graph is
//! static, and adding a behaviour is adding a variant -- which the compiler
//! then asks every match about. Generics are untouched: a `Box<dyn …>` is
//! not the only way to name "whatever draws", and `impl Trait` is the same
//! polymorphism with the table written out at compile time.
//!
//! It reads every signature and every field, a method's included, and looks
//! through what wraps the indirection: a `&dyn`, a `Box<dyn>`, an
//! `Option<fn()>` and a table of function pointers are each the same call the
//! graph does not hold. A method that implements a trait is not asked, since
//! its signature is the trait's and is asked where the trait is declared.
//!
//! The rule is asked of the flight crates (`console-kernel` and its family)
//! alone, because the desktop's crates run where a vtable is a keystroke and
//! not a commitment; it arrives `Warn` over a kernel that today has nothing
//! to catch, which is the right distance for a rule holding a shape in place
//! while the kernel grows into it.
//!
//! Its ui case is built as a crate named like the kernel's, because the rule
//! is asked only of `console-kernel*` and the case is the one way to press it
//! here on a desktop.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::trait_ref_of_method;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{
    FnDecl, FnRetTy, GenericArg, ImplItem, ImplItemKind, Item, ItemKind, QPath, TraitItem,
    TraitItemKind, Ty, TyKind,
};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT072: in the flight crates, no `dyn Trait` and no function
    /// pointers; an enum's variants keep the call graph static.
    pub EXPLICIT072_NO_DYNAMIC_DISPATCH,
    Warn,
    "a call the call graph does not hold; name the behaviours in an enum and match"
}

fn is_flight(cx: &LateContext<'_>) -> bool {
    cx.tcx
        .crate_name(LOCAL_CRATE)
        .as_str()
        .starts_with("console_kernel")
}

const TRAIT_OBJECT: &str =
    "a trait object, whose call the machine follows at a moment the stack already committed to";
const FUNCTION_POINTER: &str = "a function pointer, whose call the call graph does not contain";
const HELP: &str = "name the behaviours as an enum's variants and match where the choice is made, so \
                    the call graph stays static and adding a behaviour is adding a variant the \
                    compiler asks every match about";

// What the indirection is, said in one word for the message. It is looked
// for through whatever holds it -- a reference, a pointer, a list, the type
// arguments of a path -- because a `&dyn` and a `Box<dyn>` are the same
// vtable as a bare one.
fn what_is_indirect(ty: &Ty<'_>) -> Option<&'static str> {
    match ty.kind {
        TyKind::TraitObject(..) => Some(TRAIT_OBJECT),
        TyKind::FnPtr(..) => Some(FUNCTION_POINTER),
        TyKind::Ref(_, held) | TyKind::Ptr(held) => what_is_indirect(held.ty),
        TyKind::Slice(held) | TyKind::Array(held, _) => what_is_indirect(held),
        TyKind::Tup(held) => held.iter().find_map(what_is_indirect),
        TyKind::Path(QPath::Resolved(_, path)) => {
            let held = path.segments.last().and_then(|last| last.args.as_ref())?;

            held.args.iter().find_map(|arg| match arg {
                GenericArg::Type(inner) => what_is_indirect(inner.as_unambig_ty()),
                _ => None,
            })
        }
        _ => None,
    }
}

fn check_signature(cx: &LateContext<'_>, decl: &FnDecl<'_>) {
    let given = match decl.output {
        FnRetTy::Return(given) => Some(given),
        FnRetTy::DefaultReturn(_) => None,
    };

    for ty in decl.inputs.iter().chain(given) {
        if let Some(what) = what_is_indirect(ty) {
            span_lint_and_help(cx, EXPLICIT072_NO_DYNAMIC_DISPATCH, ty.span, what, None, HELP);
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit072NoDynamicDispatch {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if !is_flight(cx) {
            return;
        }

        if item.span.from_expansion() {
            return;
        }

        match item.kind {
            ItemKind::Fn { sig, .. } => check_signature(cx, sig.decl),

            ItemKind::Struct(_, _, data) => {
                for field in data.fields() {
                    if let Some(what) = what_is_indirect(field.ty) {
                        span_lint_and_help(
                            cx,
                            EXPLICIT072_NO_DYNAMIC_DISPATCH,
                            field.ty.span,
                            what,
                            None,
                            HELP,
                        );
                    }
                }
            }

            _ => {}
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        if !is_flight(cx) {
            return;
        }

        if item.span.from_expansion() {
            return;
        }

        let ImplItemKind::Fn(sig, _) = item.kind else {
            return;
        };

        if trait_ref_of_method(cx, item.owner_id).is_some() {
            return;
        }

        check_signature(cx, sig.decl);
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx TraitItem<'tcx>) {
        if !is_flight(cx) {
            return;
        }

        if item.span.from_expansion() {
            return;
        }

        let TraitItemKind::Fn(sig, _) = item.kind else {
            return;
        };

        check_signature(cx, sig.decl);
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "console_kernel_case");
}
