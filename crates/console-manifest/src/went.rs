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

use console_never::Never;

pub const ASKED: &str = "CONSOLE_TIMINGS";

const COLUMN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Yes,
    No,
}

pub fn asked() -> Result<Asked, Never> {
    Ok(match std::env::var(ASKED) {
        Err(_) => Asked::No,
        Ok(_) => Asked::Yes,
    })
}

pub fn to<T>(doing: &str, work: impl FnOnce() -> T) -> Result<T, Never> {
    let Ok(asked) = asked();

    timing(asked, doing, work)
}

pub fn timing<T>(asked: Asked, doing: &str, work: impl FnOnce() -> T) -> Result<T, Never> {
    Ok(match asked {
        Asked::No => work(),
        Asked::Yes => {
            let started = Instant::now();
            let done = work();
            eprintln!("  {doing:COLUMN$} {:>10.1?}", started.elapsed());
            done
        },
    })
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
        let Ok(number) = timing(Asked::Yes, "nothing", || 7);
        let Ok(word) = timing(Asked::Yes, "a word", || "written".to_string());
        let Ok(done) = timing(Asked::Yes, "failing", || Err::<(), String>("would not".into()));

        assert_eq!(number, 7);
        assert_eq!(word, "written");
        assert_eq!(done, Err("would not".to_string()));
    }

    #[test]
    fn the_work_runs_once_either_way() {
        let mut ran = 0;
        let Ok(()) = timing(Asked::No, "quiet", || ran += 1);

        assert_eq!(ran, 1, "untimed work did not run exactly once");

        let mut ran = 0;
        let Ok(()) = timing(Asked::Yes, "loud", || ran += 1);

        assert_eq!(ran, 1, "timed work did not run exactly once");
    }
}
