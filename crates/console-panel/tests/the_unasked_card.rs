//! A card that cannot ask its question answers neither yes nor no.
//!
//! A zero from `console-confirm` is a yes, so every way of calling it that it
//! cannot read -- no question, two, an empty one, the word for its own usage,
//! no word for the row that goes ahead -- comes back as `CONFIRM_UNASKED`,
//! which `console-deploy` turns into the question at its own terminal. The
//! environment is emptied first, so a card that did get past its refusal has
//! no screen to come up on and cannot answer anything either.

use std::process::Command;

use console_core_internal_programs::{CONFIRM_DOES, CONFIRM_UNASKED};

const CARD: &str = env!("CARGO_BIN_EXE_console-confirm");

#[test]
fn every_call_the_card_cannot_read_comes_back_unasked() -> Result<(), std::io::Error> {
    let calls: [(&[&str], &str); 5] = [
        (&[], "Go"),
        (&["--help"], "Go"),
        (&["   "], "Go"),
        (&["Go ahead?", "Really?"], "Go"),
        (&["Go ahead?"], ""),
    ];

    for (asked, does) in calls {
        let ran = Command::new(CARD).args(asked).env_clear().env(CONFIRM_DOES, does).output()?;

        assert_eq!(ran.status.code(), Some(i32::from(CONFIRM_UNASKED)), "{asked:?} with {CONFIRM_DOES}={does:?}");
    }

    Ok(())
}
