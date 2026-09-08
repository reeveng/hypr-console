//! Where a person's home is is asked in one place.
//!
//! It was asked in four, and three of them answered differently when `HOME` was
//! unset: two said `/root`, one said the working directory, one said there was
//! no home. The `/root` answers are the ones worth a test. They do not fail --
//! they succeed, against somebody else's dotfiles, reading settings nobody
//! wrote and writing settings nobody will look for -- and every one of them was
//! written by somebody who had the same thought in a different file on a
//! different day. That is not a mistake a reviewer catches twice.
//!
//! So `console_core_places::home` is an `Option` and the crates that want a
//! home meet the `None`, and this is what keeps the next one from writing the
//! read out again. It is the literal that is looked for rather than
//! `env::var`, because the site that hid longest was one that had wrapped the
//! read in a helper of its own and asked the helper for `"HOME"`.
//!
//! A toolkit asked the same question says none of that word. `glib::home_dir`
//! is not the raw read -- an unset `HOME` sends it to the passwd entry rather
//! than to `/root` -- so it is not the fault the first guard was written for,
//! and the sites that take it are left standing until the crates holding them
//! are turned inside out. Left standing is still a decision, and a test that
//! says nothing about them cannot be told apart from a test that never looked:
//! the second guard names them, so one more is a failure and a site that goes
//! is a line to take out. `the_places.rs` says the same of a toolkit asked for
//! a base.
//!
//! `console-test-desktop` is excused because it does not ask where a home is:
//! it hands one over, standing an environment up for a desktop nested here.

mod reading;

use reading::{naming, root};

const EXCUSED: [&str; 2] = ["console-core-places", "console-test-desktop"];

const ASKING_A_TOOLKIT: &[&str] =
    &["crates/console-downloads/src/getting.rs", "crates/console-files/src/card.rs"];

#[test]
fn one_crate_asks_where_a_person_lives() {
    let asking = naming("\"HOME\"", &EXCUSED);

    assert!(
        asking.is_empty(),
        "these work out where a person's home is for themselves; ask \
         console_core_places::home and meet the None: {asking:?}"
    );
}

#[test]
fn the_ones_that_ask_a_toolkit_instead_are_the_ones_that_are_known() {
    let asking = naming("home_dir", &EXCUSED);
    let known: Vec<String> =
        ASKING_A_TOOLKIT.iter().map(|at| root().join(at).display().to_string()).collect();

    assert_eq!(
        asking, known,
        "a toolkit is asked where a person's home is somewhere new, or one of \
         the named has stopped asking; ask console_core_places::home, and take \
         the line out of ASKING_A_TOOLKIT when the site goes"
    );
}

#[test]
fn the_crate_that_answers_still_carries_the_absence() {
    let answering = root().join("crates/console-core-places/src/lib.rs");
    let said = std::fs::read_to_string(answering).expect("the crate that holds the answer");

    assert!(
        said.contains("pub fn home() -> Result<Option<PathBuf>, Never>"),
        "an absent home is no longer absent, and /root is back within reach"
    );
}

#[test]
fn every_excused_crate_is_a_crate() {
    let missing: Vec<&str> = EXCUSED
        .iter()
        .filter(|named| !root().join("crates").join(named).is_dir())
        .copied()
        .collect();

    assert!(missing.is_empty(), "excused from a rule but not in the tree: {missing:?}");
}
