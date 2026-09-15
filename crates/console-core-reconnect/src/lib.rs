//! A subscription made again, for as long as somebody still wants one.
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
//! again forever. A program already inside a loop of somebody else's -- a
//! panel's, which is glib's -- cannot be handed a thread and has to do its own
//! awaiting, so [`Between`] is the same decision without the thread: how long
//! before the next try, given how long the last one stood. `keep` is written on
//! it, which is what stops the two from drifting.

use console_core_never::Never;
use std::time::{Duration, Instant};

pub const FIRST: Duration = Duration::from_secs(1);

pub const LONGEST: Duration = Duration::from_secs(60);

pub const STOOD: Duration = Duration::from_secs(5);

pub fn after(waited: Duration, stood: Duration) -> Result<Duration, Never> {
    Ok(match stood >= STOOD {
        true => FIRST,
        false => waited,
    })
}

pub fn longer(waited: Duration) -> Result<Duration, Never> {
    Ok((waited * 2).min(LONGEST))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    Another,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Between {
    waited: Duration,
}

impl Between {
    pub fn tries() -> Result<Self, Never> {
        Ok(Self { waited: FIRST })
    }

    pub fn after(&mut self, stood: Duration) -> Result<Duration, Never> {
        let Ok(again) = after(self.waited, stood);

        let Ok(longer) = longer(again);

        self.waited = longer;

        Ok(again)
    }
}

pub fn keep(mut once: impl FnMut() -> Round + Send + 'static) -> Result<(), Never> {
    #[cfg_attr(
        dylint_lib = "explicit035_no_loose_thread",
        allow(
            explicit035_no_loose_thread,
            reason = "this crate is what a subscription made again is, and the thread is the making: it runs until the caller's own round says Done or the program ends, which is what `keep` is asked for. There is nothing above it to hold a handle, and the word this rule asks for is the name of the function"
        )
    )]
    let _ = std::thread::spawn(move || {
        let Ok(mut between) = Between::tries();

        loop {
            #[cfg_attr(
                dylint_lib = "explicit039_no_reading_the_clock",
                allow(
                    explicit039_no_reading_the_clock,
                    reason = "the gap before the next try is measured from how long the last one took, which is this crate measuring its own waiting rather than deciding anything from the clock"
                )
            )]
            let began = Instant::now();

            match once() {
                Round::Done => return,
                Round::Another => {}
            }

            let Ok(again) = between.after(began.elapsed());

            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "the waiting between two tries is what this crate is: the far end is not there, nothing will say when it comes back, and asking without a gap is a handheld warm in somebody hands"
                )
            )]
            std::thread::sleep(again);
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_wait_is_short_enough_to_be_a_blink() {
        assert!(FIRST <= Duration::from_secs(1));
    }

    #[test]
    fn a_subscription_that_stood_starts_the_waiting_over() {
        let Ok(stood) = after(Duration::from_secs(32), STOOD);
        let Ok(longest) = after(LONGEST, Duration::from_secs(600));

        assert_eq!(stood, FIRST);
        assert_eq!(longest, FIRST);
    }

    #[test]
    fn a_far_end_that_never_answers_is_left_alone_for_longer() {
        let mut waited = FIRST;
        for _ in 0..10 {
            let Ok(kept) = after(waited, Duration::from_millis(0));
            let Ok(longer) = longer(kept);

            waited = longer;
        }
        assert_eq!(waited, LONGEST);
    }

    #[test]
    fn the_waiting_never_grows_past_the_longest() {
        let Ok(at_the_longest) = longer(LONGEST);
        let Ok(nearly) = longer(Duration::from_secs(59));

        assert_eq!(at_the_longest, LONGEST);
        assert!(nearly <= LONGEST);
    }

    #[test]
    fn a_try_that_failed_at_once_keeps_the_wait_it_had() {
        let waited = Duration::from_secs(8);
        let Ok(kept) = after(waited, Duration::from_millis(1));

        assert_eq!(kept, waited);
    }

    #[test]
    fn the_waiting_a_caller_does_itself_is_the_waiting_the_thread_does() {
        let Ok(mut between) = Between::tries();
        let mut waited = FIRST;

        for _ in 0..10 {
            let Ok(theirs) = after(waited, Duration::from_millis(0));
            let Ok(longer) = longer(theirs);

            waited = longer;

            let Ok(ours) = between.after(Duration::from_millis(0));

            assert_eq!(ours, theirs);
        }
    }

    #[test]
    fn a_caller_whose_last_try_stood_waits_from_the_start_again() {
        let Ok(mut between) = Between::tries();

        for _ in 0..5 {
            let Ok(_growing) = between.after(Duration::from_millis(0));
        }

        let Ok(after_it_stood) = between.after(STOOD);

        assert_eq!(after_it_stood, FIRST);
    }

    #[test]
    fn something_that_ends_is_done_again() {
        let (say, heard) = std::sync::mpsc::channel();
        let Ok(()) = keep(move || match say.send(()) {
            Ok(()) => Round::Another,
            Err(_) => Round::Done,
        });
        for turn in 1..=3 {
            heard
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| panic!("turn {turn} of 3"));
        }
    }

    #[test]
    fn something_that_says_it_is_done_is_left_alone() {
        let (say, heard) = std::sync::mpsc::channel();
        keep(move || {
            say.send(()).ok();
            Round::Done
        });
        heard.recv_timeout(Duration::from_secs(5)).expect("the one turn");
        assert!(
            heard.recv_timeout(Duration::from_secs(3)).is_err(),
            "it went round again after saying it was done"
        );
    }
}
