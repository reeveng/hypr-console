//! Where a person's home is is asked in one place.
//!
//! It was asked in four, and three of them answered differently when `HOME` was
//! unset: two said `/root`, one said the working directory, one said there was
//! no home. The `/root` answers are the ones worth a test. They do not fail --
//! they succeed, against someone else's dotfiles, reading settings no one
//! wrote and writing settings no one will look for -- and every one of them was
//! written by someone who had the same thought in a different file on a
//! different day. That is not a mistake a reviewer catches twice.
//!
//! So `console_core_places::home` is an `Option` and the crates that want a
//! home meet the `None`, and this is what keeps the next one from writing the
//! read out again. It is the literal that is looked for rather than
//! `env::var`, because the site that hid longest was one that had wrapped the
//! read in a helper of its own and asked the helper for `"HOME"`.
//!
//! A toolkit asked the same question says none of that word, and for a while
//! two crates asked it that way: `glib::home_dir` is not the raw read -- an
//! unset `HOME` sends it to the passwd entry rather than to `/root` -- so it
//! was not the fault the first guard was written for and it was left standing
//! while the toolkit was still in the room. It is not, in those two, any more,
//! so the second guard is the same guard: nobody asks anybody but this crate.
//! `the_places.rs` says the same of a toolkit asked for a base.
//!
//! `console-test-desktop` is excused because it does not ask where a home is:
//! it hands one over, standing an environment up for a desktop nested here.
//! `console-login-window` is excused for the same reason: it reads the home
//! out of the passwd entry of the person logging in and hands it to the
//! session it starts, which is where every other crate's `HOME` comes from.

mod reading;

use reading::{naming, root};

const EXCUSED: [&str; 3] = ["console-core-places", "console-login-window", "console-test-desktop"];

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
fn nobody_asks_a_toolkit_where_a_person_lives_either() {
    let asking = naming("home_dir", &EXCUSED);

    assert!(
        asking.is_empty(),
        "a toolkit is asked where a person's home is; ask console_core_places::home \
         and meet the None: {asking:?}"
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
