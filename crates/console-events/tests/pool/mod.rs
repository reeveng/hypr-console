//! The pool stood up on a socket in a temporary directory, with a source a
//! test speaks through.
//!
//! `serve` is handed its sources as a plain `fn`, because that is what the
//! pool holds on to for as long as it runs, so the one way a source can reach
//! the test that wants to say something through it is a value the whole test
//! binary shares. `SAYING` is that value, and it is filled once, before the
//! pool is up, by `serve_at`. Every test binary compiles this module whole and
//! each one uses a different part of it.

#![allow(
    dead_code,
    reason = "every test binary compiles this whole module and each one asks it for a different part"
)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_core_never::Never;
use console_events::serving;
use console_events::sources::Subscribed;
use console_events::subscription::Received;
use console_program_contract::{Change, Topic};
use console_program_lifetime::threads;
use console_waiting::{Outcome, Ready, Schedule};

pub type Failure = Box<dyn std::error::Error>;

pub const BEFORE_LONG: Duration = Duration::from_secs(5);

const EVERY_SO_OFTEN: Duration = Duration::from_millis(5);

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "`serve` holds its sources as a plain `fn`, which cannot carry anything the test hands it, so the channel a source is told through has to be where the whole test binary can reach it; it is set once, before the pool is asked for anything"
    )
)]
static SAYING: OnceLock<Sender<Sender<Change>>> = OnceLock::new();

fn source(topic: &Topic, say: Sender<Change>) -> Result<Subscribed, Never> {
    console_events::sources::handed_to(SAYING.get(), &Topic::Sound, topic, say)
}

pub fn socket(named: &str) -> Result<PathBuf, Failure> {
    let folder = console_core_temporary_directories::fresh(&format!("events-{named}"))?;

    Ok(folder.join(format!("events-{named}.sock")))
}

pub fn serve_at(at: &Path) -> Result<Receiver<Sender<Change>>, Failure> {
    let (handing, handed) = channel();

    SAYING.set(handing).map_err(|_| "the pool was served twice in one test binary")?;

    let serving = at.to_path_buf();
    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let _the_test_reads_whether_it_came_up = serving::serve(&serving, source);
    }));

    up(at)?;

    Ok(handed)
}

pub fn up(at: &Path) -> Result<(), Failure> {
    let Ok(patience) = Schedule::asking_every(BEFORE_LONG, EVERY_SO_OFTEN);
    let Ok(came_up) = console_waiting::until(patience, || {
        Ok(match at.exists() {
            true => Ready::Yes,
            false => Ready::NotYet,
        })
    });

    match came_up {
        Outcome::Happened => Ok(()),
        Outcome::RanOut => Err(Failure::from(format!("nothing came up at {}", at.display()))),
    }
}

pub fn change(text: &str) -> Result<Change, Never> {
    Ok(Change { topic: Topic::Sound, text: text.to_string() })
}

pub fn before_long(heard: &Receiver<Received>) -> Result<Option<Change>, Never> {
    let heard = std::iter::repeat_with(|| heard.recv_timeout(BEFORE_LONG)).find_map(|received| match received {
        Ok(Received::Event(change)) => Some(Some(change)),
        Ok(Received::Connected) => None,
        Err(_nothing_before_long) => Some(None),
    });

    Ok(heard.flatten())
}
