//! What the keyboard calls itself, and what the desktop looks for.
//!
//! The compositor is the only thing that knows the keyboard is up. It knows it
//! by the name on the layer surface, and several things have to agree about
//! that name: the program that publishes it, the daemon that stands down when
//! it sees it, the script that raises it, and the manifest that installs it.
//!
//! All of them have disagreed. The crate was renamed on the way to a Rust port
//! and `mode::KEYBOARD` moved to `console-keyboard` with it, while the program
//! went on publishing `wvkbd`. Nothing failed. The daemon simply never saw a
//! keyboard again: it went on reading the pad while the keyboard read the same
//! pad, which is the flicker on the right stick that `Mode::Keyboard` exists to
//! prevent, and a panel closed under a keyboard went back to putting the wrong
//! profile on. Every unit test still passed, because the tests were moved to
//! the new name in the same breath. Then the script that raises it was moved to
//! the new name while the binary kept the old one, which is X doing nothing.
//!
//! ## What the switch to Rust settled, and what it did not
//!  This file used to ask the question twice: of the C's `main.c`, and of the
//! compiled program the tree carried beside it. They were two facts, and the
//! bug was always the same one -- a source edited without a rebuild is a device
//! that goes on publishing the old name.  The device compiles the keyboard now,
//! so those two cannot drift apart: there is no carried program to be stale.
//! `surface.rs` holds the name and a test beside it holds that name against the
//! controller's.  What is left is the part the compiler still cannot see. The
//! name is a string in four files that do not import each other -- the crate's
//! `[[bin]]`, the manifest's `[build]`, the unit's `ExecStart`, and a `pkill`
//! pattern in a shell script -- and nothing but this file reads all four.  The
//! unit is the newest of the four and the one with the quietest failure. It
//! used to name a starter that read the palette and exec'd the keyboard, and
//! `console_manifest_engine::units::named_by` reads the absolute paths off a
//! unit's Exec lines to decide whether a file just written means that unit is
//! now running the wrong thing. It cannot see through one program into the
//! program that one starts, so a rebuilt keyboard restarted nothing and the
//! device went on running the release before it -- a machine that matches the
//! manifest and behaves like the version before it.

use std::path::Path;

use console_manifest_engine::manifest::{Manifest, Section};

type Failure = Box<dyn std::error::Error>;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn manifest() -> Result<Manifest, Failure> {
    let held = std::fs::read_to_string(Path::new(ROOT).join(console_repository::MARK))?;
    let manifest = Manifest::read(&held)?;

    Ok(manifest)
}

fn section(manifest: &Manifest, want: Section) -> Result<Vec<String>, Failure> {
    let Ok(entries) = manifest.of(want);

    Ok(entries.to_vec())
}

fn the_bin() -> Result<String, Failure> {
    let held = std::fs::read_to_string(Path::new(ROOT).join("crates/console-input-keyboard/Cargo.toml"))?;
    let mut name = None;
    let mut seen = None;

    for line in held.lines().map(str::trim) {
        match line.strip_prefix("name = ") {
            Some(rest) => seen = Some(rest.trim().trim_matches('"').to_string()),
            None => {},
        }

        match line.contains("src/bin/keyboard.rs") {
            true => name = seen.clone(),
            false => {},
        }
    }

    let name = name.ok_or("a [[bin]] built from src/bin/keyboard.rs")?;

    Ok(name)
}

#[test]
fn the_program_the_manifest_builds_is_the_one_the_desktop_looks_for() -> Result<(), Failure> {
    let looked_for = console_input_controller::mode::KEYBOARD;
    let built = the_bin()?;
    let manifest = manifest()?;
    let building = section(&manifest, Section::Build)?;

    assert_eq!(
        built,
        looked_for,
        "this crate builds a program under a different name than the one the desktop looks for. \
         A daemon that never sees the keyboard never stands down, and both of them go on reading \
         the pad."
    );
    assert!(
        building.iter().any(|name| name == looked_for),
        "{looked_for} is not in the manifest's [build], so the device never compiles it"
    );

    Ok(())
}

#[test]
fn the_keyboard_is_built_and_not_also_carried() -> Result<(), Failure> {
    let looked_for = console_input_controller::mode::KEYBOARD;
    let carried = format!("/usr/local/bin/{looked_for}");
    let manifest = manifest()?;
    let files = section(&manifest, Section::Files)?;

    assert!(
        !files.contains(&carried),
        "{carried} is carried in [files] as well as built in [build]"
    );
    assert!(
        !Path::new(ROOT).join("files/usr/local/bin").join(looked_for).exists(),
        "there is a compiled keyboard in the tree again. The device builds this one now, and a \
         carried copy is the stale-binary bug this file exists for."
    );

    Ok(())
}

#[test]
fn the_unit_starts_the_program_the_desktop_looks_for() {
    let path = console_input_keyboard::palette::VIRTUAL_KEYBOARD;
    assert!(
        path.ends_with(console_input_controller::mode::KEYBOARD),
        "{path} is not the program the desktop looks for"
    );
}

#[test]
fn the_toggle_the_show_and_the_keyboard_are_all_built() -> Result<(), Failure> {
    let held = manifest()?;
    let built = section(&held, Section::Build)?;

    for name in ["keyboard-toggle", "keyboard-show", console_input_controller::mode::KEYBOARD] {
        assert!(
            built.contains(&name.to_string()),
            "[build] does not name {name}, so pressing X would ask a keyboard nothing installed"
        );
    }

    Ok(())
}

#[test]
fn the_unit_names_the_keyboard_and_not_something_that_starts_it() -> Result<(), Failure> {
    let unit = std::fs::read_to_string(
        Path::new(ROOT).join("files/etc/systemd/user/console-input-keyboard.service"),
    )?;
    let path = console_input_keyboard::palette::VIRTUAL_KEYBOARD;
    let started: Vec<&str> = unit
        .lines()
        .filter_map(|line| line.strip_prefix("ExecStart="))
        .flat_map(str::split_whitespace)
        .filter(|word| word.starts_with('/'))
        .collect();
    assert_eq!(
        started,
        [path],
        "console-input-keyboard.service starts {started:?} rather than {path} alone. A program between \
         the unit and the keyboard is a program `named_by` cannot see through, and a rebuilt \
         keyboard then restarts nothing."
    );

    Ok(())
}

#[test]
fn the_keyboard_is_a_system_surface() {
    assert!(
        console_input_controller::mode::SYSTEM_SURFACES.contains(&console_input_controller::mode::KEYBOARD),
        "the keyboard is not in SYSTEM_SURFACES, so a keyboard over the desktop is read as a window"
    );
}
