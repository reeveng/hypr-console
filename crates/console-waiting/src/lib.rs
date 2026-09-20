//! Waiting for a thing rather than for a number of seconds.
//!
//! EXPLICIT021 forbids `thread::sleep`, and the objection it makes is not
//! that time may never pass: it is that a duration is a poor name for what is
//! being waited for. A window is drawn, a child has exited, a lock is free, a
//! charger is plugged in -- each of those can be asked, and asking is right on
//! every machine, where a number is right only on the one it was measured on.
//! A sleep long enough for the slowest machine is a check that wastes the same
//! seconds on every other one, and one short enough to be quick is a check
//! that goes red for a reason that is not the feature.
//!
//! So the loop that asks is written once, here, and everything that waits
//! calls it. That is the same argument `console-core-reconnect` makes about
//! backoff: how long a handheld waits before it wakes to ask again is one
//! decision, and twenty copies of it is twenty answers waiting to disagree.
//!
//! **This crate holds the one sleep the rule allows for this reason.** Between
//! two asks about something nothing will announce, a thread has to stop, and
//! the only alternatives are a spin that heats a handheld in your hands and a
//! subscription the far end does not offer. Every other sleep left in the tree
//! is one where the elapsing is itself the thing being asked for -- a button
//! held down long enough for a compositor to see it, a note shown for a
//! moment, a backoff between two attempts -- and each of those says so at its
//! own site.
//!
//! **Each of the two has a second spelling that is handed what it asks with.**
//! A question about a child, or a lock file, or anything else reached through
//! a `&mut`, cannot be asked by a closure that was given nothing: what it
//! needs can only arrive by capture, and a closure that captured it holds the
//! permission to write it for as long as it lives, which is EXPLICIT047's
//! whole complaint. So `until_handed` and `found_handed` take the thing and
//! hand it to the question at every ask, which is EXPLICIT044's sentence about
//! a function said about a closure, and what `Device::until` has done with the
//! stage since it was written. The loop is written once, in `found_handed`,
//! and the other three are that loop with the answer said differently.
//!
//! What it does not do is decide the patience. How long is worth waiting for
//! is a question about the thing, and the caller is the only one who knows it:
//! a lock is milliseconds and a nested compositor is twenty seconds. What is
//! shared is the shape -- ask, and ask again, and say which of the two ways it
//! ended -- and `Waited` is that answer said out loud, so a caller that runs
//! out of patience cannot mistake it for one that got what it came for.

pub mod woken;

use console_core_never::Never;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Yes,
    NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    Happened,
    RanOut,
}

