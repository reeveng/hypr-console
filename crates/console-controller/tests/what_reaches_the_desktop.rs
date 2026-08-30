//! What arrives at the desktop while a chooser is open.
//!
//! The chooser profiles publish a pad, because the on-screen keyboard reads
//! one and X has to reach it. A pad means every button arrives at one, so any
//! button the controller daemon acts on has to be named there, either given a
//! job or given none. Left out, it depends on whether InputPlumber passes an
//! unmapped button through, which is not written down anywhere and is not
//! worth a desktop resting on.
//!
//! The daemon's own tables are read for the answer, so a button given a job
//! there and forgotten in a chooser is a failure rather than a surprise.

use std::collections::BTreeSet;

use evdev::KeyCode;
use console_controller::buttons::{BUTTONS, SHOULDERS};
use console_pad::profile::{Source, load_all};
use console_pad::vocabulary::{GAMEPAD_CODES, spoken_for};

/// Every pad button the daemon does something about.
fn acts_on() -> BTreeSet<KeyCode> {
    BUTTONS
        .iter()
        .map(|(key, _)| *key)
        .chain(SHOULDERS.iter().map(|(key, _)| *key))
        .collect()
}

#[test]
fn every_button_the_desktop_acts_on_is_named_in_a_chooser() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository");
    let profiles = load_all(&root).expect("the profiles");
    // As above: the chooser profiles, asked as a list because that is what
    // the question is about.
    #[allow(clippy::single_element_loop)]
    for where_ in ["tabs"] {
        let named: BTreeSet<KeyCode> = profiles[where_]
            .mappings
            .iter()
            .filter_map(|mapping| match &mapping.source {
                Source::Button(name) => GAMEPAD_CODES
                    .iter()
                    .find(|(said, _)| said == name)
                    .map(|(_, code)| *code),
                _ => None,
            })
            .collect();
        let forgotten: Vec<&str> = GAMEPAD_CODES
            .iter()
            .filter(|(_, code)| acts_on().contains(code) && !named.contains(code))
            .map(|(said, _)| spoken_for(said))
            .collect();
        assert!(
            forgotten.is_empty(),
            "{where_}: {} would reach the desktop behind an open chooser",
            forgotten.join(", ")
        );
    }
}
