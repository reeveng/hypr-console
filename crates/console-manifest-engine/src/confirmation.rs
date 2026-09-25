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
//! Printed only when asked: `CONSOLE_TIMINGS=1 console apply` puts each stage
//! on stderr as it ends. Kept whether asked or not: every apply's stages go
//! into `APPLIED` as one line in `console-response-times`' shape, so "is an
//! apply slower than it was, and which stage" is a question history answers
//! rather than one somebody has to be at the terminal for.
//! `console-response-times --file /var/lib/console/waited.jsonl` reads it.
//! It lives beside the generations rather than in the owner's home, because
//! the apply is root and a file it made there is one the owner's own programs
//! could no longer append to. What it does ask of the owner's home is the
//! `measuring` setting, which is the owner's to choose, and off means no line.
//!
//! `to` wraps the work in a closure and is what a caller with nothing to hand
//! in wants. `started` and `ended` are the same thing in two halves, for the
//! one caller that cannot use a closure: `Going::during` builds a `Moving` out
//! of a borrow of itself and hands it to the work, and a closure wrapped round
//! that would be a closure holding the permission to write it, which is what
//! EXPLICIT047 is about. The halves are three lines apart in one function body,
//! and what `ended` hands back is what `Going` keeps for the line.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use console_core_never::Never;
use console_response_times::line::{self, Entry};
use console_response_times::measuring::{self, Measuring};

pub const ASKED: &str = "CONSOLE_TIMINGS";

pub const APPLIED: &str = "/var/lib/console/waited.jsonl";

const WHO: &str = "console";

const WHAT: &str = "apply";

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
        Err(_unset) => Confirmed::No,
        Ok(_) => Confirmed::Yes,
    })
}

pub fn to<T>(doing: &str, work: impl FnOnce() -> T) -> Result<T, Never> {
    let Ok(asked) = asked();

    timing(asked, doing, work)
}

pub fn timing<T>(asked: Confirmed, doing: &str, work: impl FnOnce() -> T) -> Result<T, Never> {
    let Ok(started) = started();
    let done = work();
    let Ok(took) = took_since(started);
    let Ok(()) = printed(asked, doing, took);

    Ok(done)
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the clock is the subject here: this module is what answers where the time went, and an apply is the one caller"
    )
)]
pub fn started() -> Result<Instant, Never> {
    Ok(Instant::now())
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the other end of `started`, and an elapsed is the thing being measured"
    )
)]
fn took_since(started: Instant) -> Result<Duration, Never> {
    Ok(started.elapsed())
}

pub fn ended(doing: &str, started: Instant) -> Result<Duration, Never> {
    let Ok(took) = took_since(started);
    let Ok(asked) = asked();
    let Ok(()) = printed(asked, doing, took);

    Ok(took)
}

fn printed(asked: Confirmed, doing: &str, took: Duration) -> Result<(), Never> {
    let Ok(column) = console_core_number_conversion::index(COLUMN);

    match asked {
        Confirmed::Yes => eprintln!("  {doing:column$} {took:>10.1?}"),
        Confirmed::No => {},
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct Moment {
    pub at: u64,
    pub up: f64,
    pub load: f64,
}

pub fn entry(stages: &[(&'static str, Duration)], took: Duration, moment: Moment) -> Result<Entry, Never> {
    Ok(Entry {
        at: moment.at,
        up: moment.up,
        load: moment.load,
        who: WHO.to_string(),
        what: WHAT.to_string(),
        waited: took,
        marks: stages.iter().map(|(label, took)| ((*label).to_string(), *took)).collect(),
        notes: Vec::new(),
    })
}

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the moment the line is written is part of the line, and this is the module whose subject is the clock"
    )
)]
fn now() -> Result<Moment, Never> {
    let at = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => since.as_secs(),
        Err(_before_the_epoch) => 0,
    };
    let Ok(up) = console_response_times::uptime();
    let up = match up {
        Some(up) => up.as_secs_f64(),
        None => 0.0,
    };
    let load = match console_response_times::load() {
        Ok(load) => load,
        Err(fault) => {
            eprintln!("console: the apply's line says no load: {fault}");

            0.0
        }
    };

    Ok(Moment { at, up, load })
}

pub fn chosen_in(owner_home: &Path) -> Result<Measuring, Never> {
    let Ok(at) = console_defaults::under(owner_home);
    let Ok(held) = console_core_atomic_writes::read(&at);

    let said = match held {
        console_core_atomic_writes::Stored::Text(said) => said,
        console_core_atomic_writes::Stored::Absent => return measuring::read(None),
        console_core_atomic_writes::Stored::Failed(fault) => {
            eprintln!("console: {} will not be read ({fault}), so the apply is measured as though nobody chose", at.display());

            return measuring::read(None);
        }
    };
    let Ok(settings) = console_defaults::read(&said);
    let chosen = settings.iter().find(|(key, _)| key == measuring::SETTING).map(|(_, value)| value.as_str());

    measuring::read(chosen)
}

pub fn kept(owner_home: &Path, stages: &[(&'static str, Duration)], took: Duration) -> Result<(), Never> {
    let Ok(chosen) = chosen_in(owner_home);

    match chosen {
        Measuring::On => {}
        Measuring::Off => return Ok(()),
    }

    let Ok(moment) = now();
    let Ok(entry) = entry(stages, took, moment);
    let Ok(said) = line::written(&entry);

    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "a log appended to a line at a time: a rename over it would take away every apply already in it, and one short line written to an O_APPEND descriptor lands whole"
        )
    )]
    let opened = OpenOptions::new().create(true).append(true).open(APPLIED);

    match opened.and_then(|mut file| file.write_all(format!("{said}\n").as_bytes())) {
        Ok(()) => {}
        Err(fault) => eprintln!("console: {APPLIED}: where this apply's time went is not kept: {fault}"),
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
    fn an_apply_is_one_line_whose_stages_come_in_the_order_they_ran() {
        let stages = [("built", Duration::from_millis(61_000)), ("files", Duration::from_millis(1_500))];
        let Ok(entry) = entry(&stages, Duration::from_millis(70_000), Moment { at: 1_758_000_000, up: 3_600.0, load: 1.5 });
        let Ok(said) = line::written(&entry);
        let Ok(back) = line::read(&said);
        let back = back.expect("a line this module wrote reads back");

        assert_eq!((back.who.as_str(), back.what.as_str()), ("console", "apply"));
        assert_eq!(back.waited, Duration::from_millis(70_000));
        assert_eq!(
            back.marks.iter().map(|(label, _)| label.as_str()).collect::<Vec<_>>(),
            vec!["built", "files"]
        );
    }

    #[test]
    fn with_measuring_off_in_the_owners_home_nothing_is_kept() {
        let home = std::env::temp_dir().join(format!("console-apply-measuring-{}", std::process::id()));
        let Ok(at) = console_defaults::under(&home);
        let _ = std::fs::create_dir_all(at.parent().expect("the defaults file is under a directory"));
        let _ = std::fs::write(&at, "measuring=off\n");

        assert_eq!(chosen_in(&home), Ok(Measuring::Off));

        let _ = std::fs::write(&at, "search=ddg\n");

        assert_eq!(chosen_in(&home), Ok(Measuring::On));
        let _ = std::fs::remove_dir_all(&home);
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
