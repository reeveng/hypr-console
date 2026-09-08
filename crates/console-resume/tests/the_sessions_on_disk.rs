//! What a session is on disk, asked without a compositor.
//!
//! What a session *does* -- windows closing, programs starting, a window landing
//! on the workspace it was saved from -- cannot be answered here, and asserting
//! that the right Lua was composed would be a plumbing assertion rather than
//! coverage. That half belongs to `console-test-checks`, against a screen.
//!
//! What is answerable here is everything the program does to a directory: which
//! sessions it can see, what it does with a name nobody has saved, and that
//! throwing one away leaves the others alone. Those are the paths a person
//! reaches by typing, and each of them once ran through an `unwrap` on a
//! directory that might not be there.

use std::fs::create_dir_all;
use std::path::PathBuf;
use std::time::Duration;

use console_resume::session::{Duplicates, PutBack, Really, Restoring, Sessions};
use tempfile::TempDir;

fn sessions(at: &TempDir) -> Sessions {
    Sessions {
        at: at.path().to_path_buf(),
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
fn a_directory_nobody_has_saved_into_holds_no_sessions() {
    let at = TempDir::new().expect("somewhere to keep them");

    assert_eq!(named(&sessions(&at)), Vec::<String>::new());
}

#[test]
fn a_place_that_does_not_exist_at_all_is_no_sessions_rather_than_a_fault() {
    let gone = Sessions {
        at: PathBuf::from("/nowhere/nobody/has/been"),
        adjusting_for: Duration::from_secs(1),
        really: Really::Simulated,
        restoring: Restoring::StartingItAgain,
        duplicates: Duplicates::OnePerProgram,
    };

    assert_eq!(gone.list(), Ok(Vec::new()));
}

#[test]
fn every_session_saved_is_one_that_can_be_named_back() {
    let at = TempDir::new().expect("somewhere to keep them");

    for name in ["monday", "nightly", "yesterday"] {
        create_dir_all(at.path().join(name)).expect("a session");
    }

    assert_eq!(named(&sessions(&at)), ["monday", "nightly", "yesterday"]);
}

#[test]
fn a_file_lying_among_the_sessions_is_not_one_of_them() {
    let at = TempDir::new().expect("somewhere to keep them");

    create_dir_all(at.path().join("monday")).expect("a session");
    std::fs::write(at.path().join("notes.txt"), "not a session").expect("a file");

    assert_eq!(named(&sessions(&at)), ["monday"]);
}

#[test]
fn throwing_one_away_leaves_the_others_where_they_were() {
    let at = TempDir::new().expect("somewhere to keep them");

    for name in ["monday", "yesterday"] {
        create_dir_all(at.path().join(name)).expect("a session");
    }

    let sessions = sessions(&at);

    assert_eq!(sessions.delete("monday"), Ok(()));
    assert_eq!(named(&sessions), ["yesterday"]);
}

#[test]
fn a_session_with_nothing_in_it_leaves_the_screen_alone() {
    let at = TempDir::new().expect("somewhere to keep them");

    let said = sessions(&at).load("never-saved");

    assert_eq!(
        said,
        Ok(PutBack::NothingSaved(at.path().join("never-saved"))),
        "putting back a session nobody saved is closing every window and starting nothing"
    );
}

#[test]
fn throwing_away_one_nobody_saved_says_so_rather_than_saying_nothing() {
    let at = TempDir::new().expect("somewhere to keep them");

    let said = sessions(&at).delete("never-existed");

    assert!(said.is_err(), "a name nobody saved is a thing worth being told about");
    assert_eq!(
        said.err().map(|why| why.contains("never-existed")),
        Some(true),
        "and the fault should say which name it was"
    );
}
