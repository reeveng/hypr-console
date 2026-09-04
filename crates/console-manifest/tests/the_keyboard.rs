//! The machine carries the letters the keyboard draws Thai with.
//!
//! She writes Thai, and the on-screen keyboard is the only keyboard this device
//! has. A Thai letter with no font behind it is an empty box, on the keys and
//! again in whatever she typed it into, so a keyboard that knows the alphabet
//! and a machine that cannot draw it come to the same thing.
//!
//! Whether the keyboard knows the alphabet is `crates/keyboard`'s own question
//! and `tests/the_alphabets.rs` is where it moved to. It used to be here, and
//! it used to work by running the keyboard the tree carried at
//! `files/usr/local/bin/virtual-keyboard`, skipping when there was none --
//! which was right while the keyboard was a compiled C binary committed to the
//! tree and kept out of the public copy. The device builds its own now, nothing
//! is carried, and a check that looks for a carried keyboard is a check that
//! skips every time. Asked where the program is built, it cannot.
//!
//! This half stayed, because `[packages]` is the manifest's.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

/// The package names under [packages] in the manifest.
fn packages() -> Vec<String> {
    let said = std::fs::read_to_string(root().join("desktop.conf")).expect("the manifest");
    let mut section = String::new();
    let mut named = Vec::new();
    for line in said.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        match (line.starts_with('['), line.ends_with(']'), line.is_empty()) {
            (true, true, _) => section = line[1..line.len() - 1].to_string(),
            (_, _, false) if section == "packages" => named.push(line.to_string()),
            _ => (),
        }
    }
    named
}

#[test]
fn the_fonts_that_draw_thai_are_installed() {
    assert!(
        packages().iter().any(|name| name == "noto-fonts"),
        "nothing on the machine draws Thai: the keys and everything typed with them come out as \
         empty boxes"
    );
}

/// And the symbols the keyboard composes its alphabets out of.
///
/// The keyboard ships no keymap. `keyboard::keymap` asks xkbcommon for one, and
/// xkbcommon reads `/usr/share/X11/xkb/symbols`, so Thai is a layer this device
/// can type because `xkeyboard-config` is installed and for no other reason. It
/// arrived with the base install and was never named here, which is the same
/// gap the toolchain had: a device rebuilt from this manifest alone is not
/// promised it, and what that looks like is a keyboard that will not start.
#[test]
fn the_symbols_the_keyboard_composes_from_are_installed() {
    assert!(
        packages().iter().any(|name| name == "xkeyboard-config"),
        "the manifest does not ask for xkeyboard-config, which is where every alphabet the \
         keyboard offers comes from. Without it there is no keymap to compose and the keyboard \
         says so and stops."
    );
}
