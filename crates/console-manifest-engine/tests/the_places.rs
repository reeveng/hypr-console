//! Where this desktop keeps a person's things is asked in one place.
//!
//! `~/.config/console`, `~/.local/state/console`, `~/.local/share/console` and
//! `~/.cache/console` were spelled out in about a dozen crates, each joining
//! its own filename onto its own guess, and the guesses disagreed. Two of them
//! sent an unset `HOME` to `/tmp`, which is the `/root` fault with a different
//! address and a world-writable directory at the end of it. An `XDG_*_HOME`
//! set to nothing was a directory to some of them and no directory to others,
//! so the same empty variable put one program's notes in the person's home and
//! another's in whatever directory it happened to be started from. And a list
//! of all four, in the migration that moves a machine off the old name, had to
//! be kept in step with every one of those by hand.
//!
//! `console_core_places::Base` is the four of them and `OURS` is the word under
//! each, so the base is decided once and what goes in it stays the owning
//! crate's to name. This is what keeps the next one from writing the join out
//! again: the literal is what is looked for, because a crate that spells the
//! whole path has by definition stopped asking.
//!
//! The four are looked for with `console` on the end. A base on its own is a
//! word this tree has every right to say -- `.config/hypr` and `.config/mako`
//! are other programs' directories, `files/home/@user@/.config/...` is a path
//! in this repository rather than on a machine -- and it is the desktop's own
//! directory under a base that had drifted. Both ways of ending it are looked
//! for, the directory that goes on and the directory that stops, so that
//! `.cache/console-checks`, which is a sibling and not one of these, is
//! neither.
//!
//! What this does not see is a toolkit asked for a base. `glib::user_cache_dir`
//! and `glib::user_config_dir` are the same question with the same two answers
//! in them, and the files that ask one of them are every one a program drawn
//! with GTK, where the toolkit is already in the room. They are left standing:
//! each hands the directory to a function that joins the rest of the path onto
//! it, and those functions cannot yet say *there is no directory*, which is the
//! same sentence the fallbacks under `/tmp` needed and the reason this is a
//! sweep of its own rather than the tail of this one. Standing is a decision,
//! so the second guard names them: one more is a failure, and one that goes is
//! a line to take out.
//!
//! `console-core-places` is excused because it is the crate that answers.

mod reading;

use std::collections::BTreeSet;

use reading::{root, saying};

const EXCUSED: [&str; 1] = ["console-core-places"];

const BASES: [&str; 4] = [".config", ".local/state", ".local/share", ".cache"];

#[test]
fn one_crate_works_out_where_this_desktop_keeps_things() {
    let spelling: Vec<String> = BASES
        .iter()
        .flat_map(|base| [format!("{base}/console/"), format!("{base}/console\"")])
        .flat_map(|said| saying(&said, &EXCUSED))
        .collect();

    assert!(
        spelling.is_empty(),
        "these work out where this desktop keeps things for themselves; ask \
         console_core_places::Base for the directory and join your own name onto it: {spelling:?}"
    );
}

const BASES_OF_A_TOOLKIT: [&str; 4] =
    ["user_config_dir", "user_state_dir", "user_data_dir", "user_cache_dir"];

const ASKING_A_TOOLKIT: &[&str] = &[
    "crates/console-downloads/src/bin/download-find.rs",
    "crates/console-downloads/src/bin/one-format.rs",
    "crates/console-downloads/src/card.rs",
    "crates/console-files/src/bin/files-thumbs.rs",
    "crates/console-files/src/card.rs",
    "crates/console-music-panel/src/bin/music-index.rs",
    "crates/console-music-panel/src/card.rs",
    "crates/console-music-panel/src/library.rs",
    "crates/console-panel/src/style.rs",
];

#[test]
fn the_ones_that_ask_a_toolkit_for_a_base_are_the_ones_that_are_known() {
    let asking: BTreeSet<String> = BASES_OF_A_TOOLKIT
        .iter()
        .flat_map(|word| saying(word, &EXCUSED))
        .collect();

    let known: BTreeSet<String> =
        ASKING_A_TOOLKIT.iter().map(|at| root().join(at).display().to_string()).collect();

    assert_eq!(
        asking, known,
        "a toolkit is asked for a base somewhere new, or one of the named has stopped \
         asking; ask console_core_places::Base, and take the line out of \
         ASKING_A_TOOLKIT when the site goes"
    );
}

#[test]
fn the_crate_that_answers_still_holds_the_four() {
    let answering = root().join("crates/console-core-places/src/lib.rs");
    let said = std::fs::read_to_string(answering).expect("the crate that holds the answer");

    assert!(
        said.contains("pub const EVERY: [Base; 4]"),
        "a base has arrived or left, and the migration that moves a machine off the old name \
         is worked out from this list"
    );

    for base in BASES {
        assert!(
            said.contains(&format!("\"{base}\"")),
            "{base} is not one of the bases any more, and the crates that were under it are \
             somewhere else now"
        );
    }
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
