//! EXPLICIT078: the flight builds from what was pinned.
//!
//! A supply chain is code the build runs without anybody reading it, and the
//! flight crates are the last place this tree can afford that: what flies is
//! built once and cannot be reasoned about on a device, so every line in it
//! wants to be a line somebody can find. The pinned build is the policy --
//! `--locked`, vendored dependencies, `cargo-deny` in CI, versions held in
//! the lockfile -- and most of that lives in the manifest and the pipeline,
//! where a lint cannot read it. What a lint *can* hold is the door the build
//! opens widest: the proc macro.
//!
//! A derive on a flight crate runs code from another crate at compile time,
//! inside the build, with full access to the source and to the code it
//! generates -- and what it writes into the binary is not in any file a
//! reviewer can open. That is the one dependency whose code runs *during* the
//! build, which makes it the one a pinned build most wants to refuse. The
//! kernel's types can spell such an impl by hand or carry the allow; what they
//! cannot do is expand someone else's program into the flight.
//!
//! The rule reads the impls the derive leaves behind: by the time any pass
//! runs, the `#[derive]` attribute is spent and what remains is the impl it
//! wrote, marked as coming from an expansion. An impl whose span says a macro
//! made it is a proc macro's output, and in a flight crate that is the budget
//! being spent -- the rule names it at the impl, which is the thing that
//! actually landed in the binary. A derive the compiler ships -- `Debug`,
//! `Clone`, `PartialEq` and the rest of `core`'s own -- is not one: it is the
//! compiler writing the impl, from the toolchain the build is already pinned
//! to, and no crate's program runs to write it. Those are left alone, and
//! what is asked is the derive another crate brings.
//!
//! What the rule is asked of is the flight crates (`console-kernel` and its
//! family) alone -- the desktop derives by design, and the argument does not
//! reach it -- and it arrives `Warn` over a kernel whose derives are all the
//! compiler's own, which is the distance a ratchet starts from: the first
//! derive a crate brings into the flight is the first line it prints.
//!
//! Its ui case is built as a crate named like the kernel's, because the rule
//! is asked only of `console-kernel*` and the case is the one way to press it
//! here on a desktop.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT078: in the flight crates, no proc macros -- a derive runs
    /// another crate's program inside the build and writes a line no
    /// reviewer can open. Spell it by hand, or carry the allow.
    pub EXPLICIT078_PINNED_SUPPLY_CHAIN,
    Warn,
    "a proc macro reached for in a crate that flies; the flight builds from what was pinned"
}

fn is_flight(cx: &LateContext<'_>) -> bool {
    cx.tcx
        .crate_name(LOCAL_CRATE)
        .as_str()
        .starts_with("console_kernel")
}

impl<'tcx> LateLintPass<'tcx> for Explicit078PinnedSupplyChain {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if !is_flight(cx) {
            return;
        }

        // A `derive` is spent before any rule can read it; the impl it wrote
        // is what is left, and its span says a macro made it.
        if !item.span.from_expansion() {
            return;
        }

        let ItemKind::Impl(under) = item.kind else {
            return;
        };

        // The compiler's own derives are the toolchain, not a supply chain.
        if cx.tcx.is_builtin_derived(item.owner_id.to_def_id()) {
            return;
        }

        let named = match under.of_trait {
            Some(header) => {
                let path = header.trait_ref.path;

                match path.res {
                    rustc_hir::def::Res::Def(rustc_hir::def::DefKind::Trait, id) => {
                        cx.tcx.def_path_str(id)
                    }
                    _ => return,
                }
            }
            None => String::from("an inherent method"),
        };

        span_lint_and_help(
            cx,
            EXPLICIT078_PINNED_SUPPLY_CHAIN,
            item.span.source_callsite(),
            format!("this impl was written by a proc macro: `impl {named}`"),
            None,
            "spell the trait by hand -- an impl the kernel writes itself is a line a reviewer \
             can open -- or, where the derive really is from a crate the flight already depends \
             on, allow this rule here and let the reason name the crate and why it is trusted. \
             The pinned build is the argument; this rule is its door",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "console_kernel_case");
}
