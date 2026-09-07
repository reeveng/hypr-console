//! What somebody holding the device sees while the checks are run at it.
//!
//! A device run is driven from a laptop, and everything it says it says there:
//! a line per check on somebody else's terminal, in another room. The person
//! actually holding the machine sees the menus open by themselves for several
//! minutes and has nothing at all telling them what it is, how much of it is
//! left, or that it ended well.
//!
//! So the run says it on the device. Two things, and no more.
//!
//! ## The strip, because it is already there
//!
//! An apply fills a row of pixels under the bar as it goes, and that is exactly
//! this question asked about a different long thing:
//! `console_notifications::updating` is the file both ends of it agree on, and
//! it is filled here over the same ssh the checks are pressed through.
//!
//! Not a panel. A panel over the desktop is a layer the checks then have to
//! press through, and several of them ask what is on the screen and what colour
//! it is -- a surface put up to report the run would be the run's own worst
//! interference, and it would be the checks that went red for it.
//!
//! ## A card, because the strip has no words
//!
//! ### It fills while a check runs, not only when one ends
//!
//! The run is longest-first, so the first check is the longest there is, and
//! for its two minutes the strip stood exactly where the previous run had left
//! it. `Watching` is what moves it in between: a check that has been timed
//! before knows roughly how long it is, `lasting::crept` walks its share of the
//! bar with the clock and keeps the last tenth back, and the number is written
//! to the device on the way past.
//!
//! What is drawn is the run's own fill and never the check's. A bar that filled
//! for the check that is running restarts at nothing a dozen times in a run,
//! which is a bar that moves in steps however smoothly each step is drawn --
//! and the question nobody was asking is how far through the fourth check it
//! is. The counter and the name say which check; the bar says how much of the
//! run is left, from the first second to the last, and it only ever goes
//! forward.
//!
//! ### The strip rides along, the line runs on a clock
//!
//! These are two readers with two different costs, so they are fed two
//! different ways.
//!
//! The strip is on the device, at the far end of an ssh, and it is painted in
//! twentieths -- waybar has no progress widget, so `bar-updating` sends one of
//! a fixed set of classes and the stylesheet fills a gradient to it. Writing it
//! more often than it can be painted is a handheld woken for nothing. So the
//! write rides in front of a command that was being sent anyway, and only when
//! the number it would write has actually changed: a check that talks to the
//! device forty times does not wake the bar forty times, and a check that has
//! stopped talking to it stops moving the strip, which is the truth about it.
//!
//! Behind the command and never in front of it. A check is a sequence of
//! presses and questions whose spacing is the thing being checked, and a
//! `mkdir`, a write, a rename and a signal in front of every one of them is
//! milliseconds added to the moment that matters -- to the press, not to the
//! bookkeeping. The run does what it came to do first and the strip is told
//! afterwards, in the same round trip.
//!
//! The line on the terminal driving the run is free by comparison -- it is a
//! local write to a local screen -- and riding it on the ssh made it move in
//! steps, because that is what the ssh does. So it is drawn on a clock instead,
//! by a thread that asks `console_waiting::until` whether the run has ended and
//! redraws every time the answer is no. Two hundred milliseconds, which is
//! pacman's own `UPDATE_SPEED_SEC`: fast enough that a person reads it as
//! movement rather than as steps, slow enough that it is not a program
//! redrawing a terminal for its own sake. What the line draws between two
//! checks is arithmetic on the clock and the estimate -- `Ahead::along` -- so
//! it keeps moving whether or not the run has anything to say.
//!
//! Everything the run prints goes through `quietly`, which takes the same lock
//! the drawing thread does, wipes the line, prints under it, and lets the next
//! tick draw it again. Without the lock the line would come back between the
//! wipe and the print and the two would land on the same row.
//!
//! It is the line `console-how-far` draws an apply in, because they are the
//! same question asked about two long things and there is no reason for them to
//! have two faces. It is wiped for good when the check ends and the run prints
//! its own result line: the record of what happened is the list of checks and
//! how they went, and a bar that stayed behind would only be saying the same
//! thing worse.
//!
//! The strip is two pixels of fill with its tooltip turned off, so it can say
//! how much is left and nothing else. What it cannot say is what is happening,
//! or how it went once it is gone. A notification is raised when the run starts
//! -- so a person whose device has begun pressing its own buttons is told why
//! -- and replaced by one at the end saying how it went. Replaced rather than
//! added: one run is one card, which is the same rule everything else here
//! raising a notice keeps.
//!
//! The card at the end stays on the screen when something failed. A run that
//! ends badly while somebody is making tea is the whole reason to say it twice.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_number_conversion::toward_zero_u16;
use console_how_far::Bar;
use console_notifications::saying::Notice;
use console_notifications::updating::{self, Far, WAKING};
use console_waiting::{Patience, Seen, until};

