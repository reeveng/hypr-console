//! The machine carries the letters the keyboard draws Thai with.  She writes
//! Thai, and the on-screen keyboard is the only keyboard this device has. A
//! Thai letter with no font behind it is an empty box, on the keys and again in
//! whatever she typed it into, so a keyboard that knows the alphabet and a
//! machine that cannot draw it come to the same thing.  Whether the keyboard
//! knows the alphabet is `crates/console-input-keyboard`'s own question and
//! `tests/the_alphabets.rs` is where it moved to. It used to be here, and it
//! used to work by running the keyboard the tree carried at
//! `files/usr/local/bin/console-keyboard`, skipping when there was none --
//! which was right while the keyboard was a compiled C binary committed to the
//! tree and kept out of the public copy. The device builds its own now, nothing
//! is carried, and a check that looks for a carried keyboard is a check that
//! skips every time. Asked where the program is built, it cannot.  This half
//! stayed, because `[packages]` is the manifest's.

mod reading;

use reading::{Failure, read, Section, section};

fn packages() -> Result<Vec<String>, Failure> {
    let held = read("desktop.conf")?;
    let named = section(&held, Section::Packages)?;

    Ok(named)
}

#[test]
fn the_fonts_that_draw_thai_are_installed() -> Result<(), Failure> {
    let packages = packages()?;

    assert!(
        packages.iter().any(|name| name == "noto-fonts"),
        "nothing on the machine draws Thai: the keys and everything typed with them come out as \
         empty boxes"
    );

    Ok(())
}

#[test]
fn the_symbols_the_keyboard_composes_from_are_installed() -> Result<(), Failure> {
    let packages = packages()?;

    assert!(
        packages.iter().any(|name| name == "xkeyboard-config"),
        "the manifest does not ask for xkeyboard-config, which is where every alphabet the \
         keyboard offers comes from. Without it there is no keymap to compose and the keyboard \
         says so and stops."
    );

    Ok(())
}
