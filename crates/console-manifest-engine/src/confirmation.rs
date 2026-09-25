//! Where the time went, when anyone asks.
//!
//! Nothing on this desktop measures itself. Every `Instant` in the workspace
//! is a deadline or a backoff -- how long to wait before reaching again, how
//! long a subscription has to stand before it counts -- and not one of them is
//! a stopwatch. So "apply feels slower than it used to" has never been a
//! question anyone could answer without adding prints and taking them out
//! again, which is why no one has answered it.
//!
//! An apply is the one thing here long enough to be worth the question. It
//! installs packages, compiles every program on the device, writes sixty
//! files, makes two profiles and restarts a dozen services, on a handheld, and
//! which of those is the minute is not obvious from reading it.
//!
//! Off unless asked, so this costs one `env::var` per stage on the ordinary
//! run and nothing else. `CONSOLE_TIMINGS=1 console apply` is the whole
//! interface.
//!
//! `to` wraps the work in a closure and is what a caller with nothing to hand
//! in wants. `started` and `ended` are the same thing in two halves, for the
//! one caller that cannot use a closure: `Going::during` builds a `Moving` out
//! of a borrow of itself and hands it to the work, and a closure wrapped round
//! that would be a closure holding the permission to write it, which is what
//! EXPLICIT047 is about. The halves are three lines apart in one function body,
//! and a `None` from `started` is an `ended` that prints nothing.

use std::time::Instant;

use console_core_never::Never;

pub const ASKED: &str = "CONSOLE_TIMINGS";

const COLUMN: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmed {
    Yes,
    No,
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "the variable that turns this on, read in the module that is what it turns on"
    )
)]
pub fn asked() -> Result<Confirmed, Never> {
    Ok(match std::env::var(ASKED) {
        Err(_) => Confirmed::No,
        Ok(_) => Confirmed::Yes,
    })
}

pub fn to<T>(doing: &str, work: impl FnOnce() -> T) -> Result<T, Never> {
    let Ok(asked) = asked();

    timing(asked, doing, work)
}

pub fn timing<T>(asked: Confirmed, doing: &str, work: impl FnOnce() -> T) -> Result<T, Never> {
    let Ok(started) = starting(asked);
    let done = work();
    let Ok(()) = ended(doing, started);

    Ok(done)
}

pub fn started() -> Result<Option<Instant>, Never> {
    let Ok(asked) = asked();

    starting(asked)
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the clock is the subject here: this module is what answers where the time went, and an apply is the one caller"
    )
)]
fn starting(asked: Confirmed) -> Result<Option<Instant>, Never> {
    Ok(match asked {
        Confirmed::No => None,
        Confirmed::Yes => Some(Instant::now()),
    })
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the other end of `started`, and an elapsed is the thing being printed"
    )
)]
pub fn ended(doing: &str, started: Option<Instant>) -> Result<(), Never> {
    let Ok(column) = console_core_number_conversion::index(COLUMN);

    match started {
        Some(started) => eprintln!("  {doing:column$} {:>10.1?}", started.elapsed()),
        None => {},
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_work_is_handed_back_whatever_it_says() {
        let Ok(number) = to("nothing", || 7);
        let Ok(word) = to("a word", || "written".to_string());

        assert_eq!(number, 7);
        assert_eq!(word, "written");
    }

    #[test]
    fn a_failure_goes_through_it_whole() {
        let Ok(done) = to("failing", || Err::<(), String>("would not".to_string()));

        assert_eq!(done, Err("would not".to_string()));
    }

    #[test]
    fn being_timed_does_not_change_what_the_work_says() {
        let Ok(number) = timing(Confirmed::Yes, "nothing", || 7);
        let Ok(word) = timing(Confirmed::Yes, "a word", || "written".to_string());
        let Ok(done) = timing(Confirmed::Yes, "failing", || Err::<(), String>("would not".into()));

        assert_eq!(number, 7);
        assert_eq!(word, "written");
        assert_eq!(done, Err("would not".to_string()));
    }

    #[test]
    fn the_work_runs_once_either_way() {
        let mut ran = 0;
        let Ok(()) = timing(Confirmed::No, "quiet", || ran += 1);

        assert_eq!(ran, 1, "untimed work did not run exactly once");

        let mut ran = 0;
        let Ok(()) = timing(Confirmed::Yes, "loud", || ran += 1);

        assert_eq!(ran, 1, "timed work did not run exactly once");
    }
}
