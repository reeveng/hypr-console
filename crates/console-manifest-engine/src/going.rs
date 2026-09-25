//! How far along an apply is.
//!
//! An apply on the device is minutes, most of it silent: pacman says nothing
//! useful, `cargo build --release` says nothing at all until it is finished,
//! and what someone watching over ssh gets is a cursor. The question they are
//! actually asking is not "what is it doing" -- the lines already say that --
//! but "how much longer", and nothing here could answer it.
//!
//! # Two readers, two shapes
//!
//! The strip under the bar on the device wants one number for the whole apply,
//! because it is one row of pixels and there is nowhere to put a second. It is
//! counted in thousandths of the whole, which is the strip's own unit and about
//! a point of fill: the build is most of an apply and most of a screen, and a
//! stage that can only move the number a hundredth at a time leaves it still
//! for whole minutes of it. The
//! person at the terminal wants the opposite: what is happening right now, and
//! whether it is still happening. So the same walk feeds both, and they are
//! drawn differently.
//!
//! The terminal is drawn the way pacman draws, because pacman is the thing
//! everyone on this machine already reads while they wait, and it is the one
//! that never leaves you wondering. Its shape, off its own format strings --
//! `(%*zu/%*zu) %ls%-*s` then `[%s]` then ` %3d%%` -- is a line per item:
//! counters padded to the width of the total so nothing jitters as they climb,
//! the name left, the bar right, redrawn over itself with a carriage return
//! while it fills and ended with a newline at a hundred so the finished line
//! stays on the screen as history. A run is then a list of things that got
//! done, with one line at the bottom still moving, and the thing a person is
//! actually watching for -- movement -- is on the line their eye is already on.
//!
//! Fixed widths, where pacman measures the terminal. Pacman has to: a package
//! name plus a version can be most of a line and it has no idea in advance. The
//! stages here are named in this file and the longest of them is known, so a
//! column wide enough for all of them is arithmetic rather than an ioctl, and
//! the whole line fits eighty.
//!
//! # Why the strip's number speeds up
//!
//! The stages are not equal and are nowhere near equal. Compiling every
//! program on the machine is most of an apply; writing sixty files and
//! restarting a dozen services is the rest; and the tail -- swapping the
//! release in, packing the add-on, writing two profiles -- is renames and a
//! zip, which is under a second all together.
//!
//! So the strip is weighted rather than counted. A bar that moved a
//! fourteenth per stage would sit at 8% through the minutes of the build and
//! then jump to the end, which is a bar that lies twice. Weighted, it crawls at
//! the beginning, where the time actually is, and runs at the end, where there
//! is nothing left to wait for. That is not a trick played on the reader: it is
//! what an apply does.
//!
//! The order it happens in is not a choice. Packages have to be installed
//! before what needs them is built, what is built has to exist before it is
//! staged, and nothing is swapped in before all of it is. It happens that this
//! is also longest-first, which is why the weights come out front-loaded
//! without anything being arranged.
//!
//! # Inside a stage
//!
//! A stage that is a loop over things says which thing it is on, and the
//! line fills as it goes: `during` hands the work a `Moving`, which takes
//! either a count out of a total or a bare fraction for the one stage that
//! has no honest total to count toward. Everything a stage would have
//! printed goes through `Moving::say`, which wipes the line, prints, and draws
//! it again underneath -- so the log scrolls past above a bar that stays put,
//! which is the arrangement pacman gets by ending each line when it is full.
//!
//! # Where the numbers come from
//!
//! Estimates, and said to be. `CONSOLE_TIMINGS=1 console apply` prints what
//! each stage actually took on the machine in front of you, and
//! `the_weights_add_up` is the only thing that has to stay true when they are
//! corrected. `packages` is the one that cannot be estimated honestly: it is
//! nothing on almost every apply and minutes on the one after someone adds a
//! package, so it is given a small weight and the bar jumps when it is not. The
//! wallpapers are the same shape for the same reason -- nothing at all unless
//! the table has a picture this machine has not pressed, and then a fetch and
//! a minute of one core for each of them -- and are given a small weight on the
//! same argument.


use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_u16};
use console_how_far::{Bar, Progress, Now};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reachability {
    start: u16,
    weight: u16,
}

use console_notifications::updating;

use crate::confirmation;

