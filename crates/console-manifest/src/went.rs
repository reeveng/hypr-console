//! Where the time went, when anybody asks.
//!
//! Nothing on this desktop measures itself. Every `Instant` in the workspace
//! is a deadline or a backoff -- how long to wait before reaching again, how
//! long a subscription has to stand before it counts -- and not one of them is
//! a stopwatch. So "apply feels slower than it used to" has never been a
//! question anybody could answer without adding prints and taking them out
//! again, which is why nobody has answered it.
//!
//! An apply is the one thing here long enough to be worth the question. It
//! installs packages, compiles every program on the device, writes sixty
//! files, makes two profiles and restarts a dozen services, on a handheld, and
//! which of those is the minute is not obvious from reading it.
//!
//! Off unless asked, so this costs one `env::var` per stretch on the ordinary
//! run and nothing else. `CONSOLE_TIMINGS=1 console apply` is the whole
//! interface.

use std::time::Instant;

/// The word that turns it on.
pub const ASKED: &str = "CONSOLE_TIMINGS";

/// How wide the name of a stretch is written, so the times line up under each
/// other whatever they are called.
const COLUMN: usize = 20;

/// Whether anybody asked.
///
/// A word rather than a `bool`, so that `timing(Asked::No, ...)` reads as
/// what it is at the call site and the timed path can be asked for directly
/// by a test. Reading the environment inside the only function there was
/// meant the half that measures was the half nothing could reach: a test
/// would have had to set a variable for the whole process, and these run
/// alongside each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Yes,
    No,
}

/// Whether the word is set, whatever it is set to.
///
/// The value is not read. `CONSOLE_TIMINGS=0` asking for timings is worse
/// than it looks the other way round: somebody who typed the variable at all
/// wants the numbers.
pub fn asked() -> Asked {
    match std::env::var(ASKED) {
        // Not set, or set to something that is not text. Neither is somebody
        // asking for numbers.
        Err(_) => Asked::No,
        Ok(_) => Asked::Yes,
    }
}

/// Run one named stretch of work, and say how long it took if anybody asked.
///
/// The work is handed back whatever it returns, so putting a stretch under
/// this changes nothing about what the code around it does or means. Written
/// on stderr rather than stdout, so an apply's own account of what it changed
/// stays the thing you can read or pipe on its own.
pub fn to<T>(doing: &str, work: impl FnOnce() -> T) -> T {
    timing(asked(), doing, work)
}

/// The same, told rather than asked.
pub fn timing<T>(asked: Asked, doing: &str, work: impl FnOnce() -> T) -> T {
    match asked {
        Asked::No => work(),
        Asked::Yes => {
            let started = Instant::now();
            let done = work();
            eprintln!("  {doing:COLUMN$} {:>10.1?}", started.elapsed());
            done
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole promise: what the work says is what the caller gets, timed or
    /// not. A stretch that changed its answer when somebody switched the
    /// timings on would be worse than no timings.
    #[test]
    fn the_work_is_handed_back_whatever_it_says() {
        assert_eq!(to("nothing", || 7), 7);
        assert_eq!(to("a word", || "written".to_string()), "written");
    }

    /// And a failure travels through it unchanged, because most of what an
    /// apply does is fallible and none of it should have to be unwrapped to be
    /// timed.
    #[test]
    fn a_failure_goes_through_it_whole() {
        let done: Result<(), String> = to("failing", || Err("would not".to_string()));
        assert_eq!(done, Err("would not".to_string()));
    }

    /// The timed half is the half nothing exercised, and it is the half that
    /// runs while somebody is debugging a slow apply -- which is the worst
    /// moment for it to be the thing that changed the answer.
    #[test]
    fn being_timed_does_not_change_what_the_work_says() {
        assert_eq!(timing(Asked::Yes, "nothing", || 7), 7);
        assert_eq!(timing(Asked::Yes, "a word", || "written".to_string()), "written");
        let done: Result<(), String> = timing(Asked::Yes, "failing", || Err("would not".into()));
        assert_eq!(done, Err("would not".to_string()));
    }

    /// And it runs the work exactly once, timed or not.
    ///
    /// A stretch run twice is an apply that installs twice. Nothing in the
    /// shape of this invites it, which is why it is worth one assertion: the
    /// work is a `FnOnce` and the compiler would refuse a second call, but
    /// the count also catches a rewrite that made it `Fn` to get at something.
    #[test]
    fn the_work_runs_once_either_way() {
        let mut ran = 0;
        timing(Asked::No, "quiet", || ran += 1);
        assert_eq!(ran, 1, "untimed work did not run exactly once");

        let mut ran = 0;
        timing(Asked::Yes, "loud", || ran += 1);
        assert_eq!(ran, 1, "timed work did not run exactly once");
    }
}
