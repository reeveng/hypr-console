//! What a session is on disk, asked without a compositor.
//!
//! What a session *does* -- windows closing, programs starting, a window landing
//! on the workspace it was saved from -- cannot be answered here, and asserting
//! that the right Lua was composed would be a plumbing assertion rather than
//! coverage. That half belongs to `console-test-checks`, against a screen.
//!
//! What is answerable here is everything the program does to a directory: which
//! sessions it can see, what it does with a name no one has saved, and that
//! throwing one away leaves the others alone. Those are the paths a person
//! reaches by typing, and each of them once ran through an `unwrap` on a
//! directory that might not be there.

use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_resume::session::{Duplicates, Restore, Really, Restoring, Sessions};
fn scratch(test: &str) -> PathBuf {
    console_core_temporary_directories::fresh(&format!("resume-{test}")).expect("somewhere to keep them")
}

fn sessions(at: &Path) -> Sessions {
    Sessions {
        at: at.to_path_buf(),
        adjusting_for: Duration::from_secs(1),
        really: Really::Simulated,
        restoring: Restoring::StartingItAgain,
        duplicates: Duplicates::OnePerProgram,
    }
}

fn named(sessions: &Sessions) -> Vec<String> {
    let Ok(mut saved) = sessions.list();

    saved.sort();

    saved
}

#[test]
fn a_directory_no_one_has_saved_into_holds_no_sessions() {
    let at = scratch("a_directory_no_one_has_saved_into_holds_no_sessions");

    assert_eq!(named(&sessions(&at)), Vec::<String>::new());
}

#[test]
fn a_place_that_does_not_exist_at_all_is_no_sessions_rather_than_a_fault() {
    let gone = Sessions {
        at: PathBuf::from("/nowhere/no one/has/been"),
        adjusting_for: Duration::from_secs(1),
        really: Really::Simulated,
        restoring: Restoring::StartingItAgain,
        duplicates: Duplicates::OnePerProgram,
    };

    assert_eq!(gone.list(), Ok(Vec::new()));
}

#[test]
fn every_session_saved_is_one_that_can_be_named_back() {
    let at = scratch("every_session_saved_is_one_that_can_be_named_back");

    for name in ["monday", "nightly", "yesterday"] {
        create_dir_all(at.join(name)).expect("a session");
    }

    assert_eq!(named(&sessions(&at)), ["monday", "nightly", "yesterday"]);
}

#[test]
fn a_file_lying_among_the_sessions_is_not_one_of_them() {
    let at = scratch("a_file_lying_among_the_sessions_is_not_one_of_them");

    create_dir_all(at.join("monday")).expect("a session");
    std::fs::write(at.join("notes.txt"), "not a session").expect("a file");

    assert_eq!(named(&sessions(&at)), ["monday"]);
}

#[test]
fn throwing_one_away_leaves_the_others_where_they_were() {
    let at = scratch("throwing_one_away_leaves_the_others_where_they_were");

    for name in ["monday", "yesterday"] {
        create_dir_all(at.join(name)).expect("a session");
    }

    let sessions = sessions(&at);

    sessions.delete("monday").expect("thrown away");
    assert_eq!(named(&sessions), ["yesterday"]);
}

#[test]
fn a_session_with_nothing_in_it_leaves_the_screen_alone() {
    let at = scratch("a_session_with_nothing_in_it_leaves_the_screen_alone");

    let said = sessions(&at).load("never-saved");

    assert_eq!(
        said.expect("nothing to put back"),
        Restore::NothingSaved(at.join("never-saved")),
        "putting back a session no one saved is closing every window and starting nothing"
    );
}

#[test]
fn throwing_away_one_no_one_saved_says_so_rather_than_saying_nothing() {
    let at = scratch("throwing_away_one_no_one_saved_says_so_rather_than_saying_nothing");

    let said = sessions(&at).delete("never-existed");

    let why = said.expect_err("a name no one saved is a thing worth being told about");

    assert!(
        why.to_string().contains("never-existed"),
        "and the fault should say which name it was: {why}"
    );
}
