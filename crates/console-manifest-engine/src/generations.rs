//! Which apply this machine is running, and whether it reached the end.
//!
//! An apply rewrites packages, every program in `[build]`, the files and the
//! units, and until now it had no identity: one that stopped in the middle left
//! a machine that is neither what it was nor what it was asked to be, and
//! nothing on the device could say which. `snapshot` is the half that keeps a
//! previous; this is the half that names what is running. An apply becomes a
//! generation, numbered, recorded with the commit it came from, and an apply
//! that died partway is a generation that never finished rather than a machine
//! in an unnamed state.
//!
//! This is what the immutable systems buy with a read-only root, and it is the
//! part of it worth having: not that the machine cannot be written to, but that
//! what it is can be said out loud and put back. `desktop.conf` and
//! `migrations/` already describe the machine; what was missing was the
//! numbering.
//!
//! One file per generation, named for its number, holding the commit and a
//! word for how far it got -- the same shape and the same argument as
//! `console_manifest_migrations::done`, and under `/var/lib` for the same
//! reason: what an apply writes is `/etc` and `/usr/local/bin`, and the machine
//! is what has run it, not a person. It is one file rather than a list because
//! a list is a thing to rewrite, and the only rewrite here is the one that ends
//! a generation, which goes through the crate that writes a file whole or not
//! at all.
//!
//! A word this cannot read is read as an apply that did not finish. The two
//! wrong answers are not the same size: a generation wrongly called finished is
//! a machine nobody looks at again, and one wrongly called unfinished costs an
//! apply somebody was going to run anyway.
//!
//! What is not here: counting a boot that never came up, and going back when
//! the count runs out. Both need a machine that has been started and stopped
//! rather than arithmetic, and the boot entry they would choose is not
//! reachable from the pad yet.

use std::path::Path;

use console_core_never::Never;

use crate::unapplied::Unapplied;

pub const KEPT: &str = "/var/lib/console/generations";

const FINISHED: &str = "finished";
const STARTED: &str = "started";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Started,
    Finished,
}

impl State {
    fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            State::Started => STARTED,
            State::Finished => FINISHED,
        })
    }

    fn read(said: &str) -> Result<State, Never> {
        Ok(match said.trim() == FINISHED {
            true => State::Finished,
            false => State::Started,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commit<'a>(pub &'a str);

const UNSAID: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub number: u32,
    pub commit: String,
    pub state: State,
}

impl Generation {
    pub fn said(&self) -> Result<String, Never> {
        let Ok(word) = self.state.word();

        Ok(format!("{} {word}\n", self.commit))
    }

    pub fn named(&self) -> Result<String, Never> {
        Ok(format!("{} ({})", self.number, self.commit))
    }

    pub fn finished(&self) -> Result<Generation, Never> {
        Ok(Generation {
            number: self.number,
            commit: self.commit.clone(),
            state: State::Finished,
        })
    }
}

pub fn next(kept: &[Generation], commit: Commit<'_>) -> Result<Generation, Never> {
    let highest = match kept.iter().map(|one| one.number).max() {
        Some(highest) => highest,
        None => 0,
    };

    let named = match commit.0.trim().is_empty() {
        true => UNSAID,
        false => commit.0.trim(),
    };

    Ok(Generation {
        number: highest.saturating_add(1),
        commit: named.to_string(),
        state: State::Started,
    })
}

pub fn unfinished(kept: &[Generation]) -> Result<Option<&Generation>, Never> {
    let newest = match kept.iter().max_by_key(|one| one.number) {
        Some(newest) => newest,
        None => return Ok(None),
    };

    Ok(match newest.state {
        State::Started => Some(newest),
        State::Finished => None,
    })
}

fn one(number: u32, held: &str) -> Result<Generation, Never> {
    let said = match held.lines().next() {
        Some(said) => said,
        None => "",
    };

    let (commit, word) = match said.split_once(' ') {
        Some((commit, word)) => (commit, word),
        None => (said, STARTED),
    };

    let Ok(state) = State::read(word);

    Ok(Generation { number, commit: commit.trim().to_string(), state })
}

pub fn read(at: &Path) -> Result<Vec<Generation>, Unapplied> {
    let entries = match std::fs::read_dir(at) {
        Ok(entries) => entries,
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => return Ok(Vec::new()),
            false => return Err(Unapplied::Read(at.to_path_buf(), fault)),
        },
    };

    let mut kept: Vec<Generation> = Vec::new();

    for entry in entries.flatten() {
        let number = match entry.file_name().to_string_lossy().parse::<u32>() {
            Ok(number) => number,
            Err(_) => continue,
        };
        let Ok(held) = console_core_atomic_writes::read(&entry.path());

        let said = match held {
            console_core_atomic_writes::Stored::Text(said) => said,
            console_core_atomic_writes::Stored::Absent => continue,
            console_core_atomic_writes::Stored::Failed(fault) => {
                return Err(Unapplied::Unsaid(entry.path(), fault));
            }
        };

        let Ok(generation) = one(number, &said);

        kept.push(generation);
    }

    kept.sort_by_key(|one| one.number);

    Ok(kept)
}