pub const READING: &str = "reading packages";
pub const WANTED: &str = "reading wanted";
pub const PACKAGES: &str = "installing";
pub const KEEPING: &str = "keeping";
pub const SWEEPING: &str = "sweeping";
pub const BUILDING: &str = "building";
pub const FILES: &str = "writing files";
pub const SWAPPING: &str = "swapping in";
pub const ADD_ON: &str = "the add-on";
pub const BROWSERS: &str = "the browsers";
pub const PROFILES: &str = "the profiles";
pub const SCREEN: &str = "the screen";
pub const WALLPAPERS: &str = "the wallpapers";
pub const SERVICES: &str = "services";
pub const RELEASE: &str = "keeping the release";

#[derive(Debug, Clone, Copy)]
pub struct Stage {
    pub label: &'static str,
    pub weight: u16,
}

pub const WHOLE: u16 = updating::WHOLE;

pub const STAGES: [Stage; 15] = [
    Stage { label: READING, weight: 10 },
    Stage { label: WANTED, weight: 10 },
    Stage { label: PACKAGES, weight: 60 },
    Stage { label: KEEPING, weight: 10 },
    Stage { label: SWEEPING, weight: 10 },
    Stage { label: BUILDING, weight: 590 },
    Stage { label: FILES, weight: 80 },
    Stage { label: SWAPPING, weight: 10 },
    Stage { label: ADD_ON, weight: 20 },
    Stage { label: BROWSERS, weight: 20 },
    Stage { label: PROFILES, weight: 30 },
    Stage { label: SCREEN, weight: 10 },
    Stage { label: WALLPAPERS, weight: 20 },
    Stage { label: SERVICES, weight: 100 },
    Stage { label: RELEASE, weight: 20 },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Bar,
    #[cfg(test)]
    NoOne,
}

fn tell(permille: u16, label: &str) -> Result<(), Never> {
    let Ok(()) = updating::wrote(&updating::Progress { permille, label: label.to_string() });

    Ok(())
}

#[derive(Debug)]
pub struct Going {
    done: u16,
    bar: Bar,
    told: Audience,
    took: Vec<(&'static str, Duration)>,
}

pub struct Moving<'a> {
    going: &'a mut Going,
    label: &'static str,
    start: u16,
    weight: u16,
}

impl Moving<'_> {
    pub fn far(&mut self, far: f64, now: &str) -> Result<(), Never> {
        self.going.inside(self.label, Reachability { start: self.start, weight: self.weight }, far, Now(now))
    }

    pub fn at(&mut self, far: Progress, now: &str) -> Result<(), Never> {
        let Ok(part) = console_how_far::fraction(far);
        let Ok(counted) = console_how_far::counted(far);

        self.far(part, &format!("{counted} {now}"))
    }

    pub fn say(&mut self, line: &str) -> Result<(), Never> {
        self.going.say(line)
    }
}

impl Going {
    pub fn starting() -> Result<Self, Never> {
        let Ok(many) = fitted::<_, u32>(STAGES.len());
        let Ok(bar) = Bar::of(many);

        Ok(Going { done: 0, bar, told: Audience::Bar, took: Vec::new() })
    }

    #[cfg(test)]
    pub fn quiet() -> Self {
        let Ok(many) = fitted::<_, u32>(STAGES.len());
        let Ok(bar) = Bar::unwatched(many);

        Going { done: 0, bar, told: Audience::NoOne, took: Vec::new() }
    }

    pub fn through<T>(&mut self, label: &'static str, work: impl FnOnce() -> T) -> Result<T, Never> {
        self.through_handed(label, &mut (), |_nothing| work())
    }

    pub fn through_handed<M, T>(
        &mut self,
        label: &'static str,
        handed: &mut M,
        work: impl FnOnce(&mut M) -> T,
    ) -> Result<T, Never> {
        let Ok(()) = self.bar.on(label);
        let Ok(started) = confirmation::started();
        let done = work(handed);
        let Ok(took) = confirmation::ended(label, started);

        self.took.push((label, took));

        let Ok(()) = self.arrived(label);

        Ok(done)
    }

    pub fn during<T>(
        &mut self,
        label: &'static str,
        work: impl FnOnce(&mut Moving) -> T,
    ) -> Result<T, Never> {
        self.during_handed(label, &mut (), |_nothing, moving| work(moving))
    }

    pub fn during_handed<M, T>(
        &mut self,
        label: &'static str,
        handed: &mut M,
        work: impl FnOnce(&mut M, &mut Moving) -> T,
    ) -> Result<T, Never> {
        let Ok(()) = self.bar.on(label);

        let start = self.done;
        let Ok(weight) = weight_of(label);
        let Ok(started) = confirmation::started();

        let done = {
            let mut moving = Moving { going: &mut *self, label, start, weight };

            work(handed, &mut moving)
        };

        let Ok(took) = confirmation::ended(label, started);

        self.took.push((label, took));
        self.done = start;

        let Ok(()) = self.arrived(label);

        Ok(done)
    }

