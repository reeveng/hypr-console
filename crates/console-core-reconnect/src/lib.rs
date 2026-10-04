//! A subscription made again, for as long as someone still wants one.
//!
//! Every watcher on this desktop is a subscription to something else: the
//! compositor's socket, a `pactl subscribe`, a `nmcli monitor`. Each of them
//! was made once, when the program started, and each of them ended the moment
//! the far end went away. What that looks like is not a program that stopped:
//! it is an icon that is still drawn, still right about what it said last, and
//! never right again. A bar full of those reads as a bar that works until you
//! watch it.
//!
//! So a subscription is made again for as long as the program wants one. This
//! is the trying and the waiting between the tries, and nothing else: what is
//! being reached for is the caller's business, and four of them reach for four
//! different things. It is one crate rather than a loop written out in each,
//! because how long a handheld waits before it wakes to ask again is one
//! decision, and four copies of it is four answers waiting to disagree.
//!
//! The waiting has two shapes because a caller has two. [`keep`] is the whole
//! of it for a program with a thread to spare: hand it a round and it is made
//! again forever. A program already inside a loop of someone else's -- a
//! panel's, which is glib's -- cannot be handed a thread and has to do its own
//! awaiting, so [`schedule`] is the same decision without the thread: its
//! driver says how long before the next try, given how long the last one
//! stood. `keep` steps that same driver, which is what stops the two from
//! drifting.
//!
//! The decision itself is a `console_core_schedules::Schedule`: a second,
//! twice as long after every try that falls straight over, never longer than a
//! minute, and a second again once a try has stood. It used to be written out
//! here as two functions and a struct that said it twice; it is now said once,
//! in the words every other wait in the tree is said in.

use console_core_never::Never;
use console_core_schedules::{Decision, Elapsed, Schedule};
use std::time::{Duration, Instant};

pub const FIRST: Duration = Duration::from_secs(1);

pub const LONGEST: Duration = Duration::from_secs(60);

pub const STOOD: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    Another,
    Finished,
}

pub fn schedule() -> Result<Schedule, Never> {
    let Ok(growing) = Schedule::exponential(FIRST, LONGEST);

    growing.reset_after(STOOD)
}

pub fn keep(once: impl FnMut() -> Round + Send + 'static) -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit035_no_loose_thread",
        allow(
            explicit035_no_loose_thread,
            reason = "this crate is what a subscription made again is, and the thread is the making: it runs until the caller's own round says Halt or the program ends, which is what `keep` is asked for. There is nothing above it to hold a handle, and the word this rule asks for is the name of the function"
        )
    )]
    let _ = std::thread::spawn(move || {
        let Ok(schedule) = schedule();
        let Ok(mut driver) = schedule.driver();
        let Ok(started) = now();
        let Ok(mut began) = now();
        let rounds = std::iter::repeat_with(once).map_while(|round| match round {
            Round::Another => Some(()),
            Round::Finished => None,
        });

        for () in rounds {
            let Ok(decided) = driver.next(Elapsed { total: started.elapsed(), this_try: began.elapsed() });

            let again = match decided {
                Decision::Continue(again) => again,
                Decision::Finished => break,
            };

            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "the waiting between two tries is what this crate is: the far end is not there, nothing will say when it comes back, and asking without a gap is a handheld warm in someone hands"
                )
            )]
            std::thread::sleep(again);

            let Ok(after_the_gap) = now();

            began = after_the_gap;
        }
    });

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the gap before the next try is measured from how long the last one took, which is this crate measuring its own waiting rather than deciding anything from the clock"
    )
)]
fn now() -> Result<Instant, Never> {
    Ok(Instant::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::error::Error;

    const FELL_OVER: Elapsed = Elapsed { total: Duration::ZERO, this_try: Duration::ZERO };

    #[test]
    fn the_first_wait_is_short_enough_to_be_a_blink() {
        let Ok(schedule) = schedule();
        let Ok(mut driver) = schedule.driver();
        let Ok(first) = driver.next(FELL_OVER);

        assert!(FIRST <= Duration::from_secs(1));
        assert_eq!(first, Decision::Continue(FIRST));
    }

    #[test]
    fn a_far_end_that_never_answers_is_left_alone_for_longer_and_never_longer_than_the_longest() {
        let Ok(schedule) = schedule();
        let Ok(mut driver) = schedule.driver();
        let mut waits: Vec<Decision> = Vec::new();

        for _ in 0..10 {
            let Ok(decided) = driver.next(FELL_OVER);

            waits.push(decided);
        }

        assert_eq!(waits.last(), Some(&Decision::Continue(LONGEST)));
        assert!(waits.iter().all(|wait| *wait != Decision::Finished), "a subscription is made again forever");
    }

    #[test]
    fn a_subscription_that_stood_starts_the_waiting_over() {
        let Ok(schedule) = schedule();
        let Ok(mut driver) = schedule.driver();

        for _ in 0..5 {
            let Ok(_growing) = driver.next(FELL_OVER);
        }

        let Ok(after_it_stood) = driver.next(Elapsed { total: Duration::ZERO, this_try: STOOD });

        assert_eq!(after_it_stood, Decision::Continue(FIRST));
    }

    #[test]
    fn a_try_that_failed_at_once_keeps_the_wait_growing() {
        let Ok(schedule) = schedule();
        let Ok(mut driver) = schedule.driver();
        let Ok(_first) = driver.next(FELL_OVER);
        let Ok(nearly) = driver.next(Elapsed { total: Duration::ZERO, this_try: Duration::from_millis(4_999) });

        assert_eq!(nearly, Decision::Continue(FIRST.saturating_mul(2)));
    }

    #[test]
    fn something_that_ends_is_done_again() -> Result<(), Box<dyn Error>> {
        let (say, heard) = std::sync::mpsc::channel();
        let Ok(()) = keep(move || match say.send(()) {
            Ok(()) => Round::Another,
            Err(_nobody_is_listening) => Round::Finished,
        });

        for turn in 1..=3 {
            heard.recv_timeout(Duration::from_secs(10)).map_err(|_| format!("turn {turn} of 3"))?;
        }

        Ok(())
    }

    #[test]
    fn something_that_says_it_is_done_is_left_alone() -> Result<(), Box<dyn Error>> {
        let (say, heard) = std::sync::mpsc::channel();
        let Ok(()) = keep(move || {
            let _heard_or_gone = say.send(());

            Round::Finished
        });

        heard.recv_timeout(Duration::from_secs(5)).map_err(|_| "the one turn")?;

        match heard.recv_timeout(Duration::from_secs(3)) {
            Ok(()) => Err(Box::from("it went round again after saying it was done")),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => Ok(()),
        }
    }
}
