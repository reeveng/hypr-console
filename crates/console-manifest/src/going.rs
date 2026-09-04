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


use console_number::toward_zero_u16;
use std::io::IsTerminal;
use std::io::Write;

use console_notices::updating;

use crate::went;

/// The stretches, by the name they are asked for at the call site.
///
/// Constants rather than strings written twice, so a stretch that is timed
/// under a name the table does not have is a compile error rather than a bar
/// that quietly never reaches the end.
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

/// One stretch of an apply, and what share of it that stretch usually is.
#[derive(Debug, Clone, Copy)]
pub struct Stretch {
    pub doing: &'static str,
    pub share: u16,
}

/// What the shares add up to.
pub const WHOLE: u16 = 100;

/// Every stretch, in the order an apply does them.
///
/// The order is the apply's and cannot be rearranged: it is what depends on
/// what. The shares are what makes the bar mean anything.
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

/// How wide the bar is drawn, in cells.
const CELLS: u16 = 24;

/// Where the account of progress goes.
///
/// A bar that redraws itself needs a terminal to redraw on. Over ssh without
/// a tty -- which is how `tools/console-deploy` runs this -- stderr is a pipe,
/// and a carriage return there is a log with the whole run on one line. So
/// that case gets a line per stretch instead, which is the same information
/// in the form the reader has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawn {
    /// Redrawn in place, on a terminal.
    Bar,
    /// One line as each stretch finishes, into a pipe or a file.
    Lines,
}

/// Which of those this process has.
pub fn drawn() -> Drawn {
    match std::io::stderr().is_terminal() {
        true => Drawn::Bar,
        false => Drawn::Lines,
    }
}

/// Whether the desktop's bar is told as well.
///
/// A word rather than a `bool` for the reason `went::Asked` is one: the tests
/// walk every stretch several times over, and a suite that wrote
/// `/run/console/updating` and signalled waybar thirteen times a run would be
/// a test suite that redraws the machine it is running on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told {
    /// Written where the bar reads, and waybar woken.
    Bar,
    /// Nobody. Only the tests, which is why it is only compiled for them:
    /// an apply always has a bar to tell, even when nothing is listening.
    #[cfg(test)]
    Nobody,
}

/// What waybar is woken with.
///
/// `SIGUSR2` is taken: `units` sends that to make waybar read its config
/// again, which restarts every module on the bar. A real-time signal wakes
/// the one module that has something new to say and leaves the rest alone.
/// The number is the `signal` the module is given in `config.jsonc`, and the
/// two have to agree.
pub const WAKING: &str = "-RTMIN+4";

/// Tell the bar, and wake it.
///
/// Nothing here is checked. The bar is a convenience and the apply is the
/// work: a desktop that is not running, a waybar that is not up, a `/run` that
/// cannot be written -- none of those is a reason to stop bringing the machine
/// up to the manifest.
fn tell(percent: u16, doing: &str) {
    updating::wrote(&updating::Far { percent, doing: doing.to_string() });
    wake();
}