    fn inside(
        &mut self,
        label: &str,
        reach: Reachability,
        far: f64,
        now: Now<'_>,
    ) -> Result<(), Never> {
        let far = far.clamp(0.0, 1.0);
        let Ok(into) = console_how_far::percent(far);
        let Ok(inside) = toward_zero_u16(f64::from(reach.weight) * far);
        let reached = reach.start.saturating_add(inside).min(WHOLE);

        let Ok(()) = self.bar.filling(into, now.0);

        match reached <= self.done {
            true => return Ok(()),
            false => {},
        }

        self.done = reached;

        let Ok(far) = self.far();
        let Ok(said) = console_how_far::caption(label, now);

        match self.told == Audience::Bar {
            true => {
                let Ok(()) = tell(far, &said);
            }
            false => {},
        }

        Ok(())
    }

    pub fn arrived(&mut self, label: &'static str) -> Result<(), Never> {
        let Ok(weight) = weight_of(label);

        self.done = self.done.saturating_add(weight).min(WHOLE);

        let Ok(()) = self.bar.stays(label);
        let Ok(far) = self.far();

        match self.told == Audience::Bar {
            true => {
                let Ok(()) = tell(far, label);
            }
            false => {},
        }

        Ok(())
    }

    pub fn say(&self, line: &str) -> Result<(), Never> {
        self.bar.say(line)
    }

    pub fn far(&self) -> Result<u16, Never> {
        Ok(self.done)
    }

    pub fn done(mut self) -> Result<Vec<(&'static str, Duration)>, Never> {
        self.done = WHOLE;

        let Ok(()) = self.bar.wiped();

        match self.told == Audience::Bar {
            true => {
                let Ok(()) = updating::done();
            }
            false => {},
        }

        Ok(self.took)
    }
}

