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
//! ended -- and `Outcome` is that answer said out loud, so a caller that runs
//! out of patience cannot mistake it for one that got what it came for.
//!
//! **The loop is handed its clock.** `until_some_handed_on` is the loop, and
//! it takes the `Clock` it waits on; every other spelling hands it the
//! machine's. A test hands it a `TestClock` instead, and a patience of twenty
//! seconds is then asked every time it would have been, in no time at all.

pub mod clock;
pub mod latch;

pub use console_core_schedules::Schedule;

use clock::{Clock, LiveClock};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_schedules::{Decision, Elapsed};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ready {
    Yes,
    NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Happened,
    RanOut,
}

impl Ready {
    pub fn flipped(self) -> Result<Ready, Never> {
        Ok(match self {
            Ready::Yes => Ready::NotYet,
            Ready::NotYet => Ready::Yes,
        })
    }
}

pub fn until(
    patience: Schedule,
    mut ask: impl FnMut() -> Result<Ready, Never>,
) -> Result<Outcome, Never> {
    until_handed(patience, &mut ask, |ask| ask())
}

pub fn until_some<T>(
    patience: Schedule,
    mut look: impl FnMut() -> Result<Option<T>, Never>,
) -> Result<Option<T>, Never> {
    until_some_handed(patience, &mut look, |look| look())
}

pub fn until_handed<M>(
    patience: Schedule,
    handed: &mut M,
    ask: impl Fn(&mut M) -> Result<Ready, Never>,
) -> Result<Outcome, Never> {
    let Ok(found) = until_some_handed(patience, handed, |handed| {
        let Ok(seen) = ask(handed);

        Ok(match seen {
            Ready::Yes => Some(()),
            Ready::NotYet => None,
        })
    });

    Ok(match found {
        Some(()) => Outcome::Happened,
        None => Outcome::RanOut,
    })
}

pub fn until_some_handed<M, T>(
    patience: Schedule,
    handed: &mut M,
    look: impl Fn(&mut M) -> Result<Option<T>, Never>,
) -> Result<Option<T>, Never> {
    let Ok(mut clock) = LiveClock::started();

    until_some_handed_on(&mut clock, patience, handed, look)
}

