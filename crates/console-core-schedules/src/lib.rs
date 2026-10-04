//! How long to wait before the next try, and whether there is one.
//!
//! Two crates had answered this, each in its own words. `console-waiting`
//! asked a question every fifty milliseconds until a patience ran out, and
//! called the pair a `Schedule`. `console-core-reconnect` waited a second
//! before making a subscription again, twice as long after every try that
//! fell straight over, never longer than a minute, and from a second again
//! once a try had stood -- and called that `Between`, with two free functions
//! beside it that said the same thing a second time. Both are one question,
//! how long until the next and is there a next, and two answers to one
//! question are two answers waiting to disagree.
//!
//! effect's `Schedule` is that question as a value, and its words are taken
//! here: a schedule is `spaced` or `exponential`, lasts `up_to` a span or
//! forever, and may `reset_after` a quiet stretch. What effect builds from
//! `union` and `intersect` this keeps as three fields, because every schedule
//! the tree has is one spacing, one lasting and one reset, and a
//! combinator nobody calls is a second way to say the same schedule.
//!
//! A schedule is a value and decides nothing on its own. Its `driver` is the
//! machine that steps it: handed how long it has been since the first try
//! began and since this one did, it answers `Continue` with the gap to stop
//! for, or `Finished`. It reads no clock and stops no thread -- whoever runs it does
//! both, which is why a schedule can be asked what it would do over an hour
//! of tries in a test that takes none of the hour.
//!
//! `delays` is that driver run as if every try took no time at all: the gaps
//! the schedule would give, in order, until it finishes. It is for a caller
//! that counts its patience in gaps rather than on a clock, which is what the
//! checks on the handheld have always done -- a question over ssh takes as
//! long as it takes, and four seconds of patience there has meant eight
//! half-second gaps however slow the answers were.

use console_core_never::Never;
use std::time::Duration;

pub const BETWEEN: Duration = Duration::from_millis(50);

