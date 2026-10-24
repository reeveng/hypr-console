//! EXPLICIT074: what crosses a boundary has a shape the boundary can see.
//!
//! A type that is sent -- written to a device, handed to a protocol, saved
//! for a later run of the program to read back -- has two shapes: the shape
//! the compiler gave it, and the shape the other end expects. They are the
//! same shape only by accident, because the compiler's is *nobody's*: it is
//! an implementation detail Rust reserves the right to move, and it moves
//! between releases. The `#[repr(C)]` is the promise that pins it; the
//! `const` assertions on size and alignment beside it are the proof the pin
//! is where the wire said; and a version and a checksum on data that
//! outlives a run are the two answers to the question every reader has to
//! ask: is this the shape I was compiled against, and is it whole?
//!
//! What the lint can hold is the promise, and the mark it asks by is where
//! the bytes go. A type named in a signature another language is on the other
//! side of -- a function in an `extern` block, a function written `extern
//! "C"`, an `extern "C"` function pointer -- is a type whose memory is the
//! message, read by something that was not compiled with this program and
//! cannot ask it where a field went. That is the one place the compiler's
//! layout reaches outside the process, so it is the place the rule asks.
//!
//! A `Serialize` is not that mark, and the rule's first draft took it for
//! one. serde writes a value a field at a time, through the visitor its
//! derive wrote, and what comes out is the same whatever order the compiler
//! laid the fields out in; a `repr(C)` on it pins a shape nothing reads, and
//! the version and the checksum above are the questions such a type does
//! have to answer. It also denies
//! `#[repr(packed)]`, which is the wrong answer to the same worry: packing a
//! struct to fit the wire is layout by trickery, and a misaligned read on
//! the machine that receives it is a fault no assertion catches. The
//! assertions and the checksum are review's half, and `console-core-checksums`
//! is where the checksum half is already written.
//!
//! It arrives `Warn`. The types the tree hands across today -- to the kernel
//! through evdev and uinput, to PAM, to the display and to the firmware --
//! say `repr(C)` already, which is the distance a ratchet starts from: the
//! next one written without it is the first line it prints.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_abi::ExternAbi;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor, VisitorExt};
use rustc_hir::{
    AmbigArg, FnDecl, ForeignItem, ForeignItemKind, HirId, ImplItem, ImplItemKind, Item, ItemKind,
    Path, Ty, TyKind,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use std::collections::HashSet;

dylint_linting::impl_late_lint! {
    /// EXPLICIT074: a type handed across an `extern` signature has a shape
    /// outside the process; pin it with `#[repr(C)]` and assert size and
    /// alignment beside it. No `repr(packed)` -- layout by trickery is a
    /// misaligned read on the machine that receives it.
    pub EXPLICIT074_WIRE_LAYOUT_ASSERTED,
    Warn,
    "a type handed across an `extern` signature with no layout said; `repr(C)` and const assertions on size and alignment",
    Explicit074WireLayoutAsserted::new()
}

struct Explicit074WireLayoutAsserted {
    structs: Vec<(LocalDefId, Span, Layout)>,
    crossing: HashSet<LocalDefId>,
}

impl Explicit074WireLayoutAsserted {
    fn new() -> Self {
        Self {
            structs: Vec::new(),
            crossing: HashSet::new(),
        }
    }
}

// What the `repr` says, when it says anything: pinned, packed, or unsaid.
// `repr` is a built-in attribute and survives expansion, so this is the one
// attribute on a type a late rule can still read.
#[derive(PartialEq)]
enum Layout {
    Pinned,
    Packed,
    Unsaid,
}

// What the `repr` says, read off the layout the compiler resolved rather
// than off any attribute: `repr` has been turned into this by the time a
// late rule runs, which is the answer rather than the spelling. A
// `transparent` struct is pinned too: its shape is its one field's.
fn said_layout(cx: &LateContext<'_>, item: &Item<'_>) -> Layout {
    let adt = cx.tcx.adt_def(item.owner_id.def_id.to_def_id());

    let said = adt.repr();

    match (said.packed(), said.c() || said.transparent()) {
        (true, _) => Layout::Packed,
        (false, true) => Layout::Pinned,
        (false, false) => Layout::Unsaid,
    }
}

// Every struct of this crate's that a signature names, through a pointer, a
// reference, an array or a type argument -- which is how a C signature
// names one.
struct Named<'a> {
    crossing: &'a mut HashSet<LocalDefId>,
}

