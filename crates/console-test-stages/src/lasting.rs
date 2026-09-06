//! How long each check took last time, and the order that makes.
//!
//! A device run is minutes of somebody's handheld, and until now the only
//! thing that said how many was somebody who had watched one. The strip under
//! the bar can say it while it happens -- `console_notifications::updating` is
//! the file, and an apply already fills it -- but a bar needs to know what
//! fraction of the run each check is, and nothing here knew.
//!
//! ## Measured, not declared
//!
//! `console_manifest::going` gives each stretch of an apply a share written
//! down in its source, said to be an estimate, corrected by hand off a run with
//! `CONSOLE_TIMINGS=1`. Thirteen stretches can be kept honest that way. The
//! checks cannot: there are dozens of them, they arrive one or two at a time,
//! and a number nobody updates is a bar that lies about a run somebody is
//! watching -- which is worse than no bar, because it was asked to be believed.
//!
//! So nothing is declared. Every check is timed as it runs, and what it took is
//! kept, and the next run reads it back. A check nobody has ever timed is given
//! the middle of what is known, which is the honest guess: it is not claimed to
//! be its length, it is claimed to be no more surprising than the rest.
//!
//! The first run on a machine knows nothing at all. Then the strip counts
//! checks rather than time and no estimate is offered, because a bar that
//! divides the run into equal checks is telling somebody the ten-second one and
//! the two-minute one are the same wait. After one run it has the machine's own
//! numbers and never guesses again.
//!
//! ## Kept on the device
//!
//! Beside `waited.jsonl` under `~/.local/state/console`, and not on the laptop
//! driving the run. What is being measured is that machine: the same check is a
//! different wait on a handheld with a full disk cache than on one that just
//! came up, and a laptop that has driven two devices would average them into a
//! number describing neither. It is also where the run would read it from if
//! the device ever asked for the checks itself, which is the direction this is
//! going.
//!
//! Not under `~/.cache`. A cache is an answer that can be worked out again;
//! this one costs a run of the tier to replace.
//!
//! ## Longest first
//!
//! The checks run in the order they grew everywhere else, and that order says
//! something -- it walks the desktop the way it was built. On the device it
//! says nothing anybody watching needs, and it makes the bar crawl and jump by
//! turns. Longest first front-loads the wait, so the strip slows early and runs
//! at the end, which is what `going` says an apply's weights do and for the
//! same reason: it is what the run actually does, rather than a trick played on
//! the reader.
//!
//! It costs the order they grew, so a check that leans on the one before it
//! would break. None does -- every check leaves the desktop as it found it,
//! because it has always had to survive being named on its own.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::time::Duration;

use console_number_conversion::{Float, toward_zero_u16};

use console_never::Never;

use crate::checking::Check;
use crate::device::{Device, quoted};

pub const KEPT: &str = ".local/state/console/checked";

pub fn at(home: &str) -> Result<String, Never> {
    Ok(format!("{home}/{KEPT}"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lengths {
    took: BTreeMap<String, Duration>,
}

impl Lengths {
    pub fn of(&self, name: &str) -> Result<Option<Duration>, Never> {
        Ok(self.took.get(name).copied())
    }

    pub fn learned(&mut self, name: &str, took: Duration) -> Result<(), Never> {
        let _ = self.took.insert(name.to_string(), took);

        Ok(())
    }

    pub fn known(&self) -> Result<usize, Never> {
        Ok(self.took.len())
    }

    pub fn middle(&self) -> Result<Duration, Never> {
        let mut every: Vec<Duration> = self.took.values().copied().collect();

        every.sort_unstable();

        let at = every.len().checked_div(2).unwrap_or_default();

        Ok(every.get(at).copied().unwrap_or_default())
    }

    pub fn longest_first<'a>(&self, checks: &[&'a Check]) -> Result<Vec<&'a Check>, Never> {
        let Ok(middle) = self.middle();
        let mut ordered = checks.to_vec();

        ordered.sort_by_key(|check| {
            let Ok(took) = self.of(check.name);

            Reverse(took.unwrap_or(middle))
        });

        Ok(ordered)
    }
}

pub fn written(lengths: &Lengths) -> Result<String, Never> {
    Ok(lengths
        .took
        .iter()
        .map(|(name, took)| format!("{name} {}\n", took.as_millis()))
        .collect())
}

