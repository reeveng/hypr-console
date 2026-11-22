//! EXPLICIT099: a path is compared as a path, and an address as an address.
//!
//! `"/home/ab".starts_with("/home/a")` is true. A string has no idea where a
//! component ends, so a prefix test on text answers a question about
//! characters, and the question that was meant -- is this under that -- is a
//! question about components. The two agree on every path anybody tried.
//! They part on the sibling whose name begins the same way, which is the
//! directory somebody makes when they want to be on the inside of a check:
//! `/home/someone-else` passes for `/home/someone`, and
//! `https://example.com.elsewhere.net` passes for `https://example.com`.
//! The check is there, it is read and believed, and it holds for everything
//! except the one input written to get past it.
//!
//! The parsed thing has the right question on it. `Path::starts_with` and
//! `Path::strip_prefix` compare component by component, and a URL that has
//! been parsed has a host that is equal to another host or is not. The rule
//! is the one the whole tree is written to -- a format is read in one place,
//! by the crate that understands it, and what is handed on is what it meant
//! rather than how it was spelled.
//!
//! What it reads is narrow so that it is right: `starts_with` and
//! `strip_prefix` on text, against a literal that is a path or carries a
//! scheme and does not end in `/`. A literal that ends in one is its own
//! boundary -- `"/home/someone/"` cannot match a sibling, and `"https://"`
//! asks only for a scheme -- and a single `/` asks whether a path is
//! absolute, which text can answer.
//!
//! It arrives `Warn`.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, LangItem};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT099: no `str::starts_with` or `str::strip_prefix` against a
    /// literal path or address with no `/` at its end. Ask the `Path`, or
    /// the parsed address.
    pub EXPLICIT099_NO_PATH_AS_TEXT,
    Warn,
    "a prefix test on text does not know where a path component or a host name ends"
}

fn is_text(cx: &LateContext<'_>, receiver: &Expr<'_>) -> bool {
    match cx.typeck_results().expr_ty_adjusted(receiver).peel_refs().kind() {
        ty::Str => true,
        ty::Adt(made, _) => cx.tcx.is_lang_item(made.did(), LangItem::String),
        _ => false,
    }
}

fn spelled(expr: &Expr<'_>) -> Option<String> {
    let inner = match expr.kind {
        ExprKind::AddrOf(_, _, inner) => inner,
        _ => expr,
    };

    let ExprKind::Lit(said) = inner.kind else {
        return None;
    };

    match said.node {
        rustc_ast::LitKind::Str(text, _) => Some(text.to_string()),
        _ => None,
    }
}

fn has_no_boundary(prefix: &str) -> bool {
    let a_path = prefix.starts_with('/') && prefix.len() > 1;
    let an_address = prefix.contains("://");

    (a_path || an_address) && !prefix.ends_with('/')
}

impl<'tcx> LateLintPass<'tcx> for Explicit099NoPathAsText {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::MethodCall(path, receiver, [prefix], _) = expr.kind else {
            return;
        };

        let named = path.ident.name.as_str();

        if !matches!(named, "starts_with" | "strip_prefix") || !is_text(cx, receiver) {
            return;
        }

        let Some(prefix) = spelled(prefix) else {
            return;
        };

        if !has_no_boundary(prefix.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT099_NO_PATH_AS_TEXT,
            expr.span,
            format!("`{named}(\"{prefix}\")` on text is also true of whatever merely begins with those characters"),
            None,
            "ask `Path::starts_with` or `Path::strip_prefix`, which compare whole components, or \
             parse the address and compare its host. Where the text really is being asked about \
             characters, allow this rule here and let the reason say so",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
