//! EXPLICIT041: a program written to the contract says what it wants printed.
//!
//! `console-program-contract` is a state, a word in, a new state and a list of
//! what the program wants done, and that list is already an inventory of every
//! way a program here touches anything outside itself: `Ask`, `Start`,
//! `Listen`, `Deafen`, `Write`, `Say`, `Stop` -- and `Print`. Nothing in it
//! touches a machine, which is the whole point: `transcript` presses a program
//! with words and reads back the doings, so what a program does is a value a
//! test can look at rather than an effect a test has to go and observe.
//!
//! A `println!` in the middle of that is output the contract never hears
//! about. The program still prints it -- the terminal is right there and
//! nothing can stop it -- but the transcript does not have it, so the one test
//! that presses the program is blind to most of what the program actually
//! emits, and the doctrine in `docs/checks.md` about asserting what a person
//! would see is being kept against the half of the output that happens to be
//! declared.
//!
//! It is the effect argument the rest of this suite makes about types, said
//! about the one effect the tree already has a name for. EXPLICIT036 denies a
//! program named by a string because no list of what this desktop runs can see
//! it; this denies output nothing can see for the same reason, and the answer
//! is the same shape: say it in the vocabulary the tree already has.
//!
//! **Only the crates that speak the contract are asked**, and speaking it is
//! not the same as having it underneath. The first writing of this rule asked
//! whether `console-program-contract` was anywhere in the crate graph, which is
//! true of nearly everything here and put the apply engine at the top of the
//! list -- a program whose printing is its whole job, drawing pacman's line
//! through `console-how-far`, and which has a `Said::Doing` of its own that has
//! nothing to do with any of this. So the question is asked of what the crate
//! names. That is answered for the whole crate rather than at the line, which
//! is why this is a stateful pass and why what it reports carries the node it
//! was written at.
//!
//! Naming the crate turned out not to be speaking it either, and the tree said
//! so on the first reading: five of the seven sites were `Topic`, which is the
//! event vocabulary rather than the contract, in a waybar module whose whole
//! output is one line of JSON down a pipe and in the session watcher. Neither
//! hands a `Doing` to anybody and neither has a transcript to be blind to. So
//! what is asked for now is the doings themselves, or the trait whose turn
//! returns them: a crate that names either had somewhere to put the line and
//! printed beside it instead. This is the same narrowing the rule made about
//! the stream, one step further in, and it is what leaves the rule with nothing
//! to say that is not a real one.
//!
//! **Only stdout is asked about.** The rule was written asking about both
//! streams and the first reading of the tree settled it: nearly everything it
//! found was `eprintln!`, and nearly every one of those was a program saying a
//! fault or a usage line -- which is the shape EXPLICIT038 already has an
//! answer for. A fault is a type with a `Display`, and the sentence that
//! `Display` makes is for the journal, which is what stderr is on this machine.
//! Denying it would have been an allow at almost every site, and EXPLICIT029
//! has already had this argument with itself and come to the same answer: a
//! rule whose answer is nearly always an allow is paperwork rather than a rule,
//! so the half that is not worth stopping for goes.
//!
//! What is left is the half that is: stdout is the program's output, the
//! transcript is what reads a program's output, and a `println!` is the one
//! spelling of it the transcript cannot see. The runtime is where the allow
//! belongs -- `console-program-runtime` is what carries `Doing::Print` out, and
//! something at the bottom has to be the thing that prints.
//!
//! What was left after both narrowings is the two lines at the bottom that
//! carry a `Doing::Print` out, which is the allow the head has always
//! prescribed, so this is denied. Every site it has left to find is a program
//! handing a person a line that its own test cannot read back.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::macros::root_macro_call_first_node;
use clippy_utils::sym;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, HirId, Path};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

dylint_linting::impl_late_lint! {
    /// EXPLICIT041: a program written to `console_program_contract` hands back
    /// a list of what it wants done, and `Doing::Print` is on that list. A
    /// `println!` beside it is output the transcript cannot see, in a program
    /// whose only test is the transcript.
    pub EXPLICIT041_NO_UNSAID_PRINTING,
    Deny,
    "printing from a program written to the contract without saying so",
    Explicit041NoUnsaidPrinting::new()
}

pub struct Explicit041NoUnsaidPrinting {
    printed: Vec<(&'static str, HirId, Span)>,
    speaks: bool,
}

impl Explicit041NoUnsaidPrinting {
    pub fn new() -> Self {
        Self { printed: Vec::new(), speaks: false }
    }
}

impl Default for Explicit041NoUnsaidPrinting {
    fn default() -> Self {
        Self::new()
    }
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// Naming the crate is not speaking it. `Topic` is the event vocabulary and is
// named by a bar module and by the session watcher, neither of which hands a
// `Doing` to anybody; what makes a crate one of these programs is that it names
// the doings, or the trait whose turn returns them.
fn speaks_the_contract(cx: &LateContext<'_>, named: DefId) -> bool {
    match cx.tcx.crate_name(named.krate).as_str() == "console_program_contract" {
        false => false,
        true => cx
            .tcx
            .def_path_str(named)
            .split("::")
            .any(|segment| matches!(segment, "Doing" | "Program")),
    }
}

// The two that write to stdout, read off the diagnostic item rather than off
// the spelling. The stderr pair is deliberately absent and the head says why.
fn prints(cx: &LateContext<'_>, called: DefId) -> Option<&'static str> {
    let streams = [(sym::print_macro, "print!"), (sym::println_macro, "println!")];

    streams
        .into_iter()
        .find(|(named, _)| cx.tcx.is_diagnostic_item(*named, called))
        .map(|(_, spelled)| spelled)
}

impl<'tcx> LateLintPass<'tcx> for Explicit041NoUnsaidPrinting {
    fn check_path(&mut self, cx: &LateContext<'tcx>, path: &Path<'tcx>, _: HirId) {
        let Res::Def(_, named) = path.res else {
            return;
        };

        match speaks_the_contract(cx, named) {
            true => self.speaks = true,
            false => {}
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        let Some(called) = root_macro_call_first_node(cx, expr) else {
            return;
        };

        let Some(named) = prints(cx, called.def_id) else {
            return;
        };

        self.printed.push((named, expr.hir_id, called.span));
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        if is_test_build(cx) {
            return;
        }

        if !self.speaks {
            return;
        }

        for (named, node, at) in self.printed.iter() {
            span_lint_hir_and_then(
                cx,
                EXPLICIT041_NO_UNSAID_PRINTING,
                *node,
                *at,
                format!(
                    "`{named}` prints without saying so: the transcript cannot see this, and the \
                     transcript is what presses this program"
                ),
                |said| {
                    said.help(
                        "hand it back instead: `Doing::Print` is on the list the program returns, the \
                         runtime carries it out, and `transcript` can then assert what the program said. \
                         A fault or a usage line is a different question and is not this one -- that goes \
                         to stderr, which is the journal, and EXPLICIT038 has already asked it to be a \
                         type with a sentence. Where this is the thing that carries a `Doing::Print` out, \
                         allow this rule at the site and let the reason say so",
                    );
                },
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
