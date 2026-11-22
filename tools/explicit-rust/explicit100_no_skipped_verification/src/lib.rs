//! EXPLICIT100: who is on the other end is checked, and the check is never
//! switched off.
//!
//! A certificate is the only thing that makes an encrypted connection a
//! connection to somebody in particular. With verification off, the traffic
//! is still encrypted -- to whoever answered, which on a handheld that joins
//! whatever network is in the room is whoever is in the room. Every library
//! that can do this has a spelling for it, and every one of them was written
//! in for the same reason: a test against a server with a certificate nobody
//! signed, made to pass on the afternoon it was written. Then it ships, and
//! it is right there in the call that fetches an add-on or a model, looking
//! like configuration.
//!
//! The libraries have done half of this already, which is the half a lint
//! likes: the spelling is loud. `danger_accept_invalid_certs` says what it
//! is, and rustls keeps the whole door behind a method called `dangerous`.
//! So the rule is the list of those names, denied wherever they are called.
//! A test that needs a server of its own gives the client that server's
//! certificate to trust, which is one line more and is the thing the test
//! was meant to be about.
//!
//! It arrives `Deny`: nothing in the tree spells any of them, which makes it
//! a ratchet like 034, 037 and 059 -- what it costs is this page, and what it
//! buys is that the afternoon somebody reaches for one, the gate is there.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT100: no `danger_accept_invalid_certs`,
    /// `danger_accept_invalid_hostnames`, `dangerous` or custom certificate
    /// verifier. A test trusts its own server's certificate instead.
    pub EXPLICIT100_NO_SKIPPED_VERIFICATION,
    Deny,
    "certificate verification switched off is a connection to whoever answered"
}

fn skips_the_check(named: &str) -> bool {
    matches!(
        named,
        "danger_accept_invalid_certs"
            | "danger_accept_invalid_hostnames"
            | "dangerous"
            | "with_custom_certificate_verifier"
            | "set_certificate_verifier"
            | "tls_danger_accept_invalid_certs"
            | "tls_danger_accept_invalid_hostnames"
    )
}

impl<'tcx> LateLintPass<'tcx> for Explicit100NoSkippedVerification {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let id = match expr.kind {
            ExprKind::Call(called, _) => {
                let ExprKind::Path(ref qpath) = called.kind else {
                    return;
                };

                let Res::Def(DefKind::Fn | DefKind::AssocFn, id) = cx.qpath_res(qpath, called.hir_id)
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

        let named = cx.tcx.item_name(id);

        if !skips_the_check(named.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT100_NO_SKIPPED_VERIFICATION,
            expr.span,
            format!("`{named}` stops asking who is on the other end of the connection"),
            None,
            "leave verification on, and where a test runs a server of its own, hand the client that \
             server's certificate to trust. Where the peer is proved some other way -- a key pinned \
             in this tree -- allow this rule here and let the reason say what proves it",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
