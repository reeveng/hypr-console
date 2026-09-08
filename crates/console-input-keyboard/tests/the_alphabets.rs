//! The keyboard carries Thai, and did not lose the latin arrangements to it.
//!
//! She writes Thai, and the on-screen keyboard is the only keyboard this device
//! has. Thai is not the latin keyboard with accents on it: every key carries a
//! Thai letter and the shift level carries a second one rather than a capital,
//! so it is a layer of its own and the layer key is what reaches it.
//!
//! Asked of the program rather than of the table it is built from, because the
//! table is not the only thing between a layer existing and a person reaching
//! it: `named` has to find it by the word the walk uses, and `--list-layers` is
//! what a person would run to find that word out. Running it asks both at once.
//!
//! The walk used to be on the unit's command line and this asked the unit for
//! it. Which alphabets this machine types is a setting now --
//! `console_input_alphabets` is the list the panel writes and this reads -- so
//! the question moved with it: every arrangement anybody can choose has to be
//! one the keyboard has, or a row in the settings would be a row that does
//! nothing and says nothing about it.
//!
//! ## It used to live in `console-manifest-engine` and skip
//!  This was `crates/console-manifest-engine/tests/the_keyboard.rs`, which ran
//! the keyboard the tree carried at `files/usr/local/bin/virtual-keyboard` and
//! said "skipped: no keyboard in this tree" when there was none. That was right
//! while the keyboard was a compiled C binary committed to the tree and absent
//! from the public copy.  The device builds its own keyboard now and nothing is
//! carried, so the file that check looked for is never there and the check had
//! stopped running altogether -- silently, in the way it was written to be
//! silent for a different reason. Thai was the argument the whole keyboard
//! rests on, and it was the one thing nothing was asking any more.  Here rather
//! than there because `env!("CARGO_BIN_EXE_virtual-keyboard")` is the path to
//! the program this workspace just built, and it is only spelled that way
//! inside the crate that builds it. The other half of the old file -- whether
//! this machine has fonts that draw Thai -- stayed with the manifest, which is
//! where `[packages]` is.

use std::process::Command;

fn layers() -> Vec<String> {
    let listed = Command::new(env!("CARGO_BIN_EXE_virtual-keyboard"))
        .arg("--list-layers")
        .output()
        .expect("the keyboard answers --list-layers");
    assert!(listed.status.success(), "--list-layers did not answer: {listed:?}");
    String::from_utf8_lossy(&listed.stdout).split_whitespace().map(str::to_string).collect()
}

#[test]
fn the_keyboard_knows_thai() {
    let layers = layers();
    assert!(
        layers.iter().any(|layer| layer == "thai"),
        "the keyboard has no thai layer, only {layers:?}. It is the only keyboard on this device \
         and she writes Thai."
    );
}

#[test]
fn the_latin_layers_are_still_there() {
    let layers = layers();
    for wanted in ["full", "landscape", "landscapespecial", "special"] {
        assert!(
            layers.iter().any(|layer| layer == wanted),
            "the keyboard lost its {wanted} layer, and has {layers:?}"
        );
    }
}

#[test]
fn every_alphabet_somebody_can_choose_is_one_the_keyboard_has() {
    let layers = layers();

    for alphabet in &console_input_alphabets::EVERY {
        for wanted in [alphabet.upright, alphabet.across] {
            assert!(
                layers.iter().any(|layer| layer == wanted),
                "the settings panel offers {}, which walks the {wanted} arrangement, and the \
                 keyboard has {layers:?}. A layer nobody can find is dropped without a word, so \
                 choosing that alphabet would do nothing and say nothing.",
                alphabet.says
            );
        }
    }
}

#[test]
fn the_shelf_of_symbols_is_reachable_from_either_way_up() {
    let layers = layers();
    let Ok(chosen) = console_input_alphabets::read(console_input_alphabets::UNLESS_TOLD);

    for holding in [console_input_alphabets::Held::Upright, console_input_alphabets::Held::Across] {
        let Ok(walk) = console_input_alphabets::walk(&chosen, holding);

        for one in &walk {
            assert!(
                layers.iter().any(|layer| layer == one),
                "the walk this machine types with asks for {one}, and the keyboard has {layers:?}"
            );
        }
    }
}
