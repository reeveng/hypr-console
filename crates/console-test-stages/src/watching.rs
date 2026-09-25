//! What someone holding the device sees while the checks are run at it.
//!
//! A device run is driven from a laptop, and everything it says it says there:
//! a line per check on someone else's terminal, in another room. The person
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
//! press through, and several of them ask what is on the screen and what color
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
//! and the question no one was asking is how far through the fourth check it
//! is. The counter and the name say which check; the bar says how much of the
//! run is left, from the first second to the last, and it only ever goes
//! forward.
//!
//! ### The strip rides along, the line runs on a clock
//!
//! These are two readers with two different costs, so they are fed two
//! different ways.
//!
//! The strip is on the device, at the far end of an ssh, and it is a row of
//! the bar's own surface filled to a share of its width. Writing it more often
//! than it can be painted is a handheld woken for nothing. So the write rides
//! in front of a command that was being sent anyway, and only when
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
//! raising a notification keeps.
//!
//! The card at the end stays on the screen when something failed. A run that
//! ends badly while someone is making tea is the whole reason to say it twice.
//!
//! The strip is drawn from two places and wiped by whoever is about to print,
//! and the end of a run is where that showed. The thread here redraws it on a
//! clock, and so does every command sent to the device: `Device::ssh` ticks the
//! strip so a check that spends a minute in one ssh still moves. `ending` sets
//! the run down, stops the thread and wipes -- and then the run puts the device
//! back, which is half a dozen more ssh commands, each of which drew the strip
//! again on the line that had just been cleared. What a person saw was the
//! putting-back sentence welded to the right-hand end of a full bar, and the
//! bar again under it, on a run that had already finished.
//!
//! So a draw asked for after the run has ended is not drawn. It is asked here
//! rather than at the two call sites because the tick is the device's and the
//! device does not know the run is over. Every line the run prints goes through
//! `quietly`, which wipes before it prints: a `println!` beside the strip is a
//! line drawn on top of it.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_number_conversion::toward_zero_u16;
use console_how_far::Bar;
use console_notifications::saying::{Notification, Content};
use console_notifications::updating::{self, Progress};
use console_waiting::{Schedule, Ready, until};

use crate::device::{Device, quoted};
use crate::lasting::{Ahead, WHOLE, about, crept, over};

