//! The keyboard carries Thai, and the machine carries the letters to draw it.
//!
//! She writes Thai, and the on-screen keyboard is the only keyboard this device
//! has. Thai is not the latin keyboard with accents on it: every key carries a
//! Thai letter and the shift level carries a second one rather than a capital,
//! so it is a layer of its own and the layer key is what reaches it.
//!
//! The keyboard is a compiled fork, so which layers it knows is a property of
//! the file in the tree rather than of anything written here. The way to find
//! out is to ask it, which it answers without a compositor.
//!
//! A Thai letter with no font behind it is an empty box, on the keys and again
//! in whatever she typed it into, so the fonts are asked about here as well.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const KEYBOARD: &str = "files/usr/local/bin/wvkbd-mobintl";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

/// Every layer the keyboard in the tree knows, where the tree has one.
///
/// Asked once for the whole run, and not once per test that wants it.
///
/// Twice is not merely twice the work, it is a race. Running the keyboard means
/// forking, and a fork inherits every descriptor the process has open; while one
/// test is still writing its copy, another test's fork holds that copy open for
/// writing, and the kernel will not exec a file somebody has open for writing.
/// "Text file busy", about one run in ten, in a test whose whole subject is
/// which layers a keyboard has. Once means there is never a second copy in the
/// air to be caught mid-write.
fn layers() -> Option<&'static Vec<String>> {
    static KNOWN: OnceLock<Option<Vec<String>>> = OnceLock::new();
    KNOWN.get_or_init(asking).as_ref()
}

/// The asking itself, done the one time.
///
/// Copied out to be run: the tree keeps its files unexecutable, and the mode is
/// worked out by the engine when it installs them.
///
/// Nothing where the keyboard is not here. It is a compiled fork, and the public
/// copy of this repository carries neither it nor the answer it would give, so
/// asking it there is asking a question that has no wrong answer.
fn asking() -> Option<Vec<String>> {
    if !root().join(KEYBOARD).is_file() {
        eprintln!("skipped: no keyboard in this tree; see docs/forks.md");
        return None;
    }
    let here = std::env::temp_dir().join(format!("console-keyboard-{}", std::process::id()));
    std::fs::create_dir_all(&here).expect("somewhere to copy it");
    let runnable = here.join("wvkbd-mobintl");
    std::fs::copy(root().join(KEYBOARD), &runnable).expect("the keyboard");
    std::fs::set_permissions(&runnable, std::fs::Permissions::from_mode(0o755))
        .expect("something runnable");
    let listed = Command::new(&runnable).arg("--list-layers").output().expect("it answers");
    let _ = std::fs::remove_dir_all(&here);
    Some(String::from_utf8_lossy(&listed.stdout).split_whitespace().map(str::to_string).collect())
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
fn the_keyboard_knows_thai() {
    let Some(layers) = layers() else { return };
    assert!(layers.iter().any(|layer| layer == "thai"), "the keyboard has no thai layer, only {layers:?}");
}

/// Thai was added beside the layers the keyboard already had.
#[test]
fn the_latin_layers_are_still_there() {
    let Some(layers) = layers() else { return };
    for wanted in ["full", "landscape", "landscapespecial", "special"] {
        assert!(layers.iter().any(|layer| layer == wanted), "the keyboard lost its {wanted} layer");
    }
}

#[test]
fn the_fonts_that_draw_thai_are_installed() {
    assert!(
        packages().iter().any(|name| name == "noto-fonts"),
        "nothing on the machine draws Thai: the keys and everything typed with them come out as \
         empty boxes"
    );
}