impl Seen {
    pub fn flipped(self) -> Result<Seen, Never> {
        Ok(match self {
            Seen::Yes => Seen::NotYet,
            Seen::NotYet => Seen::Yes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patience {
    pub until: Duration,
    pub between: Duration,
}

pub const BETWEEN: Duration = Duration::from_millis(50);

impl Patience {
    pub fn of(until: Duration) -> Result<Patience, Never> {
        Ok(Patience { until, between: BETWEEN })
    }

    pub fn asking_every(until: Duration, between: Duration) -> Result<Patience, Never> {
        Ok(Patience { until, between })
    }
}

pub fn until(
    patience: Patience,
    mut ask: impl FnMut() -> Result<Seen, Never>,
) -> Result<Waited, Never> {
    until_handed(patience, &mut ask, |ask| ask())
}

pub fn found<T>(
    patience: Patience,
    mut look: impl FnMut() -> Result<Option<T>, Never>,
) -> Result<Option<T>, Never> {
    found_handed(patience, &mut look, |look| look())
}

pub fn until_handed<M>(
    patience: Patience,
    handed: &mut M,
    ask: impl Fn(&mut M) -> Result<Seen, Never>,
) -> Result<Waited, Never> {
    let Ok(found) = found_handed(patience, handed, |handed| {
        let Ok(seen) = ask(handed);

        Ok(match seen {
            Seen::Yes => Some(()),
            Seen::NotYet => None,
        })
    });

    Ok(match found {
        Some(()) => Waited::Happened,
        None => Waited::RanOut,
    })
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the elapsing is what was asked for here: this crate is the one that waits, and a patience with no clock under it is a loop that never ends"
    )
)]
pub fn found_handed<M, T>(
    patience: Patience,
    handed: &mut M,
    look: impl Fn(&mut M) -> Result<Option<T>, Never>,
) -> Result<Option<T>, Never> {
    let by = Instant::now() + patience.until;

    loop {
        let Ok(found) = look(handed);

        match found {
            Some(found) => return Ok(Some(found)),
            None => {},
        }

        match Instant::now() >= by {
            true => return Ok(None),
            false => {},
        }

        let Ok(()) = between(patience.between);
    }
}

fn between(gap: Duration) -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "this is the gap between two asks about a thing nothing will announce, which is what EXPLICIT021 asks a wait to be built out of; a spin instead of it would heat a handheld somebody is holding"
        )
    )]
    std::thread::sleep(gap);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thing_that_is_already_there_is_not_waited_for() {
        let began = Instant::now();
        let Ok(waited) = until(
            Patience { until: Duration::from_secs(30), between: Duration::from_secs(30) },
            || Ok(Seen::Yes),
        );

        assert_eq!(waited, Waited::Happened);
        assert!(began.elapsed() < Duration::from_secs(1), "it slept before it asked");
    }

    #[test]
    fn a_thing_that_never_comes_runs_out_and_says_so() {
        let Ok(waited) = until(
            Patience { until: Duration::from_millis(30), between: Duration::from_millis(5) },
            || Ok(Seen::NotYet),
        );

        assert_eq!(waited, Waited::RanOut);
    }

    #[test]
    fn a_thing_that_arrives_part_way_through_is_met() {
        let mut asks = 0;
        let Ok(waited) = until(
            Patience { until: Duration::from_secs(10), between: Duration::from_millis(1) },
            || {
                asks += 1;

                Ok(match asks >= 3 {
                    true => Seen::Yes,
                    false => Seen::NotYet,
                })
            },
        );

        assert_eq!(waited, Waited::Happened);
        assert_eq!(asks, 3);
    }

    #[test]
    fn the_patience_that_runs_out_still_asked_once() {
        let mut asks = 0;
        let Ok(waited) = until(
            Patience { until: Duration::from_millis(0), between: Duration::from_secs(30) },
            || {
                asks += 1;

                Ok(Seen::NotYet)
            },
        );

        assert_eq!(waited, Waited::RanOut);
        assert_eq!(asks, 1, "a patience of nothing at all still gets one look");
    }

    #[test]
    fn what_was_looked_for_comes_back_with_it() {
        let mut asks = 0;
        let Ok(found) = found(
            Patience { until: Duration::from_secs(10), between: Duration::from_millis(1) },
            || {
                asks += 1;

                Ok(match asks >= 2 {
                    true => Some("here"),
                    false => None,
                })
            },
        );

        assert_eq!(found, Some("here"));
    }

    #[test]
    fn nothing_found_is_nothing_rather_than_a_wait_that_looked_like_one() {
        let Ok(found) = found(
            Patience { until: Duration::from_millis(20), between: Duration::from_millis(5) },
            || Ok(None::<u8>),
        );

        assert_eq!(found, None);
    }

    #[test]
    fn a_patience_says_how_often_as_well_as_how_long() {
        let Ok(plain) = Patience::of(Duration::from_secs(3));
        let Ok(often) = Patience::asking_every(Duration::from_secs(3), Duration::from_millis(5));

        assert_eq!(plain.between, BETWEEN);
        assert_eq!(often.between, Duration::from_millis(5));
        assert_eq!(plain.until, often.until);
    }

    #[test]
    fn what_a_wait_was_handed_is_written_by_the_question_and_kept() {
        let mut asks = 0;
        let Ok(waited) = until_handed(
            Patience { until: Duration::from_secs(10), between: Duration::from_millis(1) },
            &mut asks,
            |asks| {
                *asks += 1;

                Ok(match *asks >= 3 {
                    true => Seen::Yes,
                    false => Seen::NotYet,
                })
            },
        );

        assert_eq!(waited, Waited::Happened);
        assert_eq!(asks, 3, "the question was handed the same count every time");
    }

    #[test]
    fn a_look_brings_back_what_it_found_and_what_it_was_handed() {
        let mut said = String::new();
        let Ok(found) = found_handed(
            Patience { until: Duration::from_millis(20), between: Duration::from_millis(5) },
            &mut said,
            |said| {
                said.push('.');

                Ok(match said.len() >= 2 {
                    true => Some(said.len()),
                    false => None,
                })
            },
        );

        assert_eq!(found, Some(2));
        assert_eq!(said, "..");
    }

    #[test]
    fn a_thing_being_gone_is_the_other_side_of_it_being_there() {
        let Ok(gone) = Seen::Yes.flipped();
        let Ok(there) = Seen::NotYet.flipped();

        assert_eq!(gone, Seen::NotYet);
        assert_eq!(there, Seen::Yes);
    }
}
