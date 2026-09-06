//! How far along an apply is.
//!
//! An apply on the device is minutes, most of it silent: pacman says nothing
//! useful, `cargo build --release` says nothing at all until it is finished,
//! and what somebody watching over ssh gets is a cursor. The question they are
//! actually asking is not "what is it doing" -- the lines already say that --
//! but "how much longer", and nothing here could answer it.
//!
//! # Why the bar speeds up
//!
//! The stretches are not equal and are nowhere near equal. Compiling every
//! program on the machine is most of an apply; writing sixty files and
//! restarting a dozen services is the rest; and the tail -- swapping the
//! release in, packing the add-on, writing two profiles -- is renames and a
//! zip, which is under a second all together.
//!
//! So the bar is weighted rather than counted. A bar that moved a thirteenth
//! per stretch would sit at 8% through the minutes of the build and then jump
//! to the end, which is a bar that lies twice. Weighted, it crawls at the
//! beginning, where the time actually is, and runs at the end, where there is
//! nothing left to wait for. That is not a trick played on the reader: it is
//! what an apply does.
//!
//! The order it happens in is not a choice. Packages have to be installed
//! before what needs them is built, what is built has to exist before it is
//! staged, and nothing is swapped in before all of it is. It happens that this
//! is also longest-first, which is why the weights come out front-loaded
//! without anything being arranged.
//!
//! # Where the numbers come from
//!
//! Estimates, and said to be. `CONSOLE_TIMINGS=1 console apply` prints what
//! each stretch actually took on the machine in front of you, and
//! `the_shares_add_up` is the only thing that has to stay true when they are
//! corrected. `packages` is the one that cannot be estimated honestly: it is
//! nothing on almost every apply and minutes on the one after somebody adds a
//! package, so it is given a small share and the bar jumps when it is not. The
//! wallpapers are the same shape for the same reason -- nothing at all unless
//! the table has a picture this machine has not pressed, and then a fetch and
//! a minute of one core for each of them -- and are given a small share on the
//! same argument.


use console_number_conversion::toward_zero_u16;
use std::io::IsTerminal;
use std::io::Write;

use console_never::Never;
use console_notifications::updating;

use crate::went;

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
pub const WALLPAPERS: &str = "the wallpapers";
pub const SERVICES: &str = "services";
pub const RELEASE: &str = "keeping the release";

#[derive(Debug, Clone, Copy)]
pub struct Stretch {
    pub doing: &'static str,
    pub share: u16,
}

pub const WHOLE: u16 = 100;

pub const STRETCHES: [Stretch; 14] = [
    Stretch { doing: READING, share: 1 },
    Stretch { doing: WANTED, share: 1 },
    Stretch { doing: PACKAGES, share: 6 },
    Stretch { doing: KEEPING, share: 1 },
    Stretch { doing: SWEEPING, share: 1 },
    Stretch { doing: BUILDING, share: 60 },
    Stretch { doing: FILES, share: 8 },
    Stretch { doing: SWAPPING, share: 1 },
    Stretch { doing: ADD_ON, share: 2 },
    Stretch { doing: BROWSERS, share: 2 },
    Stretch { doing: PROFILES, share: 3 },
    Stretch { doing: WALLPAPERS, share: 2 },
    Stretch { doing: SERVICES, share: 10 },
    Stretch { doing: RELEASE, share: 2 },
];

const CELLS: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawn {
    Bar,
    Lines,
}

