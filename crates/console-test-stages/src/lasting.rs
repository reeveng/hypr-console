//! How long each check took last time, and the order that makes.
//!
//! A device run is minutes of somebody's handheld, and until now the only
//! thing that said how many was somebody who had watched one. The strip under
//! the bar can say it while it happens -- `console_notifications::updating` is
//! the file, and an apply already fills it -- but a bar needs to know what
//! fraction of the run each check is, and nothing here knew.
//!
//! ## Measured, not declared
//!  `console_manifest_engine::going` gives each stretch of an apply a share
//! written down in its source, said to be an estimate, corrected by hand off a
//! run with `CONSOLE_TIMINGS=1`. A handful of stretches can be kept honest that
//! way. The checks cannot: there are dozens of them, they arrive one or two at
//! a time, and a number nobody updates is a bar that lies about a run somebody
//! is watching -- which is worse than no bar, because it was asked to be
//! believed.  So nothing is declared. Every check is timed as it runs, and what
//! it took is kept, and the next run reads it back.
//!
//! ## And a second table behind it
//!
//! What that leaves is the first run on a machine, which knows nothing at all
//! -- and that is the run most likely to be watched, because it is the one on a
//! device somebody has just put back together. It used to count checks, which
//! tells a person the two-second one and the three-minute one are the same
//! wait.
//!
//! So `baseline` carries a table in the source as well, and the two are not
//! rivals. This machine's own measurement is used wherever it exists and the
//! carried one is not consulted; the carried one answers only where this
//! machine has nothing to say. What makes that work is that the two tables
//! differ mostly by a single number: which check is the slow one is a fact
//! about the checks, and how fast the whole run goes is a fact about the
//! machine. `pace_of` measures that one number over the checks both tables
//! know, and `expecting` carries the rest across at it. A check in neither
//! table still gets the middle of what is known, which is not a claim about its
//! length but a claim that it is no more surprising than the rest.
//!
//! ## Only what a check that worked has to say
//!
//! A length is learned from a check that passed and from nothing else. A check
//! that fails does it at the first thing that is wrong, which is usually early,
//! and a run that goes red therefore teaches the table that the check is quick
//! -- so the bar on the next run is confidently wrong about the longest thing
//! in it, and it was a failure that made it so. A skipped one has not run at
//! all. Neither is a measurement of anything, and a table is worth more with a
//! gap in it than with a number nobody should believe.
//!
//! ## And a third correction, while the run is happening
//!
//! Both tables are from before. `Ahead::pace` is the same ratio measured
//! against this run as it goes: what the finished checks were expected to take
//! against what they actually took. It starts at one and every check that ends
//! says how wrong the estimate was, so a machine that is busy, or cold, or
//! simply not the machine the table came from, is corrected within a check or
//! two rather than at the end. It is clamped, because one check that hung is
//! not evidence about the rest of the run.
//!
//! ## Leaning behind
//!
//! `leaning` bends the whole thing down a little: at the halfway mark of the
//! run the bar says forty per cent, and it catches up as it goes. That is
//! deliberate and it is not decoration. A bar that runs ahead of the work
//! arrives at ninety-eight and stops there, and the last two per cent taking a
//! third of the wait is the exact thing that makes somebody stop believing a
//! bar -- once, and then for every bar after it. A bar that lags and then
//! gathers speed is never wrong in the direction that costs anything.
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
//!
//! ## Inside the one that is running
//!
//! Longest first has a cost of its own: the first check of the run is the
//! longest one there is, so the strip stood at nothing for the worst two
//! minutes of the run -- the two minutes a person is most likely to conclude it
//! has hung. A check that has been timed before carries its own estimate, so
//! the run can say how far into it is as it goes, and `crept` is that: it walks
//! the check's own share of the bar with the clock, and it keeps a tenth of it
//! back.
//!
//! The tenth is what makes it honest rather than a guess dressed as a fact. An
//! estimate off a previous run is right until the machine is busy, the disk
//! cache is cold or the check is waiting on something that will not come, and a
//! bar that ran to the end of a check's share and sat there would say the check
//! had finished when it had not. So the clock fills nine tenths of the share
//! and the last tenth is only ever approached, never reached: the check
//! arriving is the one thing that fills it. A check running long still moves --
//! slower and slower, which is the truth about it -- and a check that has hung
//! is a bar that has visibly stopped climbing rather than one that lied and
//! stopped.
//!
//! Past its estimate the line says so in words as well. A bar creeping through
//! its last tenth and a bar that has stopped look the same in a photograph, and
//! the thing a person actually wants to know at that moment is not a
//! percentage: it is that this check has been running longer than it usually
//! does, and how much longer. `over` is that question, and the answer goes on
//! the line beside the check's own name.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::time::Duration;

