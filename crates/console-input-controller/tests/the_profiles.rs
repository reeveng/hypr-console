//! The rules `controller-profile` and the profiles keep to each other.  Every
//! word the switcher takes has to name a profile that will be on the machine,
//! and one of the two is not in this repository at all: the router is written
//! by `console buttons` out of what this device says it can send. So the check
//! is not "is the file here" for both -- it is "is it here, or is it the one
//! thing the engine writes", and a third path appearing that is neither is what
//! this is for.  These came from
//! `console-input-gamepad/tests/the_button_contract.rs`, where they read the
//! switcher's shell source and looked for paths in the text. It is a program
//! now, and this asks it.

use std::path::{Path, PathBuf};

use console_input_controller::profile::Which;
use console_input_gamepad::router::{self, PROFILES};
use console_program_contract::Argv;

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    from.canonicalize().unwrap_or(from)
}

fn every_word() -> Vec<(&'static str, Option<String>)> {
    ["router", "desktop", "tabs", "game"]
        .into_iter()
        .map(|word| {
            let Ok(argv) = Argv::of(&[word]);
            let Ok(which) = Which::of(&argv);
            let Ok(file) = which.file();

            (word, file)
        })
        .collect()
}

#[test]
fn every_profile_the_switcher_names_is_one_of_these() {
    let written = format!("{PROFILES}{}", router::FILE);

    for (word, file) in every_word() {
        let path = file.unwrap_or_else(|| panic!("{word} loads no profile at all"));

        match path == written {
            true => continue,
            false => {}
        }

        let held = root().join("files").join(path.trim_start_matches('/'));

        assert!(held.is_file(), "controller-profile loads {path} for {word}, which is not in the tree");
    }
}

#[test]
fn the_switcher_knows_the_profile_that_is_made_rather_than_kept() {
    let written = format!("{PROFILES}{}", router::FILE);
    let named: Vec<String> = every_word().into_iter().filter_map(|(_, file)| file).collect();

    assert!(
        named.contains(&written),
        "controller-profile takes no word for {written}, which is the profile the desktop wears"
    );
}

#[test]
fn a_word_nobody_defined_loads_nothing() {
    let Ok(keyboard) = Argv::of(&["keyboard"]);
    let Ok(unknown) = Which::of(&keyboard);
    let Ok(nothing) = Which::of(&Argv::default());

    assert_eq!(unknown.file(), Ok(None));
    assert_eq!(nothing.file(), Ok(None));
}
