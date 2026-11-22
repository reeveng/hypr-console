//! EXPLICIT104: a secret is not written in the source.
//!
//! A password typed into a `const` is in the repository, in every clone of
//! it, in the public copy `console-manifest-publish` builds, in the binary
//! on the device where `strings` will print it, and in the history for as
//! long as there is one -- removing the line removes it from none of those.
//! It is also the same on every machine that runs the build, which is the
//! other half: a default credential is a credential everybody has. Of all
//! the families of fault the published lists keep, this is the one most
//! often rated critical, because there is nothing to exploit. It is read.
//!
//! It is the argument EXPLICIT039 to 044 make, at the place it costs the
//! most. What a function needs in order to run is an input, and an input is
//! handed in from the edge: read from the keyring or a file only its owner
//! can open, at the moment it is wanted, into EXPLICIT094's type, and never
//! given a name in the code. `@user@` is why this tree names nobody; this is
//! why it knows nobody's password either.
//!
//! The rule reads names, as 094 does and for the reason 094 gives. A `const`,
//! a `static`, a `let` or a field of a struct being built, named for a
//! password, a passphrase, a passkey, a pin or a secret and given a string
//! literal that is not empty. A test that needs a password to type carries
//! the allow, and its reason is that nothing anywhere accepts the word.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::DefKind;
use rustc_hir::{Expr, ExprKind, Item, LetStmt, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

dylint_linting::declare_late_lint! {
    /// EXPLICIT104: nothing named for a password, a passphrase, a passkey, a
    /// pin or a secret is given a string literal.
    pub EXPLICIT104_NO_SECRET_IN_SOURCE,
    Warn,
    "a secret written in the source is in every clone, every binary and all of the history"
}

const SECRETS: [&str; 7] = ["password", "passphrase", "passkey", "passcode", "pincode", "secret", "psk"];

fn names_a_secret(named: &str) -> bool {
    let lowered = named.to_ascii_lowercase();
    let mut parts = lowered.split('_').filter(|part| !part.is_empty());

    lowered.contains("pin_code") || parts.any(|part| SECRETS.contains(&part) || part == "pin")
}

// A literal reaches a binding through whatever its type asked for.
fn peeled<'tcx>(expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    match expr.kind {
        ExprKind::AddrOf(_, _, inner) => peeled(inner),
        ExprKind::MethodCall(path, receiver, [], _)
            if matches!(path.ident.name.as_str(), "to_string" | "to_owned" | "into" | "as_bytes" | "to_vec") =>
        {
            peeled(receiver)
        }
        ExprKind::Call(_, [inner]) => peeled(inner),
        _ => expr,
    }
}

fn is_written_out(value: &Expr<'_>) -> bool {
    let ExprKind::Lit(said) = peeled(value).kind else {
        return false;
    };

    match said.node {
        rustc_ast::LitKind::Str(text, _) => !text.as_str().is_empty(),
        rustc_ast::LitKind::ByteStr(ref bytes, _) => !bytes.as_byte_str().is_empty(),
        _ => false,
    }
}

fn said(cx: &LateContext<'_>, named: &str, at: Span) {
    span_lint_and_help(
        cx,
        EXPLICIT104_NO_SECRET_IN_SOURCE,
        at,
        format!("`{named}` is a secret by its name, and its value is typed out here"),
        None,
        "read it at the edge, from the keyring or a file only its owner can open, and hand it \
         inward in a type made for one. Where the word is one nothing anywhere accepts -- a test \
         typing into a field -- allow this rule here and let the reason say so",
    );
}

impl<'tcx> LateLintPass<'tcx> for Explicit104NoSecretInSource {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if item.span.from_expansion() {
            return;
        }

        let id = item.owner_id.def_id;

        if !matches!(cx.tcx.def_kind(id), DefKind::Const { .. } | DefKind::Static { .. }) {
            return;
        }

        let named = cx.tcx.item_name(id.to_def_id());

        let Some(body) = cx.tcx.hir_maybe_body_owned_by(id) else {
            return;
        };

        if names_a_secret(named.as_str()) && is_written_out(body.value) {
            said(cx, named.as_str(), item.span);
        }
    }

    fn check_local(&mut self, cx: &LateContext<'tcx>, made: &'tcx LetStmt<'tcx>) {
        if made.span.from_expansion() {
            return;
        }

        let PatKind::Binding(_, _, named, _) = made.pat.kind else {
            return;
        };

        let Some(value) = made.init else {
            return;
        };

        if names_a_secret(named.name.as_str()) && is_written_out(value) {
            said(cx, named.name.as_str(), made.span);
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Struct(_, fields, _) = expr.kind else {
            return;
        };

        for field in fields {
            if names_a_secret(field.ident.name.as_str()) && is_written_out(field.expr) {
                said(cx, field.ident.name.as_str(), field.span);
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