pub fn reading(held: &str) -> Result<Lengths, Never> {
    let took = held
        .lines()
        .filter_map(|line| line.trim().split_once(' '))
        .filter(|(name, _)| !name.is_empty())
        .filter_map(|(name, said)| match said.trim().parse::<u64>() {
            Ok(milliseconds) => Some((name.to_string(), Duration::from_millis(milliseconds))),
            Err(_not_ours) => None,
        })
        .collect();

    Ok(Lengths { took })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ahead {
    expecting: Vec<Duration>,
    passed: Duration,
    whole: Duration,
    done: usize,
    many: usize,
}

impl Ahead {
    pub fn of(lengths: &Lengths, running: &[&Check]) -> Result<Self, Never> {
        let Ok(middle) = lengths.middle();
        let mut expecting: Vec<Duration> = running
            .iter()
            .map(|check| {
                let Ok(took) = lengths.of(check.name);

                took.unwrap_or(middle)
            })
            .collect();
        let whole = expecting.iter().fold(Duration::ZERO, |so_far, one| so_far.saturating_add(*one));

        expecting.reverse();

        Ok(Ahead { expecting, passed: Duration::ZERO, whole, done: 0, many: running.len() })
    }

    pub fn percent(&self) -> Result<u16, Never> {
        match self.whole.is_zero() {
            true => counted(self.done, self.many),
            false => {
                let Ok(along) = toward_zero_u16(
                    self.passed.as_secs_f64() / self.whole.as_secs_f64() * 100.0,
                );

                Ok(along.min(WHOLE))
            }
        }
    }

    pub fn left(&self) -> Result<Option<Duration>, Never> {
        Ok(match self.whole.is_zero() {
            true => None,
            false => Some(self.whole.saturating_sub(self.passed)),
        })
    }

    pub fn whole(&self) -> Result<Option<Duration>, Never> {
        Ok(match self.whole.is_zero() {
            true => None,
            false => Some(self.whole),
        })
    }

    pub fn finished(&mut self) -> Result<(), Never> {
        let one = self.expecting.pop().unwrap_or_default();

        self.passed = self.passed.saturating_add(one);
        self.done = self.done.saturating_add(1);

        Ok(())
    }
}

pub const WHOLE: u16 = 100;

fn counted(done: usize, many: usize) -> Result<u16, Never> {
    Ok(match many {
        0 => 0,
        many => {
            let Ok(passed) = done.float();
            let Ok(whole) = many.float();
            let Ok(along) = toward_zero_u16(passed / whole * 100.0);

            along.min(WHOLE)
        }
    })
}

const A_MINUTE: u64 = 60;

pub fn about(long: Duration) -> Result<String, Never> {
    let seconds = long.as_secs();
    let minutes = seconds.checked_div(A_MINUTE).unwrap_or_default();

    Ok(match minutes {
        0 => format!("{seconds} seconds"),
        1 => "a minute".to_string(),
        minutes => format!("{minutes} minutes"),
    })
}

pub fn read_from(device: &mut Device) -> Result<Lengths, Never> {
    let Ok(home) = device.home();
    let Ok(at) = at(&home);
    let Ok(quoted) = quoted(&at);
    let Ok(said) = device.user(&format!("cat {quoted} 2>/dev/null"));

    reading(&said)
}

pub fn keep(device: &mut Device, lengths: &Lengths) -> Result<(), Never> {
    match device.dry {
        true => return Ok(()),
        false => {}
    }

    let Ok(home) = device.home();
    let Ok(at) = at(&home);

    let holding = match at.rsplit_once('/') {
        Some((above, _)) => above.to_string(),
        None => ".".to_string(),
    };

    let Ok(holding) = quoted(&holding);
    let Ok(written) = written(lengths);
    let Ok(written) = quoted(&written);
    let Ok(where_) = quoted(&at);
    let Ok(_) = device.user(&format!("mkdir -p {holding} && printf %s {written} > {where_}"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checking::{Body, Done};

    fn nothing(_device: &mut Device) -> Done {
        Ok(())
    }

    const fn check(name: &'static str) -> Check {
        Check {
            name,
            about: "A check.",
            feature: "nothing",
            since: "2026-09-05",
            bodies: &[Body::Device(nothing)],
        }
    }

    const ONE: Check = check("010-one");
    const TWO: Check = check("020-two");
    const THREE: Check = check("030-three");

    fn every() -> Vec<&'static Check> {
        vec![&ONE, &TWO, &THREE]
    }

    fn named(checks: &[&Check]) -> Vec<String> {
        checks.iter().map(|check| check.name.to_string()).collect()
    }

    fn timed(said: &[(&str, u64)]) -> Lengths {
        let mut lengths = Lengths::default();

        for (name, seconds) in said {
            let Ok(()) = lengths.learned(name, Duration::from_secs(*seconds));
        }

        lengths
    }

    fn reading(held: &str) -> Lengths {
        let Ok(lengths) = super::reading(held);

        lengths
    }

    fn written(lengths: &Lengths) -> String {
        let Ok(said) = super::written(lengths);

        said
    }

    fn known(lengths: &Lengths) -> usize {
        let Ok(known) = lengths.known();

        known
    }

    fn middle(lengths: &Lengths) -> Duration {
        let Ok(middle) = lengths.middle();

        middle
    }

    fn longest_first<'a>(lengths: &Lengths, checks: &[&'a Check]) -> Vec<&'a Check> {
        let Ok(ordered) = lengths.longest_first(checks);

        ordered
    }

    fn ahead(lengths: &Lengths, running: &[&Check]) -> Ahead {
        let Ok(ahead) = Ahead::of(lengths, running);

        ahead
    }

    fn percent(ahead: &Ahead) -> u16 {
        let Ok(percent) = ahead.percent();

        percent
    }

    fn left(ahead: &Ahead) -> Option<Duration> {
        let Ok(left) = ahead.left();

        left
    }

    fn whole(ahead: &Ahead) -> Option<Duration> {
        let Ok(whole) = ahead.whole();

        whole
    }

    fn finished(ahead: &mut Ahead) {
        let Ok(()) = ahead.finished();
    }

    fn about(took: Duration) -> String {
        let Ok(said) = super::about(took);

        said
    }

    #[test]
    fn what_a_run_writes_down_is_what_the_next_one_reads() {
        let lengths = timed(&[("010-one", 4), ("020-two", 90)]);

        assert_eq!(reading(&written(&lengths)), lengths);
    }

    #[test]
    fn a_line_nothing_here_wrote_is_not_read_as_a_length() {
        for held in ["", "\n", "010-one", "010-one later", " 12", "010-one 12s"] {
            assert_eq!(known(&reading(held)), 0, "{held:?} was read as a length");
        }
    }

    #[test]
    fn the_longest_goes_first() {
        let lengths = timed(&[("010-one", 4), ("020-two", 90), ("030-three", 20)]);

        assert_eq!(named(&longest_first(&lengths, &every())), ["020-two", "030-three", "010-one"]);
    }

    #[test]
    fn a_check_nothing_has_ever_timed_weighs_the_middle_and_keeps_its_place() {
        let lengths = timed(&[("010-one", 4), ("030-three", 90)]);

        assert_eq!(middle(&lengths), Duration::from_secs(90));
        assert_eq!(named(&longest_first(&lengths, &every())), ["020-two", "030-three", "010-one"]);
    }

    #[test]
    fn a_machine_nobody_has_timed_counts_checks_and_promises_no_time() {
        let mut ahead = ahead(&Lengths::default(), &every());

        assert_eq!(percent(&ahead), 0);
        assert_eq!(left(&ahead), None);
        assert_eq!(whole(&ahead), None);
        finished(&mut ahead);

        assert_eq!(percent(&ahead), 33);
    }

    #[test]
    fn the_strip_moves_by_time_rather_than_by_check() {
        let lengths = timed(&[("010-one", 10), ("020-two", 80), ("030-three", 10)]);
        let ordered = longest_first(&lengths, &every());
        let mut ahead = ahead(&lengths, &ordered);

        assert_eq!(whole(&ahead), Some(Duration::from_secs(100)));
        finished(&mut ahead);

        assert_eq!(percent(&ahead), 80, "the long one is most of the run and the strip says so");
        assert_eq!(left(&ahead), Some(Duration::from_secs(20)));
    }

    #[test]
    fn the_strip_ends_full_and_goes_no_further() {
        let lengths = timed(&[("010-one", 10), ("020-two", 80), ("030-three", 10)]);
        let mut ahead = ahead(&lengths, &every());

        for _each in every() {
            finished(&mut ahead);
        }

        assert_eq!(percent(&ahead), WHOLE);
        assert_eq!(left(&ahead), Some(Duration::ZERO));
        finished(&mut ahead);

        assert_eq!(percent(&ahead), WHOLE, "a check nobody expected pushed the strip off the end");
    }

    #[test]
    fn a_length_is_said_the_way_somebody_waiting_would_say_it() {
        assert_eq!(about(Duration::from_secs(40)), "40 seconds");
        assert_eq!(about(Duration::from_secs(75)), "a minute");
        assert_eq!(about(Duration::from_secs(400)), "6 minutes");
    }
}
