//! Only one picker is ever up, and it is the last one asked for.
//!
//! The menu is on a button, on a paddle and on a key; the settings are on a
//! button and on four of the bar's icons. Every one of those roads starts a
//! process that knows nothing about the others, and two pickers at once take
//! each other's controller profile: the second to open claims it, the first to
//! close hands the desktop's buttons back while the other is still on screen.
//! Since both are drawn in the same place, backing out of one leaves you
//! looking at what appears to be the same picker refusing to close.
//!
//! Turning the second one away instead would be worse in the one case that
//! matters: the bar is reachable with a finger while a picker is up, and an
//! icon that does nothing at all reads as a broken bar. So the one on screen
//! goes. Ask through the door it came out of and nothing replaces it, which is
//! how a finger closes a panel it opened from the bar.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use console_core_never::Never;

type Failure = Box<dyn std::error::Error>;

const OTHER: &str = env!("CARGO_BIN_EXE_second-picker");

const PATIENCE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum How {
    Hold,
    Coming,
    Stuck,
    Going,
    Ask,
    Twice,
}

impl How {
    fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            How::Hold => "hold",
            How::Coming => "coming",
            How::Stuck => "stuck",
            How::Going => "going",
            How::Ask => "ask",
            How::Twice => "twice",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    Yes,
    No,
}

fn runtime(what: &str) -> Result<PathBuf, Failure> {
    let runtime = console_core_temporary_directories::fresh(&format!("lock-{what}"))?;

    Ok(runtime)
}

fn spawn_other(runtime: &Path, how: How, name: &str) -> Result<Child, Failure> {
    let Ok(word) = how.word();
    let mut child = Command::new(OTHER)
        .args([word, name])
        .env("XDG_RUNTIME_DIR", runtime)
        .stdout(Stdio::piped())
        .spawn()?;
    let voice = child.stdout.take().ok_or("the picker has no voice")?;
    let mut said = String::new();

    BufReader::new(voice).read_line(&mut said)?;

    assert_eq!(said.trim(), "held");

    Ok(child)
}

fn run_other(runtime: &Path, how: How, name: &str) -> Result<String, Failure> {
    let Ok(word) = how.word();
    let done = Command::new(OTHER)
        .args([word, name])
        .env("XDG_RUNTIME_DIR", runtime)
        .output()?;

    Ok(String::from_utf8_lossy(&done.stdout).trim().to_string())
}

fn ended(up: &mut Child) -> Result<Ended, Never> {
    let Ok(patience) = console_waiting::Schedule::of(PATIENCE);
    let ended = console_waiting::until_handed(patience, up, |up| {
        Ok(match up.try_wait() {
            Ok(Some(_ended)) => console_waiting::Ready::Yes,
            Ok(None) => console_waiting::Ready::NotYet,
            Err(_unasked) => console_waiting::Ready::NotYet,
        })
    });

    Ok(match ended {
        Ok(console_waiting::Outcome::Happened) => Ended::Yes,
        Ok(console_waiting::Outcome::RanOut) => {
            let _ = up.kill();

            Ended::No
        }
    })
}

#[test]
fn the_door_that_opened_it_closes_it() -> Result<(), Failure> {
    let runtime = runtime("same-door")?;
    let mut up = spawn_other(&runtime, How::Hold, "settings Sound")?;
    let answer = run_other(&runtime, How::Ask, "settings Sound")?;

    assert_eq!(answer, "no");
    assert_eq!(ended(&mut up), Ok(Ended::Yes), "the panel that was up is still up");

    Ok(())
}

#[test]
fn a_door_that_names_no_tab_is_still_the_door_it_came_out_of() -> Result<(), Failure> {
    let runtime = runtime("no-tab")?;
    let mut up = spawn_other(&runtime, How::Hold, "notifications ")?;
    let answer = run_other(&runtime, How::Ask, "notifications ")?;

    assert_eq!(answer, "no");
    assert_eq!(ended(&mut up), Ok(Ended::Yes), "the bell opened again the panel it had just put away");

    Ok(())
}

#[test]
fn another_door_takes_its_place() -> Result<(), Failure> {
    let runtime = runtime("other-door")?;
    let mut up = spawn_other(&runtime, How::Hold, "settings Sound")?;
    let answer = run_other(&runtime, How::Ask, "settings Battery")?;

    assert_eq!(answer, "yes");
    assert_eq!(ended(&mut up), Ok(Ended::Yes), "two panels are up at once");

    Ok(())
}

#[test]
fn the_screen_is_taken_before_it_is_drawn_on() -> Result<(), Failure> {
    let runtime = runtime("in-order")?;
    let mut up = spawn_other(&runtime, How::Hold, "menu")?;
    let answer = run_other(&runtime, How::Ask, "settings ")?;

    assert_eq!(answer, "yes");
    assert!(
        up.try_wait().is_ok_and(|ended| ended.is_some()),
        "it drew before the last one had gone"
    );

    Ok(())
}

#[test]
fn the_one_that_holds_it_may_ask_twice() -> Result<(), Failure> {
    let runtime = runtime("twice")?;
    let answer = run_other(&runtime, How::Twice, "menu")?;

    assert_eq!(answer, "yes yes");

    Ok(())
}

#[test]
fn a_picker_that_dies_does_not_keep_the_lock() -> Result<(), Failure> {
    let runtime = runtime("died")?;
    let first = run_other(&runtime, How::Ask, "menu")?;
    let second = run_other(&runtime, How::Ask, "menu")?;

    assert_eq!(first, "yes");
    assert_eq!(second, "yes");

    Ok(())
}

#[test]
fn a_picker_on_its_way_is_left_to_come() -> Result<(), Failure> {
    let runtime = runtime("coming")?;
    let mut coming = spawn_other(&runtime, How::Coming, "menu")?;
    let answer = run_other(&runtime, How::Ask, "menu")?;

    assert_eq!(answer, "no");
    assert!(
        coming.try_wait().is_ok_and(|ended| ended.is_none()),
        "the menu that was coming was cancelled by the press that waited for it"
    );

    let _ = coming.kill();

    Ok(())
}

#[test]
fn a_picker_whose_window_has_gone_hands_the_screen_over() -> Result<(), Failure> {
    let runtime = runtime("going")?;
    let mut last = spawn_other(&runtime, How::Going, "menu")?;
    let answer = run_other(&runtime, How::Ask, "menu")?;

    assert_eq!(answer, "yes");

    let _ = last.kill();
    let _ = last.wait();

    Ok(())
}

#[test]
fn a_picker_that_never_draws_is_taken_over() -> Result<(), Failure> {
    let runtime = runtime("stuck")?;
    let mut never = spawn_other(&runtime, How::Stuck, "menu")?;
    let answer = run_other(&runtime, How::Ask, "menu")?;

    assert_eq!(answer, "yes");

    let _ = never.kill();
    let _ = never.wait();

    Ok(())
}