pub fn drawn() -> Result<Drawn, Never> {
    Ok(match std::io::stderr().is_terminal() {
        true => Drawn::Bar,
        false => Drawn::Lines,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told {
    Bar,
    #[cfg(test)]
    Nobody,
}

fn tell(percent: u16, doing: &str) -> Result<(), Never> {
    let Ok(()) = updating::wrote(&updating::Far { percent, doing: doing.to_string() });
    let Ok(()) = updating::wake();

    Ok(())
}

#[derive(Debug)]
pub struct Going {
    done: u16,
    drawn: Drawn,
    told: Told,
}

impl Going {
    pub fn starting() -> Result<Self, Never> {
        let Ok(drawn) = drawn();

        Ok(Going { done: 0, drawn, told: Told::Bar })
    }

    #[cfg(test)]
    pub fn drawing(drawn: Drawn) -> Self {
        Going { done: 0, drawn, told: Told::Nobody }
    }

    pub fn through<T>(&mut self, doing: &'static str, work: impl FnOnce() -> T) -> Result<T, Never> {
        let Ok(done) = went::to(doing, work);
        let Ok(()) = self.arrived(doing);

        Ok(done)
    }

    pub fn during<T>(
        &mut self,
        doing: &'static str,
        work: impl FnOnce(&mut dyn FnMut(f64)) -> T,
    ) -> Result<T, Never> {
        let start = self.done;
        let Ok(share) = share_of(doing);
        let done = {
            let going = &mut *self;
            let mut moved = |far: f64| {
                let Ok(()) = going.inside(doing, start, share, far);
            };
            let Ok(done) = went::to(doing, || work(&mut moved));

            done
        };
        self.done = start;

        let Ok(()) = self.arrived(doing);

        Ok(done)
    }

    fn inside(&mut self, doing: &str, start: u16, share: u16, far: f64) -> Result<(), Never> {
        let far = far.clamp(0.0, 1.0);
        let Ok(inside) = toward_zero_u16(f64::from(share) * far);
        let now = start.saturating_add(inside).min(WHOLE);

        match now <= self.done {
            true => return Ok(()),
            false => {},
        }

        self.done = now;

        let Ok(far) = self.far();

        match self.told == Told::Bar {
            true => {
                let Ok(()) = tell(far, doing);
            }
            false => {},
        }

        Ok(())
    }

    pub fn arrived(&mut self, doing: &'static str) -> Result<(), Never> {
        let Ok(share) = share_of(doing);

        self.done = self.done.saturating_add(share).min(WHOLE);

        let Ok(()) = self.draw(doing);
        let Ok(far) = self.far();

        match self.told == Told::Bar {
            true => {
                let Ok(()) = tell(far, doing);
            }
            false => {},
        }

        Ok(())
    }

    pub fn far(&self) -> Result<u16, Never> {
        Ok(self.done)
    }

    pub fn done(mut self) -> Result<(), Never> {
        self.done = WHOLE;

        let Ok(()) = self.draw("done");

        match self.drawn == Drawn::Bar {
            true => {
                let _ = writeln!(std::io::stderr());
            }
            false => {},
        }

        match self.told == Told::Bar {
            true => {
                let Ok(()) = updating::done();
                let Ok(()) = updating::wake();
            }
            false => {},
        }

        Ok(())
    }

    fn draw(&self, doing: &str) -> Result<(), Never> {
        let Ok(far) = self.far();
        let full = usize::from(far.saturating_mul(CELLS).saturating_div(WHOLE));
        let empty = usize::from(CELLS).saturating_sub(full);
        let mut out = std::io::stderr();
        let _ = match self.drawn {
            Drawn::Bar => write!(
                out,
                "\r  [{}{}] {:>3}%  {:<20}",
                "#".repeat(full),
                "-".repeat(empty),
                far,
                doing
            ),
            Drawn::Lines => writeln!(out, "  {far:>3}%  {doing}"),
        };
        let _ = out.flush();

        Ok(())
    }
}

fn share_of(doing: &str) -> Result<u16, Never> {
    Ok(STRETCHES
        .iter()
        .find(|stretch| stretch.doing == doing)
        .map_or(0, |stretch| stretch.share))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn far(going: &Going) -> u16 {
        let Ok(far) = going.far();

        far
    }

    #[test]
    fn a_stretch_that_says_how_far_it_has_got_moves_the_bar_and_still_lands_where_it_should() {
        let mut going = Going::drawing(Drawn::Lines);
        let Ok(()) = going.arrived(READING);
        let Ok(()) = going.arrived(WANTED);
        let Ok(()) = going.arrived(PACKAGES);
        let Ok(()) = going.arrived(KEEPING);
        let Ok(()) = going.arrived(SWEEPING);
        let before = far(&going);
        let mut seen = Vec::new();
        let Ok(()) = going.during(BUILDING, |moved| {
            for far in [0.1, 0.5, 0.9] {
                moved(far);
                seen.push(0);
            }
        });
        let Ok(share) = share_of(BUILDING);

        assert_eq!(seen.len(), 3);
        assert_eq!(far(&going), before + share);
    }

    #[test]
    fn what_a_stretch_says_on_the_way_stays_inside_its_own_share() {
        let mut going = Going::drawing(Drawn::Lines);
        let Ok(()) = going.inside(BUILDING, 10, 60, 0.5);
        assert_eq!(far(&going), 40);
        let Ok(()) = going.inside(BUILDING, 10, 60, 2.0);
        assert_eq!(far(&going), 70, "a stretch reported past its end went past it");
    }

    #[test]
    fn a_stretch_that_says_it_has_gone_backwards_moves_nothing() {
        let mut going = Going::drawing(Drawn::Lines);
        let Ok(()) = going.inside(BUILDING, 10, 60, 0.5);
        let Ok(()) = going.inside(BUILDING, 10, 60, 0.1);
        let Ok(()) = going.inside(BUILDING, 10, 60, -1.0);
        assert_eq!(far(&going), 40);
    }

    #[test]
    fn the_shares_add_up() {
        let all: u16 = STRETCHES.iter().map(|stretch| stretch.share).sum();
        assert_eq!(all, WHOLE, "the shares come to {all} rather than {WHOLE}");
    }

    #[test]
    fn no_stretch_is_named_duplicates() {
        let mut seen = BTreeSet::new();
        let twice: Vec<&str> = STRETCHES
            .iter()
            .map(|stretch| stretch.doing)
            .filter(|doing| !seen.insert(*doing))
            .collect();

        assert!(twice.is_empty(), "{twice:?} is in the table more than once");
    }

    #[test]
    fn the_first_half_of_the_stretches_is_most_of_the_work() {
        let half = STRETCHES.len() / 2;
        let front: u16 = STRETCHES.iter().take(half).map(|stretch| stretch.share).sum();
        assert!(
            front > WHOLE / 2,
            "the first {half} stretches are only {front} of {WHOLE}, so the bar would run at \
             the start and crawl at the end"
        );
    }

    #[test]
    fn walking_all_of_them_arrives() {
        let mut going = Going::drawing(Drawn::Lines);
        for stretch in STRETCHES {
            let Ok(()) = going.through(stretch.doing, || ());
        }
        assert_eq!(far(&going), WHOLE);
    }

    #[test]
    fn a_skipped_stretch_still_ends_at_the_end() {
        let mut going = Going::drawing(Drawn::Lines);
        for stretch in STRETCHES.iter().filter(|stretch| stretch.doing != PACKAGES) {
            let Ok(()) = going.through(stretch.doing, || ());
        }
        assert!(far(&going) < WHOLE, "nothing was skipped");

        let Ok(()) = going.done();
    }

    #[test]
    fn a_stretch_says_what_it_would_have_said_without_a_bar() {
        let mut going = Going::drawing(Drawn::Lines);
        let Ok(seven) = going.through(BUILDING, || 7);
        let Ok(said) = going.through(FILES, || Err::<(), String>("would not".to_string()));

        assert_eq!(seven, 7);
        assert_eq!(said, Err("would not".to_string()));

        let mut ran = 0;
        let Ok(()) = going.through(SERVICES, || ran += 1);

        assert_eq!(ran, 1, "the work did not run exactly once");
    }

    #[test]
    fn a_stretch_nobody_weighed_does_not_move_it() {
        let mut going = Going::drawing(Drawn::Lines);
        let Ok(()) = going.through(BUILDING, || ());
        let before = far(&going);
        let Ok(()) = going.through("something nobody put in the table", || ());

        assert_eq!(far(&going), before);
    }

    #[test]
    fn it_does_not_go_past_the_end() {
        let mut going = Going::drawing(Drawn::Lines);
        for _ in 0..4 {
            for stretch in STRETCHES {
                let Ok(()) = going.arrived(stretch.doing);
            }
        }
        assert_eq!(far(&going), WHOLE);
    }
}