use crate::device::{Device, quoted};
use crate::lasting::{Ahead, WHOLE, about, crept, over};

pub fn writing(far: &Far) -> Result<String, Never> {
    let Ok(where_at) = updating::at();
    let Ok(written) = updating::written(far);

    let at = where_at.display().to_string();
    let beside = format!("{at}.writing");
    let Ok(said) = quoted(&written);
    let Ok(holding) = holding_of(&at);
    let Ok(holding) = quoted(&holding);
    let Ok(writing) = quoted(&beside);
    let Ok(where_) = quoted(&at);
    let Ok(waking) = waking();

    Ok(format!(
        "mkdir -p {holding} && printf %s {said} > {writing} && mv {writing} {where_}; {waking}"
    ))
}

pub fn showing(device: &mut Device, ahead: &Ahead, doing: &str) -> Result<(), Never> {
    let Ok(percent) = ahead.percent();
    let Ok(said) = writing(&Far { percent, doing: doing.to_string() });
    let Ok(_) = device.ssh(&said);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reached {
    pub into: u16,
    pub far: u16,
}

pub fn late(gone: Duration, expecting: Duration) -> Result<String, Never> {
    let Ok(over) = over(gone, expecting);

    let Some(over) = over else {
        return Ok(String::new());
    };

    let Ok(about) = about(over);

    Ok(format!("({about} so far)"))
}

pub fn reached(at: u16, span: u16, gone: Duration, expecting: Duration) -> Result<Reached, Never> {
    let Ok(part) = crept(gone, expecting);
    let Ok(into) = console_how_far::percent(part);
    let Ok(inside) = toward_zero_u16(f64::from(span) * part);

    Ok(Reached { into, far: at.saturating_add(inside).min(WHOLE) })
}

pub const BETWEEN: Duration = Duration::from_millis(200);

pub const OUTSIDE: Duration = Duration::from_secs(60 * 60 * 4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Yes,
    No,
}

#[derive(Debug)]
pub struct Watching {
    bar: Bar,
    ahead: Option<Ahead>,
    doing: String,
    started: Instant,
    said: u16,
    ended: Ended,
}

impl Watching {
    pub fn of(many: usize) -> Result<Self, Never> {
        let Ok(bar) = Bar::of(many);

        Ok(Watching {
            bar,
            ahead: None,
            doing: String::new(),
            started: Instant::now(),
            said: 0,
            ended: Ended::No,
        })
    }

    pub fn on(&mut self, ahead: &Ahead, doing: &str) -> Result<(), Never> {
        let Ok(at) = ahead.percent();

        self.ahead = Some(ahead.clone());
        self.started = Instant::now();
        self.said = at;
        self.doing = doing.to_string();

        let Ok(along) = ahead.along(Duration::ZERO);

        self.bar.onto(doing, along)
    }

    pub fn ended(&self) -> Result<Ended, Never> {
        Ok(self.ended)
    }

    pub fn drawn(&mut self) -> Result<(), Never> {
        let Some(ahead) = &self.ahead else {
            return Ok(());
        };

        let gone = self.started.elapsed();
        let Ok(along) = ahead.along(gone);
        let Ok(expecting) = ahead.expecting();
        let Ok(late) = late(gone, expecting);

        self.bar.filling(along, &late)
    }

    pub fn tick(&mut self) -> Result<String, Never> {
        let Ok(()) = self.drawn();

        let Some(ahead) = &self.ahead else {
            return Ok(String::new());
        };

        let Ok(far) = ahead.along(self.started.elapsed());

        match far == self.said {
            true => return Ok(String::new()),
            false => {},
        }

        self.said = far;

        let Ok(said) = writing(&Far { percent: far, doing: self.doing.clone() });

        Ok(format!("; {said}"))
    }

    pub fn quiet(&mut self) -> Result<(), Never> {
        self.bar.wiped()
    }

    pub fn ending(&mut self) -> Result<(), Never> {
        self.ended = Ended::Yes;

        self.bar.wiped()
    }
}

pub fn held(watching: &Mutex<Watching>) -> Result<MutexGuard<'_, Watching>, Never> {
    Ok(match watching.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

pub fn on(watching: &Mutex<Watching>, ahead: &Ahead, doing: &str) -> Result<(), Never> {
    let Ok(mut held) = held(watching);

    held.on(ahead, doing)
}

pub fn quietly(watching: &Mutex<Watching>, work: impl FnOnce()) -> Result<(), Never> {
    let Ok(mut held) = held(watching);
    let Ok(()) = held.quiet();

    work();

    Ok(())
}

pub fn ending(watching: &Mutex<Watching>) -> Result<(), Never> {
    let Ok(mut held) = held(watching);

    held.ending()
}

pub fn drawing(watching: Arc<Mutex<Watching>>) -> Result<std::thread::JoinHandle<()>, Never> {
    Ok(std::thread::spawn(move || {
        let Ok(patience) = Patience::asking_every(OUTSIDE, BETWEEN);
        let Ok(_) = until(patience, || {
            let Ok(mut held) = held(&watching);
            let Ok(ended) = held.ended();

            Ok(match ended {
                Ended::Yes => Seen::Yes,

                Ended::No => {
                    let Ok(()) = held.drawn();

                    Seen::NotYet
                }
            })
        });
    }))
}

pub fn done_showing(device: &mut Device) -> Result<(), Never> {
    let Ok(where_at) = updating::at();
    let Ok(at) = quoted(&where_at.display().to_string());
    let Ok(waking) = waking();
    let Ok(_) = device.ssh(&format!("rm -f {at}; {waking}"));

    Ok(())
}

fn waking() -> Result<String, Never> {
    Ok(format!("pkill {WAKING} -x waybar || true"))
}

fn holding_of(at: &str) -> Result<String, Never> {
    Ok(match at.rsplit_once('/') {
        Some((above, _)) => above.to_string(),
        None => ".".to_string(),
    })
}

pub fn said(device: &mut Device, notice: &Notice) -> Result<Option<u32>, Never> {
    let Ok(said) = notice.argv();

    let argv: Vec<String> = said
        .iter()
        .map(|word| {
            let Ok(quoted) = quoted(word);

            quoted
        })
        .collect();
    let Ok(said) = device.in_session(&argv.join(" "));

    Ok(match said.trim().parse::<u32>() {
        Ok(id) => Some(id),
        Err(_no_id) => None,
    })
}

pub const STARTING: &str = "Checking the desktop";

pub fn starting(many: usize, ahead: &Ahead) -> Result<Notice, Never> {
    let Ok(whole) = ahead.whole();

    let long = match whole {
        Some(whole) => {
            let Ok(about) = about(whole);

            format!(", about {about}")
        }
        None => String::new(),
    };

    let Ok(notice) =
        Notice::new(STARTING, &format!("{many} checks{long}. Don't touch the controls."));

    notice.staying()
}

pub fn ended(
    ok: usize,
    failed: &[String],
    took: Duration,
    was: Option<u32>,
) -> Result<Notice, Never> {
    let notice = match failed.first() {
        None => {
            let Ok(about) = about(took);
            let Ok(notice) =
                Notice::new("Checks passed", &format!("{ok} of them, in {about}."));
            let Ok(notice) = notice.lasting(8000);

            notice
        }

        Some(_something) => {
            let Ok(notice) = Notice::new(
                "Checks failed",
                &format!("{ok} passed, {} failed: {}.", failed.len(), failed.join(", ")),
            );
            let Ok(notice) = notice.urgent();
            let Ok(notice) = notice.staying();

            notice
        }
    };

    notice.replacing(was)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checking::{Body, Check, Done};

    fn nothing(_device: &mut Device) -> Done {
        Ok(())
    }

    const fn check(name: &'static str) -> Check {
        Check {
            name,
            about: "A check.",
            feature: "nothing",
            since: "2026-09-06",
            bodies: &[Body::Device(nothing)],
        }
    }

    const ONE: Check = check("010-one");
    const TWO: Check = check("020-two");

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    #[test]
    fn the_card_at_the_start_says_how_long_when_the_machine_has_been_timed() {
        let ahead = ok(Ahead::of(&crate::lasting::Lengths::default(), &[]));
        let said = ok(starting(10, &ahead));

        assert!(!said.body.contains("about"), "a machine nobody timed was promised a length");
    }

    #[test]
    fn the_card_at_the_end_replaces_the_one_at_the_start() {
        let said = ok(ended(10, &[], Duration::from_secs(120), Some(41)));

        let argv = ok(said.argv());

        assert_eq!(said.replacing, Some(41));
        assert!(argv.iter().any(|word| word == "--replace-id=41"));
    }

    #[test]
    fn a_run_that_failed_names_what_failed_and_stays_on_the_screen() {
        let said = ok(ended(9, &["120-a-page".to_string()], Duration::from_secs(120), None));

        assert!(said.body.contains("120-a-page"), "the card does not say what failed");
        assert_eq!(said.expiry, console_notifications::saying::Expiry::Stays);
    }

    #[test]
    fn the_strip_is_told_after_the_command_rather_than_before_it() {
        let Ok(mut watching) = Watching::of(2);
        let Ok(carried) = crate::lasting::reading("010-one 1000\n020-two 1000\n");
        let Ok(ahead) =
            Ahead::from(&crate::lasting::Lengths::default(), &carried, &[&ONE, &TWO]);
        let Ok(()) = watching.on(&ahead, ONE.name);
        watching.said = u16::MAX;

        let Ok(said) = watching.tick();

        assert!(
            said.starts_with("; "),
            "the strip is written in front of the press rather than behind it: {said}"
        );
    }

    #[test]
    fn what_is_written_is_the_file_the_strip_reads_and_the_number_it_reads() {
        let Ok(said) = writing(&Far { percent: 40, doing: "120-a-page".to_string() });

        assert!(said.contains("/run/console/updating"), "{said}");
        assert!(said.contains("40 120-a-page"), "{said}");
        assert!(said.contains("waybar"), "the bar was not woken: {said}");
    }

    #[test]
    fn the_line_is_drawn_on_a_clock_rather_than_when_the_run_happens_to_speak() {
        assert!(
            BETWEEN <= Duration::from_millis(200),
            "the line is redrawn every {BETWEEN:?}, which reads as steps rather than movement"
        );
        assert!(
            BETWEEN >= Duration::from_millis(50),
            "the line is redrawn every {BETWEEN:?}, which is a handheld's ssh session woken for \
             nothing a person could see"
        );
    }

    #[test]
    fn a_check_that_has_only_just_started_has_not_moved_the_strip() {
        assert_eq!(
            reached(20, 10, Duration::ZERO, Duration::from_secs(60)),
            Ok(Reached { into: 0, far: 20 })
        );
    }

    #[test]
    fn a_check_halfway_through_its_estimate_is_halfway_along_its_own_share() {
        let Ok(reached) = reached(20, 10, Duration::from_secs(30), Duration::from_secs(60));

        assert_eq!(reached, Reached { into: 45, far: 24 });
    }

    #[test]
    fn a_check_running_long_says_how_long_rather_than_only_creeping() {
        assert_eq!(late(Duration::from_secs(30), Duration::from_secs(60)), Ok(String::new()));
        assert_eq!(
            late(Duration::from_secs(180), Duration::from_secs(60)),
            Ok("(3 minutes so far)".to_string())
        );
        assert_eq!(
            late(Duration::from_secs(180), Duration::ZERO),
            Ok(String::new()),
            "a check nothing has ever timed was called late"
        );
    }

    #[test]
    fn a_check_stays_inside_its_own_share_however_long_it_runs() {
        let Ok(reached) = reached(20, 10, Duration::from_secs(6000), Duration::from_secs(60));

        assert!(reached.far < 30, "it walked into the next check's share: {reached:?}");
        assert!(reached.far > 28, "it stopped moving well short of its own end: {reached:?}");
    }

    #[test]
    fn a_check_nothing_has_ever_timed_leaves_the_strip_where_the_last_one_left_it() {
        assert_eq!(
            reached(20, 10, Duration::from_secs(30), Duration::ZERO),
            Ok(Reached { into: 0, far: 20 }),
            "a run with nothing to go on invented a number"
        );
    }

    #[test]
    fn the_last_check_of_a_run_cannot_carry_the_strip_past_the_end() {
        let Ok(reached) = reached(96, 10, Duration::from_secs(600), Duration::from_secs(1));

        assert_eq!(reached.far, WHOLE);
    }

    #[test]
    fn the_directory_the_strip_is_written_in_is_the_one_holding_it() {
        assert_eq!(ok(holding_of("/run/console/updating")), "/run/console");
        assert_eq!(ok(holding_of("updating")), ".");
    }
}
