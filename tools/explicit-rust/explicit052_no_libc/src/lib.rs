#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{HirId, Path};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT052: the kernel is asked through rustix, and never through libc.
    ///
    /// libc is the C library's own spelling of a system call: an `int` for a
    /// file, `-1` and `errno` for a failure, a raw pointer for a buffer, and
    /// every call `unsafe` because nothing in the signature says what it may
    /// touch. rustix asks the same kernel with an `OwnedFd` that closes itself,
    /// a `Result` whose error names the errno, and a borrow the compiler holds
    /// the call to. Most of what this tree reached libc for -- a signal sent,
    /// a file locked, a memory mapping, a pipe, a poll -- rustix already says
    /// the Rust way, and a call written against libc is one more place where a
    /// descriptor can leak or a failure can read as a number.
    ///
    /// What rustix will not do is install a signal handler into a process
    /// that also links libc: its `runtime` module says in so many words that
    /// it is not for a program libc is running. `console-signals` declares
    /// glibc's `signal` itself for that one call, so nothing needs the `libc`
    /// crate, and a site that reaches for it is a site that should be asking
    /// rustix or `console-signals` instead.
    pub EXPLICIT052_NO_LIBC,
    Deny,
    "a system call is asked through rustix, not libc"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

impl<'tcx> LateLintPass<'tcx> for Explicit052NoLibc {
    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, _: HirId) {
        if is_test_build(cx) || path.span.from_expansion() {
            return;
        }
        let Some(id) = path.res.opt_def_id() else {
            return;
        };
        let libc = rustc_span::Symbol::intern("libc");
        let spelled = path.segments.first().is_some_and(|first| first.ident.name == libc);
        if cx.tcx.crate_name(id.krate) != libc && !spelled {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT052_NO_LIBC,
            path.span,
            "this is the C library's spelling of a system call",
            None,
            "ask the kernel through rustix, which hands back an owned descriptor and a `Result`; installing a \
             signal handler is the one thing rustix will not do beneath glibc, and `console-signals` is where \
             that is done",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
