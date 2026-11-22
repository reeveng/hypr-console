//! EXPLICIT096: EXPLICIT059's list, carried on to the calls that make memory
//! out of a promise.
//!
//! 059 denies the spellings that skip one check -- an index, a validation, a
//! destructor. These are the ones that skip the question of whether there is
//! anything there at all. `slice::from_raw_parts` declares that a pointer and
//! a length are a slice, and every bounds check after it is measured against
//! a length nobody checked. `Vec::set_len` declares elements written that
//! nothing wrote. `mem::zeroed` declares that all-zeroes is a value of the
//! type, which is false for a reference, a `NonZero` and most enums.
//! `new_unchecked` builds a type with an invariant by skipping the invariant,
//! `unreachable_unchecked` tells the compiler a branch cannot be taken and
//! lets it delete whatever was on the far side, and the `unchecked_`
//! arithmetic is EXPLICIT015's bare operator with the trap taken out. Each is
//! where a read past the end of something begins, and each compiles to
//! nothing, so the fault is found somewhere else.
//!
//! They are a rule of their own for one reason, which is the ratchet. 059
//! arrived denied because nothing in the tree spelled its names, and a
//! denied rule never grows a name the tree still writes. This tree writes
//! these: a framebuffer and a shared page are a pointer and a length the
//! kernel handed over, and `from_raw_parts` is how that becomes a slice. So
//! the list stands warned, the sites that are a mapping say whose promise the
//! length is, the rest find the checked spelling -- `MaybeUninit` and a
//! write, `NonZero::new`, `checked_add` -- and when the count is what the
//! allows explain, the names move into 059 and this crate goes.
//!
//! Reads through raw pointers can duplicate ownership even when the memory is
//! initialized, and manually dropped values must never be destroyed again.
//! The rule warns at calls that require those ownership guarantees.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT096: `from_raw_parts`, `Vec::set_len`, `mem::zeroed`,
    /// `from_raw`, `new_unchecked`, `unreachable_unchecked` and the
    /// `unchecked_` arithmetic make memory or a value out of a promise.
    pub EXPLICIT096_NO_UNCHECKED_MEMORY,
    Warn,
    "memory declared rather than checked: a length, a value or a branch nobody verified"
}

fn promises(path: &str) -> Option<&'static str> {
    let last = path.rsplit("::").next().unwrap_or_default();

    match last {
        "transmute_copy" => Some("a value copied bitwise across types; the source remains live"),
        "assume_init_read" | "assume_init_ref" | "assume_init_mut" | "assume_init_drop"
            if path.contains("MaybeUninit") =>
        {
            Some("an unchecked read, borrow or drop of potentially uninitialized memory")
        }
        "drop" | "take" if path.contains("ManuallyDrop") => {
            Some("manual destruction or extraction requires exactly one owner of the live value")
        }
        "drop_in_place" => Some("manual destruction requires a live value and no subsequent drop"),
        "from_raw_parts" | "from_raw_parts_mut" | "from_raw_parts_in" => Some(
            "a pointer and a length declared to be a slice; every check after it trusts a length nobody checked",
        ),
        "set_len" if path.contains("vec::") || path.contains("string::") => Some(
            "elements declared written that nothing wrote; `resize`, `extend` and `truncate` change a length by doing the work",
        ),
        "zeroed" | "uninitialized" if path.contains("mem::") => Some(
            "all-zeroes declared to be a value of the type, which it is not for a reference, a `NonZero` or most enums",
        ),
        "from_raw" | "from_raw_in" if !path.contains("Pid") => {
            Some("an owner declared out of a pointer, whoever else already owns what it points at")
        }
        "new_unchecked" => Some(
            "a type with an invariant built by skipping the invariant; the checked constructor answers with a value",
        ),
        "unreachable_unchecked" => Some(
            "a branch declared impossible, which lets the compiler delete whatever was on the far side of it",
        ),
        "unchecked_add" | "unchecked_sub" | "unchecked_mul" | "unchecked_neg" | "unchecked_shl"
        | "unchecked_shr" | "unchecked_div" | "unchecked_rem" => Some(
            "arithmetic with the overflow declared away; `checked_*`, `saturating_*` and `wrapping_*` each name a policy",
        ),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit096NoUncheckedMemory {
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
            ExprKind::MethodCall(..) => {
                let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
                    return;
                };

                id
            }
            _ => return,
        };

        if !matches!(
            cx.tcx.crate_name(id.krate).as_str(),
            "core" | "alloc" | "std"
        ) {
            return;
        }

        let path = cx.tcx.def_path_str(id);

        let harm = match promises(&path) {
            Some(harm) => harm,
            None if matches!(
                cx.tcx.item_name(id).as_str(),
                "read" | "read_unaligned" | "read_volatile"
            ) && (path.starts_with("core::ptr::")
                || path.starts_with("std::ptr::")
                || cx
                    .tcx
                    .inherent_impl_of_assoc(id)
                    .is_some_and(|implementation| {
                        cx.tcx
                            .type_of(implementation)
                            .instantiate_identity()
                            .skip_norm_wip()
                            .is_raw_ptr()
                    }))
                && !cx.type_is_copy_modulo_regions(cx.typeck_results().expr_ty(expr)) =>
            {
                "a raw read of a non-Copy value leaves ownership of the source to be proved"
            }
            None => return,
        };

        span_lint_and_help(
            cx,
            EXPLICIT096_NO_UNCHECKED_MEMORY,
            expr.span,
            format!("`{path}` checks nothing: {harm}"),
            None,
            "use the spelling that does the work or answers with a value. Where the promise is \
             somebody else's -- a length the kernel handed over with a mapping -- allow this rule \
             here and let the reason say whose it is and where it was made",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
