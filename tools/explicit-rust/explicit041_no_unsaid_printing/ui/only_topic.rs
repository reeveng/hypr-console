// aux-build:console_program_contract.rs

// UI test for EXPLICIT041 — naming the contract crate is not speaking it. This
// crate takes the event vocabulary and nothing else, the way a bar module does,
// and what it prints goes down a pipe to whatever is reading it. There is no
// transcript here to be blind to, so there is nothing to say.

extern crate console_program_contract;

use console_program_contract::Topic;

// GOOD — the whole output of this program, and no `Doing` anywhere in it.
fn draws(woken: Topic) {
    match woken {
        Topic::Player => println!("{{\"text\":\"playing\"}}"),
    }
}

fn main() {
    draws(Topic::Player);
}
