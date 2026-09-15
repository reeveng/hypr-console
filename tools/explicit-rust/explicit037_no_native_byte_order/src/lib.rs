#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT037: a number laid out as bytes says which end it starts at.
    ///
    /// `to_ne_bytes` and `from_ne_bytes` are the byte order of whatever machine
    /// compiled the call, which is the one fact about the layout that is
    /// nowhere in the line. Everywhere those bytes are going -- a wire, a file,
    /// a device, another program -- has an order of its own, and the two agree
    /// today because this desktop runs on one kind of machine. That is the
    /// shape of every assumption this suite is against: correct, unwritten, and
    /// checked by nothing.
    ///
    /// `console-bus` is the crate this is about. A D-Bus message carries its
    /// byte order in the first byte of its own header -- the sender says `l` or
    /// `B` and the reader believes it -- so the order is a value that arrives
    /// rather than a property of whoever is reading, and a walk over a
    /// signature that used the native one would be right until the day
    /// something on the other end was not.
    ///
    /// It arrives with nothing to say, which is the point of writing it now.
    /// There is no `ne` in the tree today, and a rule that is already kept
    /// costs a reader nothing and holds the day somebody reaches for the
    /// spelling that looks like it means "no conversion". `to_le_bytes` and
    /// `to_be_bytes` say which end, and where the end is a value somebody was
    /// told, it is that value the code should be asking.
    pub EXPLICIT037_NO_NATIVE_BYTE_ORDER,
    Deny,
    "a number laid out in the machine's own byte order is a layout nobody wrote down"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

fn says_nothing_about_the_order(named: &str) -> bool {
    matches!(named, "to_ne_bytes" | "from_ne_bytes")
}

impl<'tcx> LateLintPass<'tcx> for Explicit037NoNativeByteOrder {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        if expr.span.from_expansion() {
            return;
        }

        let named = match expr.kind {
            ExprKind::MethodCall(path, receiver, _, _) => {
                match cx.typeck_results().expr_ty_adjusted(receiver).peel_refs().is_numeric() {
                    true => path.ident.name.as_str().to_string(),
                    false => return,
                }
            }
            ExprKind::Call(called, _) => match called.kind {
                ExprKind::Path(QPath::TypeRelative(of, segment)) => {
                    match cx.typeck_results().node_type(of.hir_id).is_numeric() {
                        true => segment.ident.name.as_str().to_string(),
                        false => return,
                    }
                }
                _ => return,
            },
            _ => return,
        };

        if !says_nothing_about_the_order(named.as_str()) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT037_NO_NATIVE_BYTE_ORDER,
            expr.span,
            format!("`{named}` lays a number out in whatever order this machine happens to use"),
            None,
            "say which end it starts at: `to_le_bytes` and `to_be_bytes`, and their `from_` pair. Where \
             something else declares the order -- a D-Bus header says `l` or `B` in its first byte -- it \
             is what that says that decides, not what the compiler was run on",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