use console_core_number_conversion::{Float, toward_zero_u16};

use console_core_never::Never;

use crate::checking::Check;
use crate::device::{Device, quoted};

pub const NAMED: &str = "checked";

pub fn at(home: &str) -> Result<String, Never> {
    let Ok(ours) = console_core_places::Base::State.ours_under(std::path::Path::new(home));

    Ok(ours.join(NAMED).display().to_string())
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

    pub fn names(&self) -> Result<Vec<String>, Never> {
        Ok(self.took.keys().cloned().collect())
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

pub fn pace_of(measured: &Lengths, carried: &Lengths) -> Result<f64, Never> {
    let both: Vec<(Duration, Duration)> = measured
        .took
        .iter()
        .filter_map(|(name, here)| {
            let Ok(there) = carried.of(name);

            there.map(|there| (*here, there))
        })
        .filter(|(_, there)| !there.is_zero())
        .collect();
    let here = both.iter().fold(Duration::ZERO, |so_far, (here, _)| so_far.saturating_add(*here));
    let there = both.iter().fold(Duration::ZERO, |so_far, (_, there)| so_far.saturating_add(*there));

    Ok(match there.is_zero() {
        true => 1.0,
        false => (here.as_secs_f64() / there.as_secs_f64()).clamp(QUICKEST, SLOWEST),
    })
}

pub fn expecting(
    measured: &Lengths,
    carried: &Lengths,
    pace: f64,
    name: &str,
) -> Result<Duration, Never> {
    let Ok(here) = measured.of(name);

    match here {
        Some(here) => return Ok(here),
        None => {},
    }

    let Ok(there) = carried.of(name);

    match there {
        Some(there) => return Ok(there.mul_f64(pace)),
        None => {},
    }

    let Ok(middle) = measured.middle();

    match middle.is_zero() {
        false => Ok(middle),

        true => {
            let Ok(middle) = carried.middle();

            Ok(middle.mul_f64(pace))
        }
    }
}

pub const SLOWEST: f64 = 8.0;

pub const QUICKEST: f64 = 0.125;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ahead {
    expecting: Vec<Duration>,
    passed: Duration,
    whole: Duration,
    took: Duration,
    done: usize,
    many: usize,
}

impl Ahead {
    pub fn of(lengths: &Lengths, running: &[&Check]) -> Result<Self, Never> {
        let Ok(carried) = crate::baseline::lengths();

        Ahead::from(lengths, &carried, running)
    }

    pub fn from(
        measured: &Lengths,
        carried: &Lengths,
        running: &[&Check],
    ) -> Result<Self, Never> {
        let Ok(pace) = pace_of(measured, carried);
        let mut expecting: Vec<Duration> = running
            .iter()
            .map(|check| {
                let Ok(one) = expecting(measured, carried, pace, check.name);

                one
            })
            .collect();
        let whole = expecting.iter().fold(Duration::ZERO, |so_far, one| so_far.saturating_add(*one));

        expecting.reverse();

        Ok(Ahead {
            expecting,
            passed: Duration::ZERO,
            whole,
            took: Duration::ZERO,
            done: 0,
            many: running.len(),
        })
    }

    pub fn pace(&self) -> Result<f64, Never> {
        match self.passed.is_zero() {
            true => Ok(1.0),
            false => {
                let pace = self.took.as_secs_f64() / self.passed.as_secs_f64();

                Ok(pace.clamp(QUICKEST, SLOWEST))
            }
        }
    }

    pub fn percent(&self) -> Result<u16, Never> {
        match self.whole.is_zero() {
            true => counted(self.done, self.many),
            false => {
                let Ok(leaning) =
                    leaning(self.passed.as_secs_f64() / self.whole.as_secs_f64());
                let Ok(along) = toward_zero_u16(leaning * 100.0);

                Ok(along.min(WHOLE))
            }
        }
    }

    pub fn along(&self, gone: Duration) -> Result<u16, Never> {
        match self.whole.is_zero() {
            true => counted(self.done, self.many),
            false => {
                let Ok(inside) = self.inside(gone);
                let at = self.passed.saturating_add(inside);
                let Ok(leaning) = leaning(at.as_secs_f64() / self.whole.as_secs_f64());
                let Ok(along) = toward_zero_u16(leaning * 100.0);

                Ok(along.min(WHOLE))
            }
        }
    }

    fn part(&self, gone: Duration) -> Result<f64, Never> {
        let Ok(pace) = self.pace();
        let Ok(one) = self.expecting();

        crept(gone.div_f64(pace), one)
    }

    fn inside(&self, gone: Duration) -> Result<Duration, Never> {
        let Ok(part) = self.part(gone);
        let Ok(one) = self.expecting();

        Ok(one.mul_f64(part))
    }

    pub fn left(&self, gone: Duration) -> Result<Option<Duration>, Never> {
        match self.whole.is_zero() {
            true => Ok(None),
            false => {
                let Ok(pace) = self.pace();
                let Ok(inside) = self.inside(gone);
                let at = self.passed.saturating_add(inside);

                Ok(Some(self.whole.saturating_sub(at).mul_f64(pace)))
            }
        }
    }

    pub fn whole(&self) -> Result<Option<Duration>, Never> {
        Ok(match self.whole.is_zero() {
            true => None,
            false => Some(self.whole),
        })
    }

    pub fn expecting(&self) -> Result<Duration, Never> {
        Ok(self.expecting.last().copied().unwrap_or_default())
    }

    pub fn span(&self) -> Result<u16, Never> {
        match self.whole.is_zero() {
            true => {
                let Ok(one) = counted(1, self.many);

                Ok(one)
            }

            false => {
                let Ok(one) = self.expecting();
                let Ok(span) =
                    toward_zero_u16(one.as_secs_f64() / self.whole.as_secs_f64() * 100.0);

                Ok(span.min(WHOLE))
            }
        }
    }

    pub fn finished(&mut self, took: Duration) -> Result<(), Never> {
        let one = self.expecting.pop().unwrap_or_default();

        self.passed = self.passed.saturating_add(one);
        self.took = self.took.saturating_add(took);
        self.done = self.done.saturating_add(1);

        Ok(())
    }
}

pub const WHOLE: u16 = 100;

pub const LEANING: f64 = 1.3;

pub fn leaning(part: f64) -> Result<f64, Never> {
    Ok(part.clamp(0.0, 1.0).powf(LEANING))
}

pub const BY_THE_CLOCK: f64 = 0.9;

pub fn over(gone: Duration, expecting: Duration) -> Result<Option<Duration>, Never> {
    Ok(match expecting.is_zero() || gone <= expecting {
        true => None,
        false => Some(gone),
    })
}

pub fn crept(gone: Duration, expecting: Duration) -> Result<f64, Never> {
    match expecting.is_zero() {
        true => Ok(0.0),
        false => {
            let along = gone.as_secs_f64() / expecting.as_secs_f64();

            Ok(match along < 1.0 {
                true => along * BY_THE_CLOCK,
                false => {
                    let over = along - 1.0;

                    BY_THE_CLOCK + (1.0 - BY_THE_CLOCK) * (over / (over + 1.0))
                }
            })
        }
    }
}

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

    #[test]
    fn the_bar_leans_behind_early_rather_than_stalling_late() {
        assert_eq!(leaning(0.0), Ok(0.0));
        assert_eq!(leaning(1.0), Ok(1.0));

        for part in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let Ok(leaning) = leaning(part);

            assert!(leaning < part, "the bar ran ahead of the run at {part}");
            assert!(leaning > part * 0.5, "the bar fell so far behind it says nothing: {part}");
        }
    }

    #[test]
    fn leaning_behind_still_only_ever_moves_forward() {
        let mut before = 0.0;

        for step in 0..=100 {
            let Ok(leaning) = leaning(f64::from(step) / 100.0);

            assert!(leaning >= before, "the bar went backwards at {step}");
            before = leaning;
        }
    }

    #[test]
    fn a_check_that_has_been_timed_fills_its_own_share_with_the_clock() {
        let one = Duration::from_secs(100);

        assert_eq!(crept(Duration::ZERO, one), Ok(0.0));
        assert_eq!(crept(Duration::from_secs(50), one), Ok(0.45));
        assert_eq!(crept(one, one), Ok(BY_THE_CLOCK));
    }

    #[test]
    fn a_check_running_long_still_moves_and_never_arrives_on_its_own() {
        let one = Duration::from_secs(100);
        let Ok(late) = crept(Duration::from_secs(200), one);
        let Ok(later) = crept(Duration::from_secs(400), one);
        let Ok(much_later) = crept(Duration::from_secs(100_000), one);

        assert!(late > BY_THE_CLOCK, "a check past its estimate stopped moving");
        assert!(later > late, "it stopped moving after that");
        assert!(much_later < 1.0, "the bar said the check had finished when it had not");
    }

    #[test]
    fn a_check_says_so_once_it_has_run_longer_than_it_usually_does() {
        let one = Duration::from_secs(100);

        assert_eq!(over(Duration::from_secs(50), one), Ok(None));
        assert_eq!(over(one, one), Ok(None), "a check bang on its estimate is not yet late");
        assert_eq!(over(Duration::from_secs(101), one), Ok(Some(Duration::from_secs(101))));
        assert_eq!(
            over(Duration::from_secs(101), Duration::ZERO),
            Ok(None),
            "a check nothing has ever timed cannot be late"
        );
    }

    #[test]
    fn a_check_nobody_has_ever_timed_is_not_pretended_to_be_measured() {
        assert_eq!(crept(Duration::from_secs(30), Duration::ZERO), Ok(0.0));
    }

    #[test]
    fn a_checks_share_of_the_bar_is_its_share_of_the_run() {
        let mut lengths = Lengths::default();
        let Ok(()) = lengths.learned(SLOW.name, Duration::from_secs(75));
        let Ok(()) = lengths.learned(QUICK.name, Duration::from_secs(25));

        let Ok(ahead) = Ahead::of(&lengths, &[&SLOW, &QUICK]);

        assert_eq!(ahead.span(), Ok(75));
    }

    #[test]
    fn a_run_nothing_has_ever_timed_gives_every_check_the_same_share() {
        let Ok(ahead) = Ahead::of(&Lengths::default(), &[&SLOW, &QUICK]);

        assert_eq!(ahead.span(), Ok(50));
    }

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

    const SLOW: Check = check("040-slow");
    const QUICK: Check = check("050-quick");
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
        let Ok(left) = ahead.left(Duration::ZERO);

        left
    }

    fn whole(ahead: &Ahead) -> Option<Duration> {
        let Ok(whole) = ahead.whole();

        whole
    }

    fn finished(ahead: &mut Ahead) {
        let Ok(one) = ahead.expecting();
        let Ok(()) = ahead.finished(one);
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
    fn a_machine_nobody_has_timed_is_given_the_table_that_travels() {
        let mut ahead = ahead(&Lengths::default(), &every());

        assert_eq!(percent(&ahead), 0);
        assert!(whole(&ahead).is_some(), "a fresh machine was promised nothing");
        finished(&mut ahead);

        assert!(percent(&ahead) > 0, "a check finished and the bar did not move");
    }

    #[test]
    fn a_machine_with_no_table_at_all_still_counts_checks() {
        let Ok(mut ahead) = Ahead::from(&Lengths::default(), &Lengths::default(), &every());

        assert_eq!(percent(&ahead), 0);
        assert_eq!(left(&ahead), None);
        assert_eq!(whole(&ahead), None);

        let Ok(()) = ahead.finished(Duration::ZERO);

        assert_eq!(percent(&ahead), 33);
    }

    #[test]
    fn what_this_machine_measured_is_never_overruled_by_what_travels() {
        let here = timed(&[("010-one", 4)]);
        let there = timed(&[("010-one", 400), ("020-two", 40)]);

        assert_eq!(expecting(&here, &there, 1.0, "010-one"), Ok(Duration::from_secs(4)));
    }

    #[test]
    fn a_check_this_machine_has_not_timed_is_carried_over_at_this_machines_pace() {
        let here = timed(&[("010-one", 4)]);
        let there = timed(&[("010-one", 8), ("020-two", 40)]);
        let Ok(pace) = pace_of(&here, &there);

        assert_eq!(pace, 0.5, "this machine is twice the speed of the one that was carried");
        assert_eq!(expecting(&here, &there, pace, "020-two"), Ok(Duration::from_secs(20)));
    }

    #[test]
    fn a_machine_with_nothing_in_common_with_the_carried_table_takes_it_as_it_stands() {
        let here = timed(&[("040-slow", 4)]);
        let there = timed(&[("010-one", 8)]);
        let Ok(pace) = pace_of(&here, &there);

        assert_eq!(pace, 1.0);
        assert_eq!(expecting(&here, &there, pace, "010-one"), Ok(Duration::from_secs(8)));
    }

    #[test]
    fn one_wild_check_cannot_send_the_whole_estimate_wild() {
        let here = timed(&[("010-one", 100_000)]);
        let there = timed(&[("010-one", 1)]);
        let Ok(pace) = pace_of(&here, &there);

        assert_eq!(pace, SLOWEST, "a single outlier was believed whole");
    }

    #[test]
    fn a_check_in_neither_table_is_no_more_surprising_than_the_rest() {
        let here = Lengths::default();
        let there = timed(&[("010-one", 8), ("020-two", 40)]);
        let Ok(one) = expecting(&here, &there, 1.0, "999-brand-new");

        assert!(!one.is_zero(), "a new check was given no length at all");
    }

    #[test]
    fn the_strip_moves_by_time_rather_than_by_check() {
        let lengths = timed(&[("010-one", 10), ("020-two", 80), ("030-three", 10)]);
        let ordered = longest_first(&lengths, &every());
        let mut ahead = ahead(&lengths, &ordered);

        assert_eq!(whole(&ahead), Some(Duration::from_secs(100)));
        finished(&mut ahead);

        assert_eq!(
            percent(&ahead),
            74,
            "the long one is most of the run and the strip says so, leaning behind"
        );
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
