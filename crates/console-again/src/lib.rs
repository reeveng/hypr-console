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

use std::time::{Duration, Instant};

/// How long to wait before the first try after one has ended.
///
/// A daemon restarting, a resume from sleep, a compositor socket that was not
/// there when the bar started: all of those are over in about a second, and
/// this is what makes them cost about a second.
pub const FIRST: Duration = Duration::from_secs(1);

/// The longest it will ever wait.
///
/// The wait grows because a source that is not on this machine at all would
/// otherwise be a wake-up every second for the life of the session, on a
/// machine that runs off a battery. `nmcli monitor` on a device with no
/// NetworkManager is exactly that, and the comment in `watch.rs` about a
/// machine where one of these is not running is about a real one.
pub const LONGEST: Duration = Duration::from_secs(60);

/// How long a subscription has to stand before it counts as having worked.
///
/// Without this a far end that accepts a connection and drops it at once --
/// a daemon in a restart loop of its own -- is met with a retry as fast as the
/// thing it is retrying, which is the wake-up every second this is here to
/// avoid. Standing this long is what makes the next failure a fresh one.
pub const STOOD: Duration = Duration::from_secs(5);

/// The wait to use now, given the one used last and how long the try stood.
///
/// A subscription that stood long enough to be real starts the waiting over,
/// so the ordinary case -- something restarted under us -- always heals in
/// `FIRST` however long the program has been running.
pub fn after(waited: Duration, stood: Duration) -> Duration {
    match stood >= STOOD {
        true => FIRST,
        false => waited,
    }
}

/// The wait after this one, for as long as nothing answers.
pub fn longer(waited: Duration) -> Duration {
    (waited * 2).min(LONGEST)
}

/// Whether a retry loop has any reason to go round again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Round {
    /// Somebody is still listening, so try again after the wait.
    Another,
    /// Nobody is, or what was asked for can never arrive. This is the end of it.
    Done,
}

/// Do something over and over, waiting longer each time it comes to nothing.
///
/// `once` is one attempt, from making the subscription to the moment it ends,
/// and it answers whether another is wanted. `Done` is the end of it: nobody
/// is listening any more, or what was asked for is not a thing that can ever
/// arrive.
///
/// The thread is the point. Every caller here already has a loop of its own
/// that has to go on answering while this waits, and a socket cannot be read
/// on the same thread as a timeout.
pub fn keep(mut once: impl FnMut() -> Round + Send + 'static) {
    std::thread::spawn(move || {
        let mut waited = FIRST;

        loop {
            let began = Instant::now();

            if once() == Round::Done {
                return;
            }

            waited = after(waited, began.elapsed());
            std::thread::sleep(waited);
            waited = longer(waited);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_wait_is_short_enough_to_be_a_blink() {
        assert!(FIRST <= Duration::from_secs(1));
    }

    /// The whole point: something that came back stays cheap to notice.
    #[test]
    fn a_subscription_that_stood_starts_the_waiting_over() {
        assert_eq!(after(Duration::from_secs(32), STOOD), FIRST);
        assert_eq!(after(LONGEST, Duration::from_secs(600)), FIRST);
    }

    /// And something that never answers is asked after less and less often.
    #[test]
    fn a_far_end_that_never_answers_is_left_alone_for_longer() {
        let mut waited = FIRST;
        for _ in 0..10 {
            waited = longer(after(waited, Duration::from_millis(0)));
        }
        assert_eq!(waited, LONGEST);
    }

    #[test]
    fn the_waiting_never_grows_past_the_longest() {
        assert_eq!(longer(LONGEST), LONGEST);
        assert!(longer(Duration::from_secs(59)) <= LONGEST);
    }

    /// A try that failed at once does not reset anything, or the growth above
    /// never happens and the battery pays for it.
    #[test]
    fn a_try_that_failed_at_once_keeps_the_wait_it_had() {
        let waited = Duration::from_secs(8);
        assert_eq!(after(waited, Duration::from_millis(1)), waited);
    }

    /// What every watcher on this desktop rests on: a thing that ends is a
    /// thing that is done again, without the caller asking twice.
    #[test]
    fn something_that_ends_is_done_again() {
        let (say, heard) = std::sync::mpsc::channel();
        keep(move || match say.send(()) {
            Ok(()) => Round::Another,
            Err(_) => Round::Done,
        });
        for turn in 1..=3 {
            heard
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| panic!("turn {turn} of 3"));
        }
    }

    /// And it stops when it says it is done, rather than waking a handheld
    /// every minute for the rest of the session on nobody's behalf.
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
