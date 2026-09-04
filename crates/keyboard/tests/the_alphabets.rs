//! The keyboard carries Thai, and did not lose the latin arrangements to it.
//!
//! She writes Thai, and the on-screen keyboard is the only keyboard this device
//! has. Thai is not the latin keyboard with accents on it: every key carries a
//! Thai letter and the shift level carries a second one rather than a capital,
//! so it is a layer of its own and the layer key is what reaches it.
//!
//! Asked of the program rather than of the table it is built from, because the
//! table is not the only thing between a layer existing and a person reaching
//! it: `named` has to find it by the word the unit's command line uses, and
//! `--list-layers` is what a person would run to find that word out. Running it
//! asks both at once.
//!
//! ## It used to live in `console-manifest` and skip
//!
//! This was `crates/console-manifest/tests/the_keyboard.rs`, which ran the
//! keyboard the tree carried at `files/usr/local/bin/virtual-keyboard` and said
//! "skipped: no keyboard in this tree" when there was none. That was right while
//! the keyboard was a compiled C binary committed to the tree and absent from
//! the public copy.
//!
//! The device builds its own keyboard now and nothing is carried, so the file
//! that check looked for is never there and the check had stopped running
//! altogether -- silently, in the way it was written to be silent for a
//! different reason. Thai was the argument the whole keyboard rests on, and it
//! was the one thing nothing was asking any more.
//!
//! Here rather than there because `env!("CARGO_BIN_EXE_virtual-keyboard")` is
//! the path to the program this workspace just built, and it is only spelled
//! that way inside the crate that builds it. The other half of the old file --
//! whether this machine has fonts that draw Thai -- stayed with the manifest,
//! which is where `[packages]` is.

use std::process::Command;

/// Every layer the keyboard knows, in the words `-l` and `--landscape-layers`
/// take.
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

/// Thai was added beside the layers the keyboard already had, not instead of
/// them.
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

/// Every layer the unit asks for on the command line is one the keyboard has.
///
/// The other half of the same fault, and the one that has actually happened:
/// a layer named in `console-keyboard.service` that `named` cannot find is
/// dropped without a word, and what that looks like is a language key that
/// walks past the alphabet somebody wanted.
#[test]
fn the_unit_asks_for_layers_the_keyboard_has() {
    let unit = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../files/etc/systemd/user/console-keyboard.service"),
    )
    .expect("console-keyboard.service");
    let asked: Vec<String> = unit
        .lines()
        .filter_map(|line| line.strip_prefix("ExecStart="))
        .flat_map(str::split_whitespace)
        .skip_while(|word| *word != "--landscape-layers" && *word != "-l")
        .nth(1)
        .unwrap_or_default()
        .split(',')
        .map(str::to_string)
        .collect();
    assert!(!asked.is_empty(), "the unit names no layers at all");

    let layers = layers();
    for one in &asked {
        assert!(
            layers.iter().any(|layer| layer == one),
            "console-keyboard.service asks for the layer {one}, which the keyboard does not have. \
             It has {layers:?}. A layer nobody can find is dropped without a word."
        );
    }
}