pub fn until_some_handed_on<C: Clock, M, T>(
    clock: &mut C,
    patience: Schedule,
    handed: &mut M,
    look: impl Fn(&mut M) -> Result<Option<T>, Never>,
) -> Result<Option<T>, Never> {
    let Ok(began) = clock.elapsed();
    let Ok(driver) = patience.driver();
    let ended = console_core_iteration::iterate((clock, handed, driver, began), |(clock, handed, mut driver, this_try)| {
        let Ok(found) = look(handed);

        Ok(match found {
            Some(found) => Step::Halt(Some(found)),
            None => {
                let Ok(now) = clock.elapsed();
                let Ok(decided) =
                    driver.next(Elapsed { total: now.saturating_sub(began), this_try: now.saturating_sub(this_try) });

                match decided {
                    Decision::Finished => Step::Halt(None),
                    Decision::Continue(gap) => {
                        let Ok(()) = clock.pause(gap);
                        let Ok(next_try) = clock.elapsed();

                        Step::Again((clock, handed, driver, next_try))
                    }
                }
            }
        })
    });

    Ok(match ended {
        Ok(found) => found,
        Err(_endless) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clock::TestClock;
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    #[test]
    fn a_thing_that_is_already_there_is_not_waited_for() {
        let began = Instant::now();
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(30), Duration::from_secs(30));
        let Ok(waited) = until(patience, || Ok(Ready::Yes));

        assert_eq!(waited, Outcome::Happened);
        assert!(began.elapsed() < Duration::from_secs(1), "it slept before it asked");
    }

    #[test]
    fn a_thing_that_never_comes_runs_out_and_says_so() {
        let Ok(patience) = Schedule::asking_every(Duration::from_millis(30), Duration::from_millis(5));
        let Ok(waited) = until(patience, || Ok(Ready::NotYet));

        assert_eq!(waited, Outcome::RanOut);
    }

    #[test]
    fn a_patience_of_twenty_seconds_is_asked_every_time_it_would_have_been() {
        let mut clock = TestClock::default();
        let mut asks = 0_u32;
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(20), Duration::from_millis(50));
        let Ok(found) = until_some_handed_on(
            &mut clock,
            patience,
            &mut asks,
            |asks| {
                *asks = (*asks).saturating_add(1);

                Ok(None::<()>)
            },
        );

        assert_eq!(found, None);
        assert_eq!(asks, 401, "a look at the start and one after every gap, the last of them at twenty seconds");
        assert_eq!(clock.paused.len(), 400);
        assert_eq!(clock.elapsed, Duration::from_secs(20));
    }

    #[test]
    fn a_thing_that_arrives_part_way_through_is_met() {
        let mut clock = TestClock::default();
        let mut asks = 0_u32;
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(10), Duration::from_secs(1));
        let Ok(found) = until_some_handed_on(
            &mut clock,
            patience,
            &mut asks,
            |asks| {
                *asks = (*asks).saturating_add(1);

                Ok(match *asks >= 3 {
                    true => Some(()),
                    false => None,
                })
            },
        );

        assert_eq!(found, Some(()));
        assert_eq!(asks, 3);
        assert_eq!(clock.paused, vec![Duration::from_secs(1), Duration::from_secs(1)], "it stopped waiting when it was met");
    }

    #[test]
    fn the_patience_that_runs_out_still_asked_once() {
        let mut clock = TestClock::default();
        let mut asks = 0_u32;
        let Ok(patience) = Schedule::asking_every(Duration::from_millis(0), Duration::from_secs(30));
        let Ok(found) = until_some_handed_on(
            &mut clock,
            patience,
            &mut asks,
            |asks| {
                *asks = (*asks).saturating_add(1);

                Ok(None::<()>)
            },
        );

        assert_eq!(found, None);
        assert_eq!(asks, 1, "a patience of nothing at all still gets one look");
        assert_eq!(clock.paused, Vec::<Duration>::new(), "and no gap after it");
    }

    #[test]
    fn a_wait_is_measured_from_when_it_began_on_a_clock_that_was_already_running() {
        let mut clock = TestClock::default();
        let Ok(()) = clock.adjust(Duration::from_secs(600));
        let mut asks = 0_u32;
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(1), Duration::from_millis(500));
        let Ok(_found) = until_some_handed_on(
            &mut clock,
            patience,
            &mut asks,
            |asks| {
                *asks = (*asks).saturating_add(1);

                Ok(None::<()>)
            },
        );

        assert_eq!(asks, 3, "ten minutes on the clock before the wait is not ten minutes of patience spent");
    }

    #[test]
    fn what_was_looked_for_comes_back_with_it() {
        let asks = Cell::new(0_u32);
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(10), Duration::from_millis(1));
        let Ok(found) = until_some(
            patience,
            || {
                asks.set(asks.get().saturating_add(1));

                Ok(match asks.get() >= 2 {
                    true => Some("here"),
                    false => None,
                })
            },
        );

        assert_eq!(found, Some("here"));
    }

    #[test]
    fn nothing_found_is_nothing_rather_than_a_wait_that_looked_like_one() {
        let mut clock = TestClock::default();
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(20), Duration::from_secs(5));
        let Ok(found) = until_some_handed_on(
            &mut clock,
            patience,
            &mut (),
            |()| Ok(None::<u8>),
        );

        assert_eq!(found, None);
    }

    #[test]
    fn what_a_wait_was_handed_is_written_by_the_question_and_kept() {
        let mut asks = 0_u32;
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(10), Duration::from_millis(1));
        let Ok(waited) = until_handed(
            patience,
            &mut asks,
            |asks| {
                *asks = (*asks).saturating_add(1);

                Ok(match *asks >= 3 {
                    true => Ready::Yes,
                    false => Ready::NotYet,
                })
            },
        );

        assert_eq!(waited, Outcome::Happened);
        assert_eq!(asks, 3, "the question was handed the same count every time");
    }

    #[test]
    fn a_look_brings_back_what_it_found_and_what_it_was_handed() {
        let mut clock = TestClock::default();
        let mut said = String::new();
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(20), Duration::from_secs(5));
        let Ok(found) = until_some_handed_on(
            &mut clock,
            patience,
            &mut said,
            |said| {
                said.push('.');

                Ok(match said.len() >= 2 {
                    true => Some(said.clone()),
                    false => None,
                })
            },
        );

        assert_eq!(found.as_deref(), Some(".."));
        assert_eq!(said, "..");
    }

    #[test]
    fn a_thing_being_gone_is_the_other_side_of_it_being_there() {
        let Ok(gone) = Ready::Yes.flipped();
        let Ok(there) = Ready::NotYet.flipped();

        assert_eq!(gone, Ready::NotYet);
        assert_eq!(there, Ready::Yes);
    }
}