pub fn remember(at: &Path, generation: &Generation) -> Result<(), Unapplied> {
    std::fs::create_dir_all(at).map_err(|fault| Unapplied::Making(at.to_path_buf(), fault))?;

    let Ok(said) = generation.said();

    console_core_atomic_writes::whole(&at.join(generation.number.to_string()), said.as_bytes())
        .map_err(Unapplied::Wrote)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(number: u32, commit: &str) -> Generation {
        Generation { number, commit: commit.to_string(), state: State::Started }
    }

    #[test]
    fn a_machine_that_has_never_applied_gets_the_first_generation() {
        let Ok(next) = next(&[], Commit("a1b2c3d"));

        assert_eq!(next, started(1, "a1b2c3d"));
    }

    #[test]
    fn the_next_generation_is_one_past_the_highest_and_not_one_past_the_last_read() {
        let kept = vec![started(7, "a1b2c3d"), started(2, "0ff0ff0")];
        let Ok(next) = next(&kept, Commit("beefbee"));

        assert_eq!(next.number, 8);
    }

    #[test]
    fn a_tree_that_would_not_say_which_commit_is_still_a_generation() {
        let Ok(next) = next(&[], Commit("  "));

        assert_eq!(next.commit, UNSAID);
    }

    #[test]
    fn an_apply_that_reached_the_end_leaves_nothing_unfinished() {
        let Ok(done) = started(3, "a1b2c3d").finished();
        let kept = [done];
        let Ok(unfinished) = unfinished(&kept);

        assert_eq!(unfinished, None);
    }

    #[test]
    fn an_apply_that_stopped_partway_is_the_generation_that_never_finished() {
        let kept = [started(3, "a1b2c3d")];
        let Ok(unfinished) = unfinished(&kept);

        assert_eq!(unfinished, Some(&started(3, "a1b2c3d")));
    }

    #[test]
    fn an_older_generation_that_never_finished_is_not_what_is_running() {
        let Ok(done) = started(4, "beefbee").finished();
        let kept = vec![started(3, "a1b2c3d"), done];
        let Ok(unfinished) = unfinished(&kept);

        assert_eq!(unfinished, None);
    }

    #[test]
    fn what_was_written_is_what_is_read_back() {
        let Ok(done) = started(12, "a1b2c3d").finished();
        let Ok(said) = done.said();
        let Ok(read) = one(12, &said);

        assert_eq!(read, done);
    }

    #[test]
    fn a_word_nothing_here_wrote_is_an_apply_that_did_not_finish() {
        let Ok(read) = one(12, "a1b2c3d confirmed\n");

        assert_eq!(read, started(12, "a1b2c3d"));
    }
}
