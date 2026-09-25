//! EXPLICIT044: a function decides from what it was handed.
//!
//! EXPLICIT026 says a name out of the environment is read in one crate. This
//! is the same argument about the values a process keeps for itself: a
//! `static` that can be written after it has been read, and the two settings
//! -- the environment and the working directory -- that a process can change
//! under its own feet.
//!
//! A `static OnceLock` filled the first time someone asks is the shape most
//! of them take here, and it looks harmless because it is written once. What
//! it costs is not a race, it is an answer: the value is frozen for the life
//! of the process, so the second caller gets the first caller's answer to a
//! question it never saw asked, and no check can arrange for it to be
//! anything else. A locale cached on the first draw is the locale for every
//! draw afterwards, and the check that means to press the other one has
//! nowhere to stand. An `AtomicBool` or a `static Mutex` is the louder
//! version: it can be written at any moment by anyone, and what a function
//! reads out of it depends on what else the program happened to be doing.
//!
//! `set_var` and `remove_var` are the same fault crossing out of the process
//! entirely. They are unsafe in this edition for the honest reason -- the
//! environment is read by the C library from any thread and writing it races
//! with that -- and what they do here is arrange the ambient answer that
//! EXPLICIT026 is trying to get read in one place. `set_current_dir` is the
//! one with the longest reach: every relative path in the process means
//! something different afterwards, including the ones in crates that never
//! heard of the call. `current_dir` is denied with them, because a path
//! resolved against wherever the program was started from is the fault this
//! tree already knows by another name -- the stage is the tree somewhere
//! else, and an absolute answer read on this machine is not the device's.
//!
//! `thread_local!` is deliberately not here, and EXPLICIT026's head is why: a
//! `thread_local!` in a panel is there because glib's main context is
//! per-thread and what is held is not `Send`. Every violation would carry the
//! same sentence, which is paperwork rather than a rule. That argument was
//! made once and this rule does not reopen it.
//!
//! A `static` that cannot be written is not touched either, and is most of
//! them: a `const`-like table, a palette, a list of names. What is asked about
//! is a `static mut` and a `static` whose type has interior mutability, which
//! is the compiler's own question -- `Freeze` -- rather than a list of type
//! names that would go stale the first time someone reached for a different
//! cell.
//!
//! The answer at nearly every site is to hand the value in: the memo becomes a
//! field on something the caller already holds, filled where the caller can
//! see it filled. Where the process really is the only place it can live -- a
//! signal handler's flag, a connection a D-Bus callback is handed no state to
//! find -- the site carries the allow and EXPLICIT018 asks the reason to say
//! why nothing could hold it instead.
//!
//! It arrived `Warn` and is denied. What it found came out in two roughly equal
//! halves. The memos went: the language someone reads is asked again at every
//! `say`, the never-resume list is read again at every save, the player's two
//! `OnceLock`s became one value `main` holds and hands to the callback that
//! could not find it, the empty profile became a field, and the wallpaper's
//! two say-it-once flags became a `Told` handed in, which is the shape
//! `console-cpu-boost` already had. One was a fifth crate quietly deciding what
//! marks the top of this tree, and it now asks `console_repository` like the
//! other four.
//!
//! The other half carries the allow and is nearly all one thing: a signal
//! handler is handed nothing and may allocate nothing, so the pid it passes a
//! signal to, the descriptor it writes `close` down and the flag saying a run
//! was asked to stop have nowhere else to live. Beside them are a lock held for
//! exactly as long as the process it speaks for, a queue one writing thread
//! owns for the life of the process, an icon store whose slices are still being
//! drawn from, and a counter handing out a directory no one else is in. Each
//! says which.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind, Item, ItemKind, Mutability, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT044: a value the whole process shares is one a function reads
    /// without being handed it, and one no caller can arrange. A `static` that
    /// can be written, the environment and the working directory are all the
    /// same fault: hand it in instead.
    pub EXPLICIT044_NO_AMBIENT_VALUE,
    Deny,
    "a value read from the process rather than handed in"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// The two ways a process changes what every later reader of it will see, and
// the one way it reads a path out of where it happened to be started.
fn reaches_past_the_call(path: &str) -> Option<&'static str> {
    match path {
        "std::env::set_var" | "std::env::remove_var" => {
            Some("every later reader of that name gets this, including the crates that own it")
        }
        "std::env::set_current_dir" => {
            Some("every relative path in the process means something else after this line")
        }
        "std::env::current_dir" => {
            Some("this is wherever the program was started from, which is not a place anyone chose")
        }
        _ => None,
    }
}

// A `static` no one can write is a table and is not asked about. What is left
// is a `static mut`, and a `static` whose type has interior mutability --
// which is `Freeze`, the compiler's own question, rather than a list of cells
// that would go stale.
fn can_be_written(cx: &LateContext<'_>, item: &Item<'_>, mutability: Mutability) -> Option<&'static str> {
    match mutability {
        Mutability::Mut => return Some("a `static mut` is read and written by anything in the process"),
        Mutability::Not => {}
    }

    let held = cx.tcx.type_of(item.owner_id).instantiate_identity();

    match held.skip_normalization().is_freeze(cx.tcx, cx.typing_env()) {
        true => None,
        false => Some("this holds a value that can be filled or changed after someone has read it"),
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit044NoAmbientValue {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if item.span.from_expansion() {
            return;
        }

        let ItemKind::Static(mutability, named, ..) = item.kind else {
            return;
        };

        let Some(harm) = can_be_written(cx, item, mutability) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT044_NO_AMBIENT_VALUE,
            item.span,
            format!("`{named}` is a value the whole process shares: {harm}"),
            None,
            "hand it in. A memo filled on first use answers every later caller with the first caller's \
             answer, and nothing a check does can arrange for it to be anything else -- so it belongs on \
             something the caller already holds, filled where the caller can see it filled. Where the \
             process really is the only thing that can hold it, allow this rule here and let the reason \
             say why nothing else could",
        );
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::Call(called, _) = expr.kind else {
            return;
        };

        let ExprKind::Path(QPath::Resolved(_, path)) = called.kind else {
            return;
        };

        let Res::Def(DefKind::Fn, id) = path.res else {
            return;
        };

        let named = cx.tcx.def_path_str(id);

        let Some(harm) = reaches_past_the_call(named.as_str()) else {
            return;
        };

        span_lint_and_help(
            cx,
            EXPLICIT044_NO_AMBIENT_VALUE,
            expr.span,
            format!("`{named}` reaches past this call: {harm}"),
            None,
            "the environment is read in the crate that owns what a name means, which is EXPLICIT026, and \
             writing one from somewhere else arranges that crate's answer behind its back. A path is \
             joined to a root someone named -- `console_core_places` for a person's files, \
             `console_repository` for this tree -- rather than resolved against wherever the program was \
             started. Where a child really has to be handed a different environment, hand it to the \
             child: `Command::env` changes that process and not this one",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