/// Wake waybar, however that goes.
fn wake() {
    let _ = std::process::Command::new("pkill")
        .arg(WAKING)
        .args(["-x", "waybar"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// How far along an apply is.
#[derive(Debug)]
pub struct Going {
    done: u16,
    drawn: Drawn,
    told: Told,
}

impl Going {
    /// At the beginning, drawing wherever this process can.
    pub fn starting() -> Self {
        Going { done: 0, drawn: drawn(), told: Told::Bar }
    }

    /// The same, told where to draw and telling nobody, so the arithmetic can
    /// be tested without a terminal to test it on and without writing to the
    /// machine underneath the test.
    #[cfg(test)]
    pub fn drawing(drawn: Drawn) -> Self {
        Going { done: 0, drawn, told: Told::Nobody }
    }

    /// Run one stretch, time it if anybody asked, and move the bar on.
    ///
    /// The work is handed back whatever it returns, exactly as `went::to`
    /// does: a stretch put under this changes nothing about what the code
    /// around it means.
    pub fn through<T>(&mut self, doing: &'static str, work: impl FnOnce() -> T) -> T {
        let done = went::to(doing, work);
        self.arrived(doing);
        done
    }

    /// Run one stretch that says how far into itself it has got.
    ///
    /// The work is handed something to call as it goes, with how far through
    /// itself it is, and the bar moves inside the stretch's own share. For the
    /// one stretch long enough that a bar standing still in the middle of it
    /// reads as a machine that has stopped.
    ///
    /// It ends exactly where `through` would have ended it. What the work said
    /// on the way moves the bar and is then forgotten, so a stretch that
    /// reported nothing, or reported badly, still leaves the bar where the next
    /// stretch expects to find it.
    pub fn during<T>(
        &mut self,
        doing: &'static str,
        work: impl FnOnce(&mut dyn FnMut(f64)) -> T,
    ) -> T {
        let start = self.done;
        let share = share_of(doing);
        let done = {
            let going = &mut *self;
            let mut moved = |far: f64| going.inside(doing, start, share, far);
            went::to(doing, || work(&mut moved))
        };
        self.done = start;
        self.arrived(doing);
        done
    }

    /// Somewhere inside a stretch: the strip is told, and the terminal is not.
    ///
    /// Only when the whole number changes. A crate a second for two minutes is
    /// a hundred and twenty writes to `/run` and a hundred and twenty signals
    /// to waybar, for a strip a row tall that can say a hundred things.
    ///
    /// The terminal is left alone because the work doing the reporting is
    /// already writing to it: cargo's own lines go past as they arrive, and a
    /// bar redrawn with a carriage return between two of them is a bar written
    /// across whatever cargo said last. Somebody at the terminal can see the
    /// crates going by; the strip is the one that had nothing to show.
    fn inside(&mut self, doing: &str, start: u16, share: u16, far: f64) {
        let far = far.clamp(0.0, 1.0);
        let inside = toward_zero_u16(f64::from(share) * far);
        let now = start.saturating_add(inside).min(WHOLE);

        // Forwards only. A stretch that reported badly -- or one whose work
        // counts something that can go down -- would otherwise pull the strip
        // back, and a strip that goes back reads as an apply that has started
        // again.
        if now <= self.done {
            return;
        }

        self.done = now;

        if self.told == Told::Bar {
            tell(self.far(), doing);
        }
    }

    /// Move the bar on by one stretch's share, and draw it.
    pub fn arrived(&mut self, doing: &'static str) {
        self.done = self.done.saturating_add(share_of(doing)).min(WHOLE);
        self.draw(doing);

        if self.told == Told::Bar {
            tell(self.far(), doing);
        }
    }

    /// How far along it is, out of a hundred.
    pub fn far(&self) -> u16 {
        self.done
    }

    /// The end of it, whatever the shares came to.
    ///
    /// Always a hundred. A bar that stopped at 97 because a stretch was
    /// skipped is a bar somebody goes on waiting in front of.
    pub fn done(mut self) {
        self.done = WHOLE;
        self.draw("done");

        if self.drawn == Drawn::Bar {
            let _ = writeln!(std::io::stderr());
        }

        // Taken away rather than left at a hundred. A full bar that stays up
        // is a bar somebody has to decide is finished; an empty module is one
        // waybar leaves out, and the bar goes back to what it was.
        if self.told == Told::Bar {
            updating::done();
            wake();
        }
    }

    fn draw(&self, doing: &str) {
        let full = usize::from(self.far() * CELLS / WHOLE);
        let empty = usize::from(CELLS) - full;
        let mut out = std::io::stderr();
        let _ = match self.drawn {
            Drawn::Bar => write!(
                out,
                "\r  [{}{}] {:>3}%  {:<20}",
                "#".repeat(full),
                "-".repeat(empty),
                self.far(),
                doing
            ),
            Drawn::Lines => writeln!(out, "  {:>3}%  {doing}", self.far()),
        };
        let _ = out.flush();
    }
}

/// What share of an apply a stretch is, or nothing if it is not one.
///
/// Nothing rather than a guess: a stretch nobody put in the table should
/// leave the bar where it was rather than move it by an invented amount.
fn share_of(doing: &str) -> u16 {
    STRETCHES
        .iter()
        .find(|stretch| stretch.doing == doing)
        .map_or(0, |stretch| stretch.share)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The strip moves while the longest stretch runs, and the stretch still
    /// ends where every other stretch would have left it. A bar that ended a
    /// stretch somewhere else would put every stretch after it in the wrong
    /// place.
    #[test]
    fn a_stretch_that_says_how_far_it_has_got_moves_the_bar_and_still_lands_where_it_should() {
        let mut going = Going::drawing(Drawn::Lines);
        going.arrived(READING);
        going.arrived(WANTED);
        going.arrived(PACKAGES);
        going.arrived(KEEPING);
        going.arrived(SWEEPING);
        let before = going.far();
        let mut seen = Vec::new();
        going.during(BUILDING, |moved| {
            for far in [0.1, 0.5, 0.9] {
                moved(far);
                seen.push(0);
            }
        });
        assert_eq!(seen.len(), 3);
        assert_eq!(going.far(), before + share_of(BUILDING));
    }

    /// And what it says on the way stays inside its own share: half way
    /// through a stretch worth sixty, ten in, is forty.
    #[test]
    fn what_a_stretch_says_on_the_way_stays_inside_its_own_share() {
        let mut going = Going::drawing(Drawn::Lines);
        going.inside(BUILDING, 10, 60, 0.5);
        assert_eq!(going.far(), 40);
        going.inside(BUILDING, 10, 60, 2.0);
        assert_eq!(going.far(), 70, "a stretch reported past its end went past it");
    }

    /// Forwards only, whatever it is told. A strip that went back would be
    /// read as an apply that had started again.
    #[test]
    fn a_stretch_that_says_it_has_gone_backwards_moves_nothing() {
        let mut going = Going::drawing(Drawn::Lines);
        going.inside(BUILDING, 10, 60, 0.5);
        going.inside(BUILDING, 10, 60, 0.1);
        going.inside(BUILDING, 10, 60, -1.0);
        assert_eq!(going.far(), 40);
    }

    /// The shares are a whole, or the bar does not reach the end.
    ///
    /// This is the one thing that has to stay true when somebody corrects the
    /// numbers from a real apply, and it is the thing they will forget.
    #[test]
    fn the_shares_add_up() {
        let all: u16 = STRETCHES.iter().map(|stretch| stretch.share).sum();
        assert_eq!(all, WHOLE, "the shares come to {all} rather than {WHOLE}");
    }

    /// No stretch is named twice.
    ///
    /// Two rows with one name would count that stretch once and leave the
    /// other row's share unreachable, so the bar would stop short by exactly
    /// that much and nothing would say why.
    #[test]
    fn no_stretch_is_named_twice() {
        let mut names: Vec<&str> = STRETCHES.iter().map(|stretch| stretch.doing).collect();
        names.sort_unstable();
        let all = names.len();
        names.dedup();
        assert_eq!(names.len(), all, "a stretch is in the table more than once");
    }

    /// The bar is front-loaded, which is the whole claim it makes.
    ///
    /// Half the stretches done should be most of the time gone, because the
    /// long ones come first. If a rearrangement ever made the tail the
    /// expensive half, this says so rather than letting the bar quietly start
    /// lying about how much longer.
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

    /// Going through every stretch reaches exactly the end.
    #[test]
    fn walking_all_of_them_arrives() {
        let mut going = Going::drawing(Drawn::Lines);
        for stretch in STRETCHES {
            going.through(stretch.doing, || ());
        }
        assert_eq!(going.far(), WHOLE);
    }

    /// A stretch that is skipped does not stop the bar reaching the end.
    ///
    /// Stretches are skipped: `installing` does nothing when no package is
    /// missing. Somebody watching should not be left in front of a bar that
    /// stopped at ninety-four because their machine was already up to date.
    #[test]
    fn a_skipped_stretch_still_ends_at_the_end() {
        let mut going = Going::drawing(Drawn::Lines);
        for stretch in STRETCHES.iter().filter(|stretch| stretch.doing != PACKAGES) {
            going.through(stretch.doing, || ());
        }
        assert!(going.far() < WHOLE, "nothing was skipped");
        going.done();
    }

    /// The work is handed back whatever it says, and runs once.
    ///
    /// The same promise `went::to` makes, asked again here because this is
    /// the wrapper the apply actually calls: a bar that changed an answer
    /// would be a progress indicator that broke the thing it reports on.
    #[test]
    fn a_stretch_says_what_it_would_have_said_without_a_bar() {
        let mut going = Going::drawing(Drawn::Lines);
        assert_eq!(going.through(BUILDING, || 7), 7);
        let said: Result<(), String> = going.through(FILES, || Err("would not".to_string()));
        assert_eq!(said, Err("would not".to_string()));

        let mut ran = 0;
        going.through(SERVICES, || ran += 1);
        assert_eq!(ran, 1, "the work did not run exactly once");
    }

    /// A name the table does not have leaves the bar where it was.
    #[test]
    fn a_stretch_nobody_weighed_does_not_move_it() {
        let mut going = Going::drawing(Drawn::Lines);
        going.through(BUILDING, || ());
        let far = going.far();
        going.through("something nobody put in the table", || ());
        assert_eq!(going.far(), far);
    }

    /// It never goes past the end, whatever it is told.
    #[test]
    fn it_does_not_go_past_the_end() {
        let mut going = Going::drawing(Drawn::Lines);
        for _ in 0..4 {
            for stretch in STRETCHES {
                going.arrived(stretch.doing);
            }
        }
        assert_eq!(going.far(), WHOLE);
    }
}
