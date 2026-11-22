//! EXPLICIT082: a line a shell reads is written, not assembled.
//!
//! EXPLICIT036 took the program named by a string, because a name nothing
//! answers for is a claim about the manifest that nothing checks. This rule
//! is about the argument that comes after `-c`, which is not an argument at
//! all: it is a program in another language, and whatever is put into it with
//! `format!` is parsed by that language's rules rather than by anybody's
//! here. A file name with a space in it becomes two words, one with a `;` in
//! it becomes two commands, and the one place this tree did that as root --
//! `su owner -s /bin/sh -c` over a line with a command interpolated into it --
//! is the oldest fault there is, waiting for the day the command comes from
//! somewhere a person can write to.
//!
//! The spelling this rule asks for is the one a shell already has for it. The
//! script is a literal, so it is a text somebody read, and what varies is
//! handed over as an argument of its own, after the script, where the shell
//! binds it to `$1` and never parses it:
//!
//!     [sh, "-c", "command -v \"$1\"", "sh", named]
//!
//! It reads the element after a literal `-c` in a list of arguments, and the
//! `arg` after an `arg("-c")`. A literal, a `const` and a `static` are a
//! script somebody wrote; anything else is one that was built. It cannot know
//! which program the list is for, so a program whose `-c` means something
//! else -- snapper's names a configuration -- carries the allow and says so.
//!
//! It arrives `Warn`, with the engine's own `in_the_session` breaking it.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};

dylint_linting::declare_late_lint! {
    /// EXPLICIT082: the argument after `-c` is a script, and a script is a
    /// literal. What varies goes in an argument of its own, which the shell
    /// binds to `$1` and does not parse.
    pub EXPLICIT082_NO_BUILT_SHELL_LINE,
    Warn,
    "a shell line assembled at run time is parsed by the shell's rules, not by anybody's here"
}

// A literal reaches a list of arguments wearing whatever the list's type
// asked for: a borrow, a `to_string`, a `String::from`.
fn peeled<'tcx>(expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    match expr.kind {
        ExprKind::AddrOf(_, _, inner) => peeled(inner),
        ExprKind::MethodCall(path, receiver, [], _)
            if matches!(
                path.ident.name.as_str(),
                "to_string" | "to_owned" | "into" | "as_str" | "as_ref"
            ) =>
        {
            peeled(receiver)
        }
        ExprKind::Call(_, [inner]) if matches!(inner.kind, ExprKind::Lit(_)) => inner,
        _ => expr,
    }
}

fn spelled(expr: &Expr<'_>) -> Option<String> {
    let ExprKind::Lit(said) = peeled(expr).kind else {
        return None;
    };

    match said.node {
        rustc_ast::LitKind::Str(text, _) => Some(text.to_string()),
        _ => None,
    }
}

fn is_the_flag(expr: &Expr<'_>) -> bool {
    !expr.span.from_expansion() && spelled(expr).as_deref() == Some("-c")
}

fn was_written(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    let inner = peeled(expr);

    match inner.kind {
        ExprKind::Lit(_) => true,
        ExprKind::Path(ref qpath) => matches!(
            cx.qpath_res(qpath, inner.hir_id),
            Res::Def(DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::Static { .. }, _)
        ),
        _ => false,
    }
}

fn said(cx: &LateContext<'_>, script: &Expr<'_>) {
    span_lint_and_help(
        cx,
        EXPLICIT082_NO_BUILT_SHELL_LINE,
        script.span.source_callsite(),
        "this is the line a shell will parse, and it was put together at run time",
        None,
        "write the script as a literal and hand what varies over as an argument after it: \
         `[\"-c\", \"command -v \\\"$1\\\"\", \"sh\", named]` binds it to `$1`, which the shell never \
         parses. Where `-c` is not a script at all for this program, allow this rule here and let \
         the reason say what it is",
    );
}

impl<'tcx> LateLintPass<'tcx> for Explicit082NoBuiltShellLine {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            ExprKind::Array(listed) => {
                for pair in listed.windows(2) {
                    let [flag, script] = pair else {
                        continue;
                    };

                    if is_the_flag(flag) && !was_written(cx, script) {
                        said(cx, script);
                    }
                }
            }
            ExprKind::MethodCall(path, receiver, [script], _) if path.ident.name.as_str() == "arg" => {
                let ExprKind::MethodCall(before, _, [flag], _) = receiver.kind else {
                    return;
                };

                if before.ident.name.as_str() == "arg" && is_the_flag(flag) && !was_written(cx, script) {
                    said(cx, script);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