pub fn writing(far: &Progress) -> Result<String, Never> {
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
    let Ok(permille) = ahead.far();
    let Ok(said) = writing(&Progress { permille, label: doing.to_string() });
    let Ok(_) = device.ssh(&said);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reached {
    pub into: u16,
    pub far: u16,
}

pub fn late(elapsed: Duration, expecting: Duration) -> Result<String, Never> {
    let Ok(over) = over(elapsed, expecting);

    let over = match over {
        Some(over) => over,
        None => return Ok(String::new()),
    };

    let Ok(about) = about(over);

    Ok(format!("({about} so far)"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaching {
    pub at: u16,
    pub span: u16,
    pub elapsed: Duration,
    pub expecting: Duration,
}

pub fn reached(reaching: Reaching) -> Result<Reached, Never> {
    let Reaching { at, span, elapsed, expecting } = reaching;
    let Ok(part) = crept(elapsed, expecting);
    let Ok(into) = console_how_far::percent(part);
    let Ok(inside) = toward_zero_u16(f64::from(span) * part);

    Ok(Reached { into, far: at.saturating_add(inside).min(WHOLE) })
}

pub fn percent_of(permille: u16) -> Result<u16, Never> {
    console_how_far::percent(f64::from(permille) / f64::from(WHOLE))
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

#[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
        explicit039_no_reading_the_clock,
        reason = "the stopwatch the strip is drawn from: how far a check has crept is how long it has been running, and a reading handed in would be the caller timing the run instead of this"
    )
)]
impl Watching {
    pub fn of(many: u32) -> Result<Self, Never> {
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
        let Ok(at) = ahead.far();

        self.ahead = Some(ahead.clone());
        self.started = Instant::now();
        self.said = at;
        self.doing = doing.to_string();

        let Ok(along) = ahead.along(Duration::ZERO);
        let Ok(into) = percent_of(along);

        self.bar.onto(doing, into)
    }

    pub fn ended(&self) -> Result<Ended, Never> {
        Ok(self.ended)
    }

    pub fn drawn(&mut self) -> Result<(), Never> {
        match self.ended {
            Ended::Yes => return Ok(()),
            Ended::No => {},
        }

        let ahead = match &self.ahead {
            Some(ahead) => ahead,
            None => return Ok(()),
        };

        let elapsed = self.started.elapsed();
        let Ok(along) = ahead.along(elapsed);
        let Ok(expecting) = ahead.expecting();
        let Ok(late) = late(elapsed, expecting);
        let Ok(into) = percent_of(along);

        self.bar.filling(into, &late)
    }

    pub fn tick(&mut self) -> Result<String, Never> {
        let Ok(()) = self.drawn();

        let ahead = match &self.ahead {
            Some(ahead) => ahead,
            None => return Ok(String::new()),
        };

        let Ok(far) = ahead.along(self.started.elapsed());

        match far == self.said {
            true => return Ok(String::new()),
            false => {},
        }

        self.said = far;

        let Ok(said) = writing(&Progress { permille: far, label: self.doing.clone() });

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
    quietly_handed(watching, &mut (), |_nothing| work())
}

pub fn quietly_handed<M>(
    watching: &Mutex<Watching>,
    handed: &mut M,
    work: impl FnOnce(&mut M),
) -> Result<(), Never> {
    let Ok(mut held) = held(watching);
    let Ok(()) = held.quiet();

    work(handed);

    Ok(())
}

pub fn ending(watching: &Mutex<Watching>) -> Result<(), Never> {
    let Ok(mut held) = held(watching);

    held.ending()
}

pub fn drawing(watching: Arc<Mutex<Watching>>) -> Result<std::thread::JoinHandle<()>, Never> {
    Ok(std::thread::spawn(move || {
        let Ok(patience) = Schedule::asking_every(OUTSIDE, BETWEEN);
        let Ok(_) = until(patience, || {
            let Ok(mut held) = held(&watching);
            let Ok(ended) = held.ended();

            Ok(match ended {
                Ended::Yes => Ready::Yes,

                Ended::No => {
                    let Ok(()) = held.drawn();

                    Ready::NotYet
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
    Ok(format!("pkill {} -x {} || true", console_onscreen::WAKING, console_onscreen::BAR))
}

fn holding_of(at: &str) -> Result<String, Never> {
    Ok(match at.rsplit_once('/') {
        Some((above, _)) => above.to_string(),
        None => ".".to_string(),
    })
}

pub fn said(device: &mut Device, notification: &Notification) -> Result<Option<u32>, Never> {
    let Ok(said) = notification.arguments();

    let arguments: Vec<String> = said
        .iter()
        .map(|word| {
            let Ok(quoted) = quoted(word);

            quoted
        })
        .collect();
    let Ok(said) = device.in_session(&arguments.join(" "));

    Ok(match said.trim().parse::<u32>() {
        Ok(id) => Some(id),
        Err(_no_id) => None,
    })
}

pub const STARTING: &str = "Checking the desktop";

pub fn starting(many: u32, ahead: &Ahead) -> Result<Notification, Never> {
    let Ok(whole) = ahead.whole();

    let long = match whole {
        Some(whole) => {
            let Ok(about) = about(whole);

            format!(", about {about}")
        }
        None => String::new(),
    };

    let Ok(notification) = Notification::new(Content {
        summary: STARTING,
        body: &format!("{many} checks{long}. Don't touch the controls."),
    });

    notification.staying()
}

pub fn ended(
    ok: u32,
    failed: &[String],
    took: Duration,
    was: Option<u32>,
) -> Result<Notification, Never> {
    let notification = match failed.first() {
        None => {
            let Ok(about) = about(took);
            let Ok(notification) = Notification::new(Content {
                summary: "Checks passed",
                body: &format!("{ok} of them, in {about}."),
            });
            let Ok(notification) = notification.lasting(8000);

            notification
        }

        Some(_something) => {
            let Ok(notification) = Notification::new(Content {
                summary: "Checks failed",
                body: &format!("{ok} passed, {} failed: {}.", failed.len(), failed.join(", ")),
            });
            let Ok(notification) = notification.urgent();
            let Ok(notification) = notification.staying();

            notification
        }
    };

    notification.replacing(was)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checking::{Body, Check, CheckResult};

    fn nothing(_device: &mut Device) -> CheckResult {
        Ok(())
    }

    #[test]
    fn the_bar_is_drawn_in_percent_though_the_run_is_counted_in_thousandths() {
        assert_eq!(percent_of(0), Ok(0));
        assert_eq!(percent_of(WHOLE / 2), Ok(50));
        assert_eq!(percent_of(WHOLE / 10), Ok(10));
        assert_eq!(percent_of(WHOLE), Ok(console_how_far::WHOLE));
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

        assert!(!said.body.contains("about"), "a machine no one timed was promised a length");
    }

    #[test]
    fn the_card_at_the_end_replaces_the_one_at_the_start() {
        let said = ok(ended(10, &[], Duration::from_secs(120), Some(41)));

        let arguments = ok(said.arguments());

        assert_eq!(said.replacing, Some(41));
        assert!(arguments.iter().any(|word| word == "--replace-id=41"));
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
        let Ok(said) = writing(&Progress { permille: 400, label: "120-a-page".to_string() });

        assert!(said.contains("/run/console/updating"), "{said}");
        assert!(said.contains("400 120-a-page"), "{said}");
        assert!(said.contains(console_onscreen::BAR), "the bar was not woken: {said}");
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
            reached(Reaching {
            at: 20,
            span: 10,
            elapsed: Duration::ZERO,
            expecting: Duration::from_secs(60),
        }),
            Ok(Reached { into: 0, far: 20 })
        );
    }

    #[test]
    fn a_check_halfway_through_its_estimate_is_halfway_along_its_own_share() {
        let Ok(reached) = reached(Reaching {
            at: 20,
            span: 10,
            elapsed: Duration::from_secs(30),
            expecting: Duration::from_secs(60),
        });

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
        let Ok(reached) = reached(Reaching {
            at: 20,
            span: 10,
            elapsed: Duration::from_secs(6000),
            expecting: Duration::from_secs(60),
        });

        assert!(reached.far < 30, "it walked into the next check's share: {reached:?}");
        assert!(reached.far > 28, "it stopped moving well short of its own end: {reached:?}");
    }

    #[test]
    fn a_check_nothing_has_ever_timed_leaves_the_strip_where_the_last_one_left_it() {
        assert_eq!(
            reached(Reaching {
            at: 20,
            span: 10,
            elapsed: Duration::from_secs(30),
            expecting: Duration::ZERO,
        }),
            Ok(Reached { into: 0, far: 20 }),
            "a run with nothing to go on invented a number"
        );
    }

    #[test]
    fn the_last_check_of_a_run_cannot_carry_the_strip_past_the_end() {
        let Ok(reached) = reached(Reaching {
            at: 960,
            span: 100,
            elapsed: Duration::from_secs(600),
            expecting: Duration::from_secs(1),
        });

        assert_eq!(reached.far, WHOLE);
    }

    #[test]
    fn the_directory_the_strip_is_written_in_is_the_one_holding_it() {
        assert_eq!(ok(holding_of("/run/console/updating")), "/run/console");
        assert_eq!(ok(holding_of("updating")), ".");
    }
}
