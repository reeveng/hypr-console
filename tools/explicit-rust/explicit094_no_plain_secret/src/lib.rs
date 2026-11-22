//! EXPLICIT094: a secret is a type, so it cannot be treated like a word.
//!
//! A password held in a `String` is a string, and everything a string can do
//! is something that can now happen to a password. A struct it sits in
//! derives `Debug` and it is in the journal the first time anybody prints the
//! struct to find out why something else failed. It is compared with `==`,
//! which stops at the first byte that differs and so says, in how long it
//! took, how much of a guess was right. It is cloned into a message, kept in
//! a field for the life of a page, and still in freed memory after the page
//! has gone, because nothing wrote over it. None of those is a decision
//! anybody made; each is what the type allows, done by code that was written
//! for an ordinary word.
//!
//! EXPLICIT024 gave a type to every quantity that could be mistaken for
//! another, and a secret is the quantity with the most to lose by it. The
//! type it wants has no `Debug`, no `Display` and no `PartialEq`, so the
//! derive on a struct that holds one does not compile and the compiler does
//! the rest of this rule's work; it is asked for its bytes by a method whose
//! name a reader will notice, is compared by one that takes the same time
//! whatever it finds, and is written over when it is dropped.
//!
//! What a lint can hold is only the entrance, and it holds it by the name,
//! which is the weakest way this suite reads anything -- EXPLICIT022 resolves
//! the method rather than trusting the word, and this cannot, because no type
//! says a string is a secret until the type exists. So it asks a field and a
//! parameter whose name says password, passphrase, passkey, pin or secret and
//! whose type is text, bytes or a bare number. A name that only describes
//! one -- whether a network wants a password, how long one is -- has a type
//! that is none of those and is not asked.
//!
//! The one use it can read is the comparison. `==` on two strings stops at
//! the first byte that differs, so how long it took says how much of a guess
//! was right, and a guess can be improved a byte at a time from outside the
//! process. An `==` or a `!=` with a secret's name on either side is asked
//! for the comparison that takes the same time whatever it finds -- which is
//! the method the type has instead of `PartialEq`.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{BinOpKind, Body, Expr, ExprKind, FieldDef, FnDecl, LangItem, PatKind, QPath};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};

dylint_linting::declare_late_lint! {
    /// EXPLICIT094: a field or a parameter named for a password, a
    /// passphrase, a passkey, a pin or a secret is not a `String`, a `str`,
    /// bytes or a bare number. It is a type with no `Debug` and no `==`.
    pub EXPLICIT094_NO_PLAIN_SECRET,
    Warn,
    "a secret held as a plain string can be printed, compared and left behind like any other word"
}

const SECRETS: [&str; 7] = ["password", "passphrase", "passkey", "passcode", "pincode", "secret", "psk"];

fn names_a_secret(named: &str) -> bool {
    let lowered = named.to_ascii_lowercase();
    let mut parts = lowered.split('_').filter(|part| !part.is_empty());

    lowered.contains("pin_code") || parts.any(|part| SECRETS.contains(&part) || part == "pin")
}

fn is_plain<'tcx>(cx: &LateContext<'tcx>, held: Ty<'tcx>) -> bool {
    let held = held.peel_refs();

    match held.kind() {
        ty::Str | ty::Int(_) | ty::Uint(_) => true,
        ty::Slice(of) | ty::Array(of, _) => matches!(of.kind(), ty::Uint(ty::UintTy::U8)),
        ty::Adt(made, given) => {
            let text = cx.tcx.is_lang_item(made.did(), LangItem::String);

            let wrapped = cx.tcx.is_diagnostic_item(sym::Vec, made.did())
                || cx.tcx.is_diagnostic_item(sym::Option, made.did())
                || made.is_box();

            text || (wrapped && given.types().next().is_some_and(|inner| match inner.kind() {
                ty::Uint(ty::UintTy::U8) => cx.tcx.is_diagnostic_item(sym::Vec, made.did()),
                _ => !cx.tcx.is_diagnostic_item(sym::Vec, made.did()) && is_plain(cx, inner),
            }))
        }
        _ => false,
    }
}

fn said(cx: &LateContext<'_>, named: &str, at: Span) {
    span_lint_and_help(
        cx,
        EXPLICIT094_NO_PLAIN_SECRET,
        at,
        format!("`{named}` is a secret by its name and an ordinary value by its type"),
        None,
        "hold it in a type made for one: no `Debug`, no `Display`, no `PartialEq`, a comparison \
         that takes the same time whatever it finds, and its bytes written over when it is dropped. \
         Where the name describes a secret and the value is not one, allow this rule here and let \
         the reason say what it is",
    );
}

// The name a comparison's operand goes by: a binding, or the field it was
// read from, through whatever borrow or view was taken of it on the way.
fn named_operand(operand: &Expr<'_>) -> Option<String> {
    match operand.kind {
        ExprKind::AddrOf(_, _, inner) | ExprKind::Unary(_, inner) => named_operand(inner),
        ExprKind::MethodCall(_, receiver, [], _) => named_operand(receiver),
        ExprKind::Field(_, named) => Some(named.name.to_string()),
        ExprKind::Path(QPath::Resolved(None, path)) => path.segments.last().map(|last| last.ident.name.to_string()),
        _ => None,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit094NoPlainSecret {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Binary(asked, one, other) = expr.kind else {
            return;
        };

        if !matches!(asked.node, BinOpKind::Eq | BinOpKind::Ne) {
            return;
        }

        let secret = [one, other].into_iter().find_map(|operand| {
            let named = named_operand(operand)?;

            (names_a_secret(named.as_str()) && is_plain(cx, cx.typeck_results().expr_ty(operand))).then_some(named)
        });

        let Some(named) = secret else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT094_NO_PLAIN_SECRET,
            expr.span,
            format!("`{named}` is compared with `{}`, which stops at the first byte that differs", asked.node.as_str()),
            None,
            "compare secrets in time that does not depend on what they hold: fold the difference of \
             every byte and look at the total once, behind a method on the type that holds one. \
             Where neither side is a secret, allow this rule here and let the reason say what they are",
        );
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        if field.span.from_expansion() || !names_a_secret(field.ident.name.as_str()) {
            return;
        }

        let held = cx.tcx.type_of(field.def_id).instantiate_identity().skip_normalization();

        if is_plain(cx, held) {
            said(cx, field.ident.name.as_str(), field.span);
        }
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _kind: FnKind<'tcx>,
        _declared: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        _id: LocalDefId,
    ) {
        if span.from_expansion() {
            return;
        }

        let typeck = cx.tcx.typeck_body(body.id());

        for taken in body.params {
            let PatKind::Binding(_, _, named, _) = taken.pat.kind else {
                continue;
            };

            if names_a_secret(named.name.as_str()) && is_plain(cx, typeck.pat_ty(taken.pat)) {
                said(cx, named.name.as_str(), taken.span);
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