const GROWTH: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
    Spaced(Duration),
    Exponential { first: Duration, longest: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lasting {
    Forever,
    UpTo(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reset {
    Never,
    After(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub spacing: Spacing,
    pub lasting: Lasting,
    pub reset: Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed {
    pub total: Duration,
    pub this_try: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Continue(Duration),
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Driver {
    schedule: Schedule,
    gap: Duration,
}

impl Schedule {
    pub fn spaced(between: Duration) -> Result<Schedule, Never> {
        Ok(Schedule { spacing: Spacing::Spaced(between), lasting: Lasting::Forever, reset: Reset::Never })
    }

    pub fn exponential(first: Duration, longest: Duration) -> Result<Schedule, Never> {
        Ok(Schedule {
            spacing: Spacing::Exponential { first, longest },
            lasting: Lasting::Forever,
            reset: Reset::Never,
        })
    }

    pub fn of(until: Duration) -> Result<Schedule, Never> {
        Schedule::asking_every(until, BETWEEN)
    }

    pub fn asking_every(until: Duration, between: Duration) -> Result<Schedule, Never> {
        let Ok(spaced) = Schedule::spaced(between);

        spaced.up_to(until)
    }

    pub fn up_to(self, until: Duration) -> Result<Schedule, Never> {
        Ok(Schedule { lasting: Lasting::UpTo(until), ..self })
    }

    pub fn reset_after(self, quiet: Duration) -> Result<Schedule, Never> {
        Ok(Schedule { reset: Reset::After(quiet), ..self })
    }

    pub fn driver(self) -> Result<Driver, Never> {
        let Ok(gap) = first(self.spacing);

        Ok(Driver { schedule: self, gap })
    }

    pub fn delays(self) -> Result<impl Iterator<Item = Duration>, Never> {
        let Ok(driver) = self.driver();

        Ok(std::iter::repeat(()).scan((driver, Duration::ZERO), |(driver, total), ()| {
            let Ok(decided) = driver.next(Elapsed { total: *total, this_try: Duration::ZERO });

            match decided {
                Decision::Continue(gap) => {
                    *total = total.saturating_add(gap);

                    Some(gap)
                }
                Decision::Finished => None,
            }
        }))
    }
}

impl Driver {
    pub fn next(&mut self, elapsed: Elapsed) -> Result<Decision, Never> {
        let Ok(gap) = self.after_quiet(elapsed.this_try);
        let lasted = match self.schedule.lasting {
            Lasting::Forever => Lasted::Within,
            Lasting::UpTo(until) => match elapsed.total >= until {
                true => Lasted::Over,
                false => Lasted::Within,
            },
        };

        Ok(match lasted {
            Lasted::Over => Decision::Finished,
            Lasted::Within => {
                let Ok(grown) = grown(self.schedule.spacing, gap);

                self.gap = grown;

                Decision::Continue(gap)
            }
        })
    }

    fn after_quiet(&self, this_try: Duration) -> Result<Duration, Never> {
        Ok(match self.schedule.reset {
            Reset::Never => self.gap,
            Reset::After(quiet) => match this_try >= quiet {
                true => {
                    let Ok(again) = first(self.schedule.spacing);

                    again
                }
                false => self.gap,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lasted {
    Within,
    Over,
}

fn first(spacing: Spacing) -> Result<Duration, Never> {
    Ok(match spacing {
        Spacing::Spaced(between) => between,
        Spacing::Exponential { first, longest: _ } => first,
    })
}

fn grown(spacing: Spacing, gap: Duration) -> Result<Duration, Never> {
    Ok(match spacing {
        Spacing::Spaced(between) => between,
        Spacing::Exponential { first: _, longest } => gap.saturating_mul(GROWTH).min(longest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_TIME: Elapsed = Elapsed { total: Duration::ZERO, this_try: Duration::ZERO };

    fn at(total: Duration) -> Result<Elapsed, Never> {
        Ok(Elapsed { total, this_try: Duration::ZERO })
    }

    fn gaps(driver: &mut Driver, tries: u32) -> Result<Vec<Decision>, Never> {
        let mut decided = Vec::new();

        for _ in 0..tries {
            let Ok(next) = driver.next(NO_TIME);

            decided.push(next);
        }

        Ok(decided)
    }

    #[test]
    fn a_spaced_schedule_gives_the_same_gap_every_time() {
        let Ok(spaced) = Schedule::spaced(Duration::from_millis(500));
        let Ok(mut driver) = spaced.driver();

        let Ok(three) = gaps(&mut driver, 3);

        assert_eq!(three, vec![Decision::Continue(Duration::from_millis(500)); 3]);
    }

    #[test]
    fn a_schedule_up_to_a_span_finishes_once_the_span_has_gone() {
        let Ok(patience) = Schedule::asking_every(Duration::from_secs(1), Duration::from_millis(500));
        let Ok(mut driver) = patience.driver();
        let Ok(before) = at(Duration::from_millis(999));
        let Ok(after) = at(Duration::from_secs(1));
        let Ok(early) = driver.next(before);
        let Ok(late) = driver.next(after);

        assert_eq!(early, Decision::Continue(Duration::from_millis(500)));
        assert_eq!(late, Decision::Finished);
    }

    #[test]
    fn a_patience_of_nothing_finishes_at_the_first_decision() {
        let Ok(patience) = Schedule::of(Duration::ZERO);
        let Ok(mut driver) = patience.driver();
        let Ok(decided) = driver.next(NO_TIME);

        assert_eq!(decided, Decision::Finished);
    }

    #[test]
    fn a_patience_says_how_often_as_well_as_how_long() {
        let Ok(plain) = Schedule::of(Duration::from_secs(3));
        let Ok(often) = Schedule::asking_every(Duration::from_secs(3), Duration::from_millis(5));

        assert_eq!(plain.spacing, Spacing::Spaced(BETWEEN));
        assert_eq!(often.spacing, Spacing::Spaced(Duration::from_millis(5)));
        assert_eq!(plain.lasting, often.lasting);
    }

    #[test]
    fn an_exponential_schedule_doubles_and_never_grows_past_the_longest() {
        let Ok(growing) = Schedule::exponential(Duration::from_secs(1), Duration::from_secs(60));
        let Ok(mut driver) = growing.driver();
        let seconds: Vec<Decision> = [1, 2, 4, 8, 16, 32, 60, 60]
            .into_iter()
            .map(|seconds| Decision::Continue(Duration::from_secs(seconds)))
            .collect();

        let Ok(eight) = gaps(&mut driver, 8);

        assert_eq!(eight, seconds);
    }

    #[test]
    fn a_try_that_stood_starts_the_growing_over() {
        let Ok(growing) = Schedule::exponential(Duration::from_secs(1), Duration::from_secs(60));
        let Ok(reset) = growing.reset_after(Duration::from_secs(5));
        let Ok(mut driver) = reset.driver();
        let Ok(_grown) = gaps(&mut driver, 5);
        let Ok(fell_over) = driver.next(Elapsed { total: Duration::ZERO, this_try: Duration::from_millis(4_999) });
        let Ok(stood) = driver.next(Elapsed { total: Duration::ZERO, this_try: Duration::from_secs(5) });
        let Ok(after_it) = driver.next(NO_TIME);

        assert_eq!(fell_over, Decision::Continue(Duration::from_secs(32)), "a try that fell over keeps growing the wait");
        assert_eq!(stood, Decision::Continue(Duration::from_secs(1)));
        assert_eq!(after_it, Decision::Continue(Duration::from_secs(2)));
    }

    #[test]
    fn a_schedule_that_never_resets_ignores_how_long_a_try_stood() {
        let Ok(growing) = Schedule::exponential(Duration::from_secs(1), Duration::from_secs(60));
        let Ok(mut driver) = growing.driver();
        let Ok(_grown) = gaps(&mut driver, 2);
        let Ok(next) = driver.next(Elapsed { total: Duration::ZERO, this_try: Duration::from_secs(600) });

        assert_eq!(next, Decision::Continue(Duration::from_secs(4)));
    }

    #[test]
    fn a_patience_counted_in_gaps_is_as_many_gaps_as_fit_in_it() {
        let half = Duration::from_millis(500);
        let Ok(four) = Schedule::asking_every(Duration::from_secs(4), half);
        let Ok(gaps) = four.delays();

        assert_eq!(gaps.collect::<Vec<Duration>>(), vec![half; 8]);
    }

    #[test]
    fn a_patience_that_is_not_a_whole_number_of_gaps_is_given_the_one_that_crosses_it() {
        let Ok(uneven) = Schedule::asking_every(Duration::from_millis(1_200), Duration::from_millis(500));
        let Ok(gaps) = uneven.delays();

        assert_eq!(gaps.count(), 3);
    }

    #[test]
    fn the_delays_of_a_growing_schedule_are_its_gaps() {
        let Ok(growing) = Schedule::exponential(Duration::from_secs(1), Duration::from_secs(4));
        let Ok(gaps) = growing.delays();
        let seconds: Vec<Duration> = [1, 2, 4, 4].into_iter().map(Duration::from_secs).collect();

        assert_eq!(gaps.take(4).collect::<Vec<Duration>>(), seconds);
    }

    #[test]
    fn a_forever_schedule_never_finishes() {
        let Ok(spaced) = Schedule::spaced(Duration::from_secs(1));
        let Ok(mut driver) = spaced.driver();
        let Ok(longest) = at(Duration::MAX);
        let Ok(decided) = driver.next(longest);

        assert_eq!(decided, Decision::Continue(Duration::from_secs(1)));
    }
}
