// aux-build:console_program_contract.rs

// UI test for EXPLICIT041 — a program written to the contract says what it
// wants printed. The rule is asked only of the crates that link the contract,
// which is what the line above arranges.

extern crate console_program_contract;

use console_program_contract::Effect;

// BAD EXPLICIT041 — the transcript cannot see this.
fn says_it_itself(what: &str) {
    //~v EXPLICIT041_NO_UNSAID_PRINTING
    println!("{what}");
}

// GOOD — the other stream is the journal, which is where the sentence a fault
// makes is meant to go, and is not this rule.
fn complains(what: &str) {
    eprintln!("{what}");
}

// GOOD — handed back, so the runtime carries it out and the transcript has it.
fn says_it(what: &str) -> Effect {
    Effect::Print(what.to_string())
}

// GOOD — the one place a print stays a print, and the site says so.
#[cfg_attr(
    dylint_lib = "explicit041_no_unsaid_printing",
    allow(
        explicit041_no_unsaid_printing,
        reason = "this is what carries `Effect::Print` out, so something here has to be the thing that prints"
    )
)]
fn carries_it_out(said: &str) -> Effect {
    println!("{said}");

    Effect::Stop
}

fn main() {}