fn weight_of(label: &str) -> Result<u16, Never> {
    Ok(STAGES
        .iter()
        .find(|stage| stage.label == label)
        .map_or(0, |stage| stage.weight))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn far(going: &Going) -> u16 {
        let Ok(far) = going.far();

        far
    }

    fn line(going: &Going) -> String {
        let Ok(line) = going.bar.line();

        line
    }

    #[test]
    fn a_stage_that_says_how_far_it_has_got_moves_the_bar_and_still_lands_where_it_should() {
        let mut going = Going::quiet();
        let Ok(()) = going.through(READING, || ());
        let Ok(()) = going.through(WANTED, || ());
        let Ok(()) = going.through(PACKAGES, || ());
        let Ok(()) = going.through(KEEPING, || ());
        let Ok(()) = going.through(SWEEPING, || ());
        let before = far(&going);
        let mut seen = Vec::new();
        let Ok(()) = going.during(BUILDING, |moving| {
            for far in [0.1, 0.5, 0.9] {
                let Ok(()) = moving.far(far, "a-crate");

                seen.push(0);
            }
        });
        let Ok(weight) = weight_of(BUILDING);

        assert_eq!(seen.len(), 3);
        assert_eq!(far(&going), before + weight);
    }

    #[test]
    fn what_a_stage_says_on_the_way_stays_inside_its_own_weight() {
        let mut going = Going::quiet();
        let Ok(()) = going.inside(BUILDING, Reachability { start: 10, weight: 60 }, 0.5, Now(""));
        assert_eq!(far(&going), 40);
        let Ok(()) = going.inside(BUILDING, Reachability { start: 10, weight: 60 }, 2.0, Now(""));
        assert_eq!(far(&going), 70, "a stage reported past its end went past it");
    }

    #[test]
    fn a_stage_that_says_it_has_gone_backwards_moves_nothing() {
        let mut going = Going::quiet();
        let Ok(()) = going.inside(BUILDING, Reachability { start: 10, weight: 60 }, 0.5, Now(""));
        let Ok(()) = going.inside(BUILDING, Reachability { start: 10, weight: 60 }, 0.1, Now(""));
        let Ok(()) = going.inside(BUILDING, Reachability { start: 10, weight: 60 }, -1.0, Now(""));
        assert_eq!(far(&going), 40);
    }

    #[test]
    fn a_count_out_of_a_total_names_the_thing_and_says_how_many_are_left() {
        let mut going = Going::quiet();
        let mut said = String::new();
        let Ok(()) = going.during(FILES, |moving| {
            let Ok(()) = moving.at(Progress { done: 1, many: 4 }, "console/palette.css");
            let Ok(drawn) = moving.going.bar.line();

            said = drawn;
        });

        assert!(said.contains(" 25%"), "{said}");
        assert!(said.contains("(1/4) console/palette.css"), "{said}");
    }

    #[test]
    fn the_line_fills_as_the_count_climbs_and_never_runs_backwards() {
        let mut going = Going::quiet();
        let mut seen: Vec<String> = Vec::new();
        let Ok(()) = going.during(BUILDING, |moving| {
            for done in [0_u32, 1, 2, 1, 4] {
                let Ok(()) = moving.at(Progress { done, many: 4 }, "console-panel");
                let Ok(drawn) = moving.going.bar.line();

                seen.push(drawn);
            }
        });
        let filled: Vec<u32> = seen
            .iter()
            .map(|line| u32::try_from(line.chars().filter(|one| *one == '#').count()).unwrap())
            .collect();
        let mut sorted = filled.clone();

        sorted.sort_unstable();

        assert_eq!(filled, sorted, "the line went backwards: {filled:?}");
        assert!(filled.iter().any(|full| *full > 0), "the line never filled: {filled:?}");
    }

    #[test]
    fn the_line_is_shaped_the_way_pacman_shapes_one() {
        let mut going = Going::quiet();
        let Ok(()) = going.through(READING, || ());
        let said = line(&going);

        assert!(said.starts_with(&format!("( 1/{}) ", STAGES.len())), "{said}");
        assert!(said.ends_with("] 100%"), "{said}");
    }

    #[test]
    fn the_weights_add_up() {
        let all: u16 = STAGES.iter().map(|stage| stage.weight).sum();
        assert_eq!(all, WHOLE, "the weights come to {all} rather than {WHOLE}");
    }

    #[test]
    fn no_stage_is_named_twice() {
        let mut seen = BTreeSet::new();
        let twice: Vec<&str> = STAGES
            .iter()
            .map(|stage| stage.label)
            .filter(|label| !seen.insert(*label))
            .collect();

        assert!(twice.is_empty(), "{twice:?} is in the table more than once");
    }

    #[test]
    fn the_first_half_of_the_stages_is_most_of_the_work() {
        let front: u16 = STAGES.iter().take(STAGES.len() / 2).map(|stage| stage.weight).sum();
        assert!(
            front > WHOLE / 2,
            "the first {} stages are only {front} of {WHOLE}, so the bar would run at \
             the start and crawl at the end",
            STAGES.len() / 2
        );
    }

    #[test]
    fn walking_all_of_them_arrives() {
        let mut going = Going::quiet();
        for stage in STAGES {
            let Ok(()) = going.through(stage.label, || ());
        }
        assert_eq!(far(&going), WHOLE);
    }

    #[test]
    fn every_stage_walked_is_counted_once() {
        let mut going = Going::quiet();
        for stage in STAGES {
            let Ok(()) = going.through(stage.label, || ());
        }
        assert!(line(&going).starts_with(&format!("({0}/{0})", STAGES.len())), "{}", line(&going));
    }

    #[test]
    fn a_skipped_stage_still_ends_at_the_end() {
        let mut going = Going::quiet();
        for stage in STAGES.iter().filter(|stage| stage.label != PACKAGES) {
            let Ok(()) = going.through(stage.label, || ());
        }
        assert!(far(&going) < WHOLE, "nothing was skipped");

        let Ok(_) = going.done();
    }

    #[test]
    fn a_stage_says_what_it_would_have_said_without_a_bar() {
        let mut going = Going::quiet();
        let Ok(seven) = going.through(BUILDING, || 7);
        let Ok(said) = going.through(FILES, || Err::<(), String>("would not".to_string()));

        assert_eq!(seven, 7);
        assert_eq!(said, Err("would not".to_string()));

        let mut ran = 0;
        let Ok(()) = going.through(SERVICES, || ran += 1);

        assert_eq!(ran, 1, "the work did not run exactly once");
    }

    #[test]
    fn a_stage_no_one_weighed_does_not_move_it() {
        let mut going = Going::quiet();
        let Ok(()) = going.through(BUILDING, || ());
        let before = far(&going);
        let Ok(()) = going.through("something no one put in the table", || ());

        assert_eq!(far(&going), before);
    }

    #[test]
    fn it_does_not_go_past_the_end() {
        let mut going = Going::quiet();
        for _ in 0..4 {
            for stage in STAGES {
                let Ok(()) = going.arrived(stage.label);
            }
        }
        assert_eq!(far(&going), WHOLE);
    }
}
