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

use std::error::Error;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_never::Never;
use console_resume::session::{Duplicates, Restore, Really, Restoring, Sessions};

fn scratch(test: &str) -> Result<PathBuf, Box<dyn Error>> {
    let at = console_core_temporary_directories::fresh(&format!("resume-{test}"))?;

    Ok(at)
}

fn sessions(at: &Path) -> Result<Sessions, Never> {
    Ok(Sessions {
        at: at.to_path_buf(),
        adjusting_for: Duration::from_secs(1),
        really: Really::Simulated,
        restoring: Restoring::StartingItAgain,
        duplicates: Duplicates::OnePerProgram,
    })
}

fn named(sessions: &Sessions) -> Result<Vec<String>, Never> {
    let Ok(mut saved) = sessions.list();

    saved.sort();

    Ok(saved)
}

#[test]
fn a_directory_no_one_has_saved_into_holds_no_sessions() -> Result<(), Box<dyn Error>> {
    let at = scratch("a_directory_no_one_has_saved_into_holds_no_sessions")?;
    let Ok(sessions) = sessions(&at);

    assert_eq!(named(&sessions), Ok(Vec::new()));

    Ok(())
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
fn every_session_saved_is_one_that_can_be_named_back() -> Result<(), Box<dyn Error>> {
    let at = scratch("every_session_saved_is_one_that_can_be_named_back")?;

    for name in ["monday", "nightly", "yesterday"] {
        create_dir_all(at.join(name))?;
    }

    let Ok(sessions) = sessions(&at);
    let Ok(named) = named(&sessions);

    assert_eq!(named, ["monday", "nightly", "yesterday"]);

    Ok(())
}

#[test]
fn a_file_lying_among_the_sessions_is_not_one_of_them() -> Result<(), Box<dyn Error>> {
    let at = scratch("a_file_lying_among_the_sessions_is_not_one_of_them")?;

    create_dir_all(at.join("monday"))?;
    console_core_atomic_writes::whole(&at.join("notes.txt"), b"not a session")?;

    let Ok(sessions) = sessions(&at);
    let Ok(named) = named(&sessions);

    assert_eq!(named, ["monday"]);

    Ok(())
}

#[test]
fn throwing_one_away_leaves_the_others_where_they_were() -> Result<(), Box<dyn Error>> {
    let at = scratch("throwing_one_away_leaves_the_others_where_they_were")?;

    for name in ["monday", "yesterday"] {
        create_dir_all(at.join(name))?;
    }

    let Ok(sessions) = sessions(&at);

    sessions.delete("monday")?;

    let Ok(named) = named(&sessions);

    assert_eq!(named, ["yesterday"]);

    Ok(())
}

#[test]
fn a_session_with_nothing_in_it_leaves_the_screen_alone() -> Result<(), Box<dyn Error>> {
    let at = scratch("a_session_with_nothing_in_it_leaves_the_screen_alone")?;
    let Ok(sessions) = sessions(&at);
    let said = sessions.load("never-saved")?;

    assert_eq!(
        said,
        Restore::NothingSaved(at.join("never-saved")),
        "putting back a session no one saved is closing every window and starting nothing"
    );

    Ok(())
}

#[test]
fn throwing_away_one_no_one_saved_says_so_rather_than_saying_nothing() -> Result<(), Box<dyn Error>> {
    let at = scratch("throwing_away_one_no_one_saved_says_so_rather_than_saying_nothing")?;
    let Ok(sessions) = sessions(&at);

    let why = match sessions.delete("never-existed") {
        Err(why) => why.to_string(),
        Ok(()) => "a name no one saved was thrown away without a word".to_string(),
    };

    assert!(why.contains("never-existed"), "and the fault should say which name it was: {why}");

    Ok(())
}
