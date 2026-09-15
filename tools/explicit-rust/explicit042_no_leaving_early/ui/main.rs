// UI test for EXPLICIT042 — a program ends by returning from `main`, not by
// leaving from wherever it happened to be standing.

use std::process::{ExitCode, ExitStatus};

// BAD EXPLICIT042 — the stack is not unwound, so nothing held is dropped.
fn says_and_goes() {
    //~v EXPLICIT042_NO_LEAVING_EARLY
    std::process::exit(1);
}

// BAD EXPLICIT042 — a `use` of the same function is the same leaving.
fn imported() {
    use std::process::exit;

    //~v EXPLICIT042_NO_LEAVING_EARLY
    exit(2);
}

// BAD EXPLICIT042 — the same fault without even the flush.
fn worse() {
    //~v EXPLICIT042_NO_LEAVING_EARLY
    std::process::abort();
}

// GOOD — the fault comes back and the end of the program is a return.
fn answers(well: bool) -> ExitCode {
    match well {
        true => ExitCode::SUCCESS,
        false => ExitCode::FAILURE,
    }
}

// GOOD — a number read off somebody else's process is not a way of leaving.
fn what_it_said(status: ExitStatus) -> Option<i32> {
    status.code()
}

fn main() {}