impl<'v> Visitor<'v> for Named<'_> {
    fn visit_path(&mut self, path: &Path<'v>, _id: HirId) {
        if let Res::Def(DefKind::Struct | DefKind::Union, did) = path.res
            && let Some(local) = did.as_local()
        {
            self.crossing.insert(local);
        }

        intravisit::walk_path(self, path);
    }
}

impl Explicit074WireLayoutAsserted {
    fn crosses(&mut self, decl: &FnDecl<'_>) {
        let mut named = Named {
            crossing: &mut self.crossing,
        };

        intravisit::walk_fn_decl(&mut named, decl);
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit074WireLayoutAsserted {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        match item.kind {
            ItemKind::Struct(..) => {
                if item.span.from_expansion() {
                    return;
                }

                self.structs
                    .push((item.owner_id.def_id, item.span, said_layout(cx, item)));
            }

            // A function this program writes for another language to call.
            ItemKind::Fn { sig, .. } if sig.header.abi != ExternAbi::Rust => {
                self.crosses(sig.decl);
            }

            _ => {}
        }
    }

    fn check_impl_item(&mut self, _cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        if let ImplItemKind::Fn(sig, _) = item.kind
            && sig.header.abi != ExternAbi::Rust
        {
            self.crosses(sig.decl);
        }
    }

    // A function another language wrote, and a value it holds.
    fn check_foreign_item(&mut self, _cx: &LateContext<'tcx>, item: &'tcx ForeignItem<'tcx>) {
        match item.kind {
            ForeignItemKind::Fn(sig, _, _) => self.crosses(sig.decl),
            ForeignItemKind::Static(held, _, _) => {
                let mut named = Named {
                    crossing: &mut self.crossing,
                };

                named.visit_ty_unambig(held);
            }
            ForeignItemKind::Type => {}
        }
    }

    // A function pointer another language calls through, wherever it is
    // written: a field, an alias, a parameter.
    fn check_ty(&mut self, _cx: &LateContext<'tcx>, written: &'tcx Ty<'tcx, AmbigArg>) {
        if let TyKind::FnPtr(pointer) = written.as_unambig_ty().kind
            && pointer.abi != ExternAbi::Rust
        {
            self.crosses(pointer.decl);
        }
    }

    // An impl can come after the struct it is for, so the decision is made
    // once, at the end of the crate.
    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (def_id, span, how) in &self.structs {
            if how == &Layout::Packed {
                span_lint_and_help(
                    cx,
                    EXPLICIT074_WIRE_LAYOUT_ASSERTED,
                    *span,
                    "this type is `repr(packed)`, which is layout by trickery",
                    None,
                    "pin the shape with `#[repr(C)]` and say the padding in the fields, rather \
                     than removing it; a packed field read is a misaligned read on the machine \
                     that receives it",
                );

                continue;
            }

            if !self.crossing.contains(def_id) {
                continue;
            }

            if how == &Layout::Pinned {
                continue;
            }

            span_lint_and_help(
                cx,
                EXPLICIT074_WIRE_LAYOUT_ASSERTED,
                *span,
                "this type has a shape outside the process, and no layout said for the shape \
                 this process gives it",
                None,
                "pin it with `#[repr(C)]` and assert its size and alignment in a `const` beside \
                 it, so the shape the other end expects is a shape the compiler is held to. Where \
                 the other end only ever holds it behind a pointer and never reads inside it, \
                 allow this rule here and let the reason say so",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
