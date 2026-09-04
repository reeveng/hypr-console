//! What the keyboard calls itself, and what the desktop looks for.
//!
//! The compositor is the only thing that knows the keyboard is up. It knows it
//! by the name on the layer surface, and several things have to agree about
//! that name: the program that publishes it, the daemon that stands down when
//! it sees it, the script that raises it, and the manifest that installs it.
//!
//! All of them have disagreed. The crate was renamed on the way to a Rust port
//! and `mode::KEYBOARD` moved to `virtual-keyboard` with it, while the program
//! went on publishing `wvkbd`. Nothing failed. The daemon simply never saw a
//! keyboard again: it went on reading the pad while the keyboard read the same
//! pad, which is the flicker on the right stick that `Mode::Keyboard` exists to
//! prevent, and a panel closed under a keyboard went back to putting the wrong
//! profile on. Every unit test still passed, because the tests were moved to
//! the new name in the same breath. Then the script that raises it was moved to
//! the new name while the binary kept the old one, which is X doing nothing.
//!
//! ## What the switch to Rust settled, and what it did not
//!
//! This file used to ask the question twice: of the C's `main.c`, and of the
//! compiled program the tree carried beside it. They were two facts, and the
//! bug was always the same one -- a source edited without a rebuild is a device
//! that goes on publishing the old name.
//!
//! The device compiles the keyboard now, so those two cannot drift apart: there
//! is no carried program to be stale. `surface.rs` holds the name and a test
//! beside it holds that name against the controller's.
//!
//! What is left is the part the compiler still cannot see. The name is a string
//! in four files that do not import each other -- the crate's `[[bin]]`, the
//! manifest's `[build]`, the unit's `ExecStart`, and a `pkill` pattern in a
//! shell script -- and nothing but this file reads all four.
//!
//! The unit is the newest of the four and the one with the quietest failure.
//! It used to name a starter that read the palette and exec'd the keyboard,
//! and `console_manifest::units::named_by` reads the absolute paths off a
//! unit's Exec lines to decide whether a file just written means that unit is
//! now running the wrong thing. It cannot see through one program into the
//! program that one starts, so a rebuilt keyboard restarted nothing and the
//! device went on running the release before it -- a machine that matches the
//! manifest and behaves like the version before it.

use std::path::Path;

/// The repository root, from this crate.
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

/// The names under one heading of the manifest.
fn section(held: &str, want: &str) -> Vec<String> {
    held.lines()
        .skip_while(|line| line.trim() != format!("[{want}]"))
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The name this crate builds a program under.
fn the_bin() -> String {
    let held = std::fs::read_to_string(root().join("crates/keyboard/Cargo.toml"))
        .expect("the keyboard's manifest");
    // The `[[bin]]` whose path is the keyboard itself, rather than the starter.
    let mut name = None;
    let mut seen = None;
    for line in held.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("name = ") {
            seen = rest.trim().trim_matches('"').to_string().into();
        }
        if line.contains("src/bin/keyboard.rs") {
            name = seen.clone();
        }
    }
    name.expect("a [[bin]] built from src/bin/keyboard.rs")
}

/// The keyboard is what the desktop looks for.
///
/// `console_door::up` matches on the front of a namespace, so the constant has
/// to be a prefix of what the program publishes rather than equal to it.
/// `surface.rs` is where the program's own name lives, and the test beside it
/// holds the two together; this is the half that has to hold outside the crate.
#[test]
fn the_program_the_manifest_builds_is_the_one_the_desktop_looks_for() {
    let looked_for = console_controller::mode::KEYBOARD;
    assert_eq!(
        the_bin(),
        looked_for,
        "this crate builds a program under a different name than the one the desktop looks for. \
         A daemon that never sees the keyboard never stands down, and both of them go on reading \
         the pad."
    );
    assert!(
        section(&manifest(), "build").iter().any(|name| name == looked_for),
        "{looked_for} is not in the manifest's [build], so the device never compiles it"
    );
}

/// And nothing carries a compiled copy of it beside the one it builds.
///
/// This is the bug the switch to Rust was supposed to end, written down so that
/// it cannot come back quietly. A program that is both built and carried is a
/// program where the carried one wins or loses by the order `console apply`
/// happens to do things in, and the one that loses is the one somebody edited.
#[test]
fn the_keyboard_is_built_and_not_also_carried() {
    let looked_for = console_controller::mode::KEYBOARD;
    let carried = format!("/usr/local/bin/{looked_for}");
    assert!(
        !section(&manifest(), "files").iter().any(|path| path == &carried),
        "{carried} is carried in [files] as well as built in [build]"
    );
    assert!(
        !root().join("files/usr/local/bin").join(looked_for).exists(),
        "there is a compiled keyboard in the tree again. The device builds this one now, and a \
         carried copy is the stale-binary bug this file exists for."
    );
}

/// The script that raises it aims at the program that gets installed.
///
/// `keyboard-toggle` matches the command line rather than the process name,
/// because `virtual-keyboard` is sixteen characters and the kernel keeps
/// fifteen -- see `console-manifest`'s own test. What it matches has to be the
/// path the unit starts, or X does nothing at all.
#[test]
fn the_toggle_aims_at_the_path_the_unit_starts() {
    let toggle = std::fs::read_to_string(root().join("files/usr/local/bin/keyboard-toggle"))
        .expect("keyboard-toggle");
    let path = keyboard::palette::VIRTUAL_KEYBOARD;
    assert!(
        toggle.contains(path),
        "keyboard-toggle does not mention {path}, which is what the unit starts"
    );
    assert!(
        path.ends_with(console_controller::mode::KEYBOARD),
        "{path} is not the program the desktop looks for"
    );
}

/// And the unit starts the keyboard rather than something that starts it.
///
/// The whole of the fix, asked as a question. `named_by` reads absolute paths
/// off `Exec` lines, so the keyboard is only restarted by a release that
/// rebuilds it if this unit names the keyboard itself. A unit that names any
/// other program in `/usr/local/bin` has put a program between the two again,
/// and the failure is silent: the apply says it wrote a keyboard and the device
/// goes on running the one before it.
#[test]
fn the_unit_names_the_keyboard_and_not_something_that_starts_it() {
    let unit = std::fs::read_to_string(
        root().join("files/etc/systemd/user/console-keyboard.service"),
    )
    .expect("console-keyboard.service");
    let path = keyboard::palette::VIRTUAL_KEYBOARD;
    let started: Vec<&str> = unit
        .lines()
        .filter_map(|line| line.strip_prefix("ExecStart="))
        .flat_map(str::split_whitespace)
        .filter(|word| word.starts_with('/'))
        .collect();
    assert_eq!(
        started,
        [path],
        "console-keyboard.service starts {started:?} rather than {path} alone. A program between \
         the unit and the keyboard is a program `named_by` cannot see through, and a rebuilt \
         keyboard then restarts nothing."
    );
}

/// A keyboard over the desktop is furniture and not a window.
#[test]
fn the_keyboard_is_furniture() {
    assert!(
        console_controller::mode::FURNITURE.contains(&console_controller::mode::KEYBOARD),
        "the keyboard is not in FURNITURE, so a keyboard over the desktop is read as a window"
    );
}
