//! Reaching again for something that has gone.
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

use console_never::Never;
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

pub fn keep(mut once: impl FnMut() -> Round + Send + 'static) -> Result<(), Never> {
    let _ = std::thread::spawn(move || {
        let mut waited = FIRST;

        loop {
            let began = Instant::now();

            match once() {
                Round::Done => return,
                Round::Another => {}
            }

            let Ok(again) = after(waited, began.elapsed());

            std::thread::sleep(again);

            let Ok(longer) = longer(again);

            waited = longer;
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
