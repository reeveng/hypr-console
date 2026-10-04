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

use std::error::Error;
use std::path::{Path, PathBuf};

use console_core_never::Never;

use console_core_arguments::{Reason, read_with};
use console_input_controller::profile::{COMMAND, ProfileName, ProfileState};
use console_input_gamepad::router::{self, PROFILES};

fn root() -> Result<PathBuf, Never> {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    match from.canonicalize() {
        Ok(found) => Ok(found),
        Err(_not_there) => Ok(from),
    }
}

fn every_word() -> Result<Vec<(&'static str, Option<String>)>, Never> {
    Ok(ProfileName::VARIANTS
        .iter()
        .map(|named| {
            let Ok(word) = named.word();
            let Ok(which) = ProfileState::of(Some(*named));
            let Ok(file) = which.file();

            (word, file)
        })
        .collect())
}

#[test]
fn every_profile_the_switcher_names_is_one_of_these() -> Result<(), Box<dyn Error>> {
    let written = format!("{PROFILES}{}", router::FILE);
    let Ok(every) = every_word();
    let Ok(root) = root();

    for (word, file) in every {
        let path = file.ok_or_else(|| format!("{word} loads no profile at all"))?;

        match path == written {
            true => continue,
            false => {}
        }

        let held = root.join("files").join(path.trim_start_matches('/'));

        assert!(held.is_file(), "controller-profile loads {path} for {word}, which is not in the tree");
    }

    Ok(())
}

#[test]
fn the_switcher_knows_the_profile_that_is_made_rather_than_kept() {
    let written = format!("{PROFILES}{}", router::FILE);
    let Ok(every) = every_word();
    let named: Vec<String> = every.into_iter().filter_map(|(_, file)| file).collect();

    assert!(
        named.contains(&written),
        "controller-profile takes no word for {written}, which is the profile the desktop wears"
    );
}

#[test]
fn a_word_no_one_defined_is_refused_and_no_word_loads_nothing() {
    let keyboard = read_with::<ProfileName, &str>(&COMMAND, &["keyboard"]);
    let Ok(nothing) = ProfileState::of(None);

    assert_eq!(keyboard.map(drop).map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("keyboard".to_string())));
    assert_eq!(nothing.file(), Ok(None));
}
