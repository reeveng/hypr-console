//! Everything this desktop has grown, tried again, oldest first.
//!
//! ```text
//! console-check                           here, against the emulator
//! console-check --list                    what there is, and what each is
//! console-check brightness                only the checks about that
//! console-check --stage device --dry-run  what it would do to the device
//! console-check --stage device --yes      do it
//! console-check --stage device --yes --all   every check written for it
//! ```
//!
//! The device is the last stage and it is someone's machine. Nothing is sent to
//! it without --yes, and --dry-run prints every command first so it can be read
//! before it is run. The pressing goes through InputPlumber's own SendEvent,
//! which is how the hardware's own buttons arrive, so nothing is created on the
//! device and nothing is left behind if this stops halfway.
//!
//! It is someone's machine in the other sense too, so the run gives it back
//! the way it found it. What was true before the first press -- the workspace,
//! the brightness, the volume, the profile -- is read once and put back after
//! the last check, along with anything the run opened, and the last line says
//! what was handed back. Ctrl-C is answered rather than fatal for the same
//! reason: a run stopped halfway is the one that would otherwise leave the
//! most behind. See `console_test_stages::putting_back`.
//!
//! Here, every check is a handheld of its own with nothing on it but the
//! emulator, so they run at once, one per core, through `console_concurrency`,
//! and are said in the order they were asked for once the last has answered.
//! The desktop and the device are one screen each and are taken a check at a
//! time.
//!
//! The device is also the slow stage, and most of what is written for it was already
//! answered here a second earlier. So asked for nothing in particular, the
//! machine is asked only what nothing else can answer, and says of the rest
//! where it was answered instead. --all is the whole tier for when the answer
//! wanted is about the hardware rather than the desktop. Naming a check is
//! asking for it: `--stage device brightness` runs brightness there whatever
//! the emulator thinks.
//!
//! Every check that ran, on every stage, is also a line in the waits store --
//! who is `check`, what is its name, with the stage and how it went -- so a
//! check that has been getting slower, or one whose passing follows how long it
//! happened to take, can be found in the history rather than on the day it
//! blocks a deploy. A skipped check did not run, so it writes nothing.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::time::Instant;

use console_core_arguments::{Command, Flag, Operands, Presence, ValidationError, Takes};
use console_test_checks::select;
use console_test_checks::Unchecked;
use console_core_never::Never;
use console_response_times::{Note, Wait, Waiting};
use console_test_stages::checking::{self, Check, How, Stage};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{self, Device, DryRun};
use console_test_stages::lasting::{self, Ahead};
use console_test_stages::putting_back;
use console_test_stages::stopping::{self, Stop};
use console_test_stages::watching;
use console_test_stages::here::Here;

const NONE_OF_THEM: u32 = 0;

const HERE: &str = "here";

const DESKTOP: &str = "desktop";

const DEVICE: &str = "device";


#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "four flags someone typed, and every combination of them is a command line: `--list --all`, `--dry-run --yes`, none of them"
    )
)]
struct Arguments {
    only: Vec<String>,
    stage: String,
    list: bool,
    dry_run: bool,
    yes: bool,
    all: bool,
}

const LIST: Flag = Flag { spelling: "--list", takes: Takes::None, about: "what there is, and what each is" };

const STAGE: Flag = Flag { spelling: "--stage", takes: Takes::Value("STAGE"), about: "where they run: here, desktop or device" };

const DRY_RUN: Flag =
    Flag { spelling: "--dry-run", takes: Takes::None, about: "print every command the device would be sent, and send none" };

const YES: Flag = Flag { spelling: "--yes", takes: Takes::None, about: "the device is somebody's, and this is them saying yes" };

const ALL: Flag =
    Flag { spelling: "--all", takes: Takes::None, about: "every check written for the device, not only what nothing else can answer" };

const COMMAND: Command = Command {
    name: "console-check",
    about: "Everything this desktop has grown, tried again, oldest first.",
    flags: &[LIST, STAGE, DRY_RUN, YES, ALL],
    operands: Operands::Any("CHECK"),
};

fn parse_arguments<W: AsRef<str>>(words: &[W]) -> Result<Arguments, ValidationError> {
    let line = console_core_arguments::read(&COMMAND, words)?;
    let Ok(only) = line.operands();
    let Ok(stage) = line.value(STAGE);
    let Ok(list) = line.presence(LIST);
    let Ok(dry_run) = line.presence(DRY_RUN);
    let Ok(yes) = line.presence(YES);
    let Ok(all) = line.presence(ALL);

    Ok(Arguments {
        only: only.to_vec(),
        stage: match stage {
            Some(stage) => stage.to_string(),
            None => HERE.to_string(),
        },
        list: list == Presence::Present,
        dry_run: dry_run == Presence::Present,
        yes: yes == Presence::Present,
        all: all == Presence::Present,
    })
}

struct Ansi {
    green: &'static str,
    red: &'static str,
    dim: &'static str,
    yellow: &'static str,
    off: &'static str,
}

const COLORED: Ansi =
    Ansi { green: "\x1b[32m", red: "\x1b[31m", dim: "\x1b[2m", yellow: "\x1b[33m", off: "\x1b[0m" };
const PLAIN: Ansi = Ansi { green: "", red: "", dim: "", yellow: "", off: "" };

impl Ansi {
    fn mark(&self, how: &How) -> Result<String, Never> {
        let (color, said) = match how {
            How::Ok => (self.green, "ok"),
            How::Failed(_) => (self.red, "failed"),
            How::Skipped(_) => (self.dim, "skipped"),
            How::Would => (self.yellow, "would run"),
        };
        Ok(format!("{color}{said:<9}{}", self.off))
    }
}

fn main() -> std::process::ExitCode {
    let ink = match std::io::stdout().is_terminal() {
        true => COLORED,
        false => PLAIN,
    };

    let words: Vec<String> = std::env::args().skip(1).collect();
    let asked = match parse_arguments(&words) {
        Ok(asked) => asked,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return std::process::ExitCode::from(code);
        }
    };

    match run(asked, &ink) {
        Ok(code) => code,
        Err(why) => {
            eprintln!("{why}");
            std::process::ExitCode::from(1)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Whole,
    WhatNothingElseCanAnswer,
}

fn spared(check: &Check, tier: Tier) -> Result<Option<Stage>, Never> {
    Ok(match tier {
        Tier::Whole => None,
        Tier::WhatNothingElseCanAnswer => {
            let Ok(spared) = check.without_the_device();

            spared
        }
    })
}

fn say(
    counted: &mut BTreeMap<&'static str, u32>,
    ink: &Ansi,
    check: &Check,
    how: How,
) -> Result<(), Never> {
    let Ok(name) = how.name();
    let Ok(why) = how.why();

    let tally = counted.entry(name).or_insert(0);
    *tally = tally.saturating_add(1);

    let aside = match why.is_empty() {
        true => String::new(),
        false => format!("{}{why}{}", ink.dim, ink.off),
    };
    let Ok(mark) = ink.mark(&how);

    println!("{:<28} {mark} {aside}", check.name);

    Ok(())
}

fn kept_up(host: &str, touching: DryRun) -> Result<Option<console_awake::Staying>, Never> {
    let asked = match touching {
        DryRun::Pretend => return Ok(None),
        DryRun::Really => console_awake::taking_on(host, console_awake::InhibitReason::FromChecking),
    };
    let Ok(asked) = asked;

    Ok(match asked {
        console_awake::InhibitResult::Acquired(staying) => Some(staying),
        console_awake::InhibitResult::Failed(said) => {
            eprintln!("console-check: {said}");

            None
        }
    })
}

fn on_the_device(
    asked: &Arguments,
    checks: Vec<&'static Check>,
    ink: &Ansi,
    counted: &mut BTreeMap<&'static str, u32>,
) -> Result<(), Unchecked> {
    let touching = match asked.dry_run {
        true => DryRun::Pretend,
        false => DryRun::Really,
    };
    let host = device::host()?;
    let mut stage = Device::new(&host, touching)?;
    let Ok(_kept_up) = kept_up(&host, touching);
    let tier = match asked.all || !asked.only.is_empty() {
        true => Tier::Whole,
        false => Tier::WhatNothingElseCanAnswer,
    };
    let mut running: Vec<&'static Check> = Vec::new();

    for check in checks {
        let Ok(spared) = spared(check, tier);

        match spared {
            Some(where_) => {
                let Ok(name) = where_.name();

                let Ok(()) = say(counted, ink, check, How::Skipped(format!("{name} answers this")));
            }
            None => running.push(check),
        }
    }

    let Ok(mut lengths) = lasting::read_from(&mut stage);
    let Ok(ordered) = lengths.longest_first(&running);
    let Ok(mut ahead) = Ahead::of(&lengths, &ordered);
    let Ok(whole) = ahead.whole();

    match whole {
        Some(whole) => {
            let Ok(about) = lasting::about(whole);

            println!("{}  about {about}{}", ink.dim, ink.off);
        }
        None => {}
    }

    let Ok(()) = stopping::caught();

    let was = match asked.dry_run {
        true => None,
        false => {
            let Ok(was) = putting_back::observe(&mut stage);
            let Ok(silenced) = putting_back::silence(&mut stage);

            match silenced {
                console_test_stages::device::Outcome::Happened => {}
                console_test_stages::device::Outcome::RanOut => {
                    println!("{}the sound would not go down, so this run is heard{}", ink.dim, ink.off);
                }
            }

            Some(was)
        }
    };
    let Ok(many) = console_core_number_conversion::fitted::<_, u32>(ordered.len());
    let Ok(starting) = watching::starting(many, &ahead);
    let Ok(card) = watching::notify(&mut stage, &starting);
    let began = Instant::now();
    let mut passed: u32 = 0;
    let mut failed: Vec<String> = Vec::new();
    let Ok(watch) = watching::Watching::of(many);
    let watch = std::sync::Arc::new(std::sync::Mutex::new(watch));
    let Ok(()) = stage.set_watching(std::sync::Arc::clone(&watch));
    let Ok(drawing) = watching::drawing(std::sync::Arc::clone(&watch));

    for check in ordered {
        let Ok(()) = watching::on(&watch, &ahead, check.name);
        let Ok(()) = watching::show_progress(&mut stage, &ahead, check.name);

        let started = Instant::now();
        let Ok(timing) = timing(check);
        let Ok(how) = checking::device(check, &mut stage);
        let Ok(()) = timed(timing, DEVICE, &how);
        let took = started.elapsed();
        let Ok(stop) = stopping::stop_state();

        match stop {
            Stop::Requested => {
                let Ok(()) = watching::quietly_handed(&watch, counted, |counted| {
                    let Ok(()) = say(counted, ink, check, How::Skipped("stopped".to_string()));
                });

                break;
            }
            Stop::No => {},
        }

        match &how {
            How::Ok => {
                let Ok(()) = lengths.learned(check.name, took);
            }
            How::Failed(_why) => {},
            How::Skipped(_answered) => {},
            How::Would => {},
        }

        let Ok(()) = ahead.finished(took);

        match &how {
            How::Failed(_why) => failed.push(check.name.to_string()),
            How::Ok => passed = passed.saturating_add(1),
            How::Skipped(_answered) => {}
            How::Would => {}
        }

        let Ok(()) = watching::quietly_handed(&watch, counted, |counted| {
            let Ok(()) = say(counted, ink, check, how);
        });
    }

    let Ok(()) = watching::ending(&watch);
    let _ = drawing.join();

    let Ok(()) = stopping::no_longer();

    match was {
        Some(was) => {
            let Ok(handed) = putting_back::back(&mut stage, &was);
            let Ok(said) = putting_back::summarize(&handed);
            let Ok(()) = watching::quietly(&watch, || {
                println!("{}{said}{}", ink.dim, ink.off);
            });
        }
        None => {},
    }

    let Ok(()) = watching::done_showing(&mut stage);
    let Ok(()) = lasting::keep(&mut stage, &lengths);
    let Ok(ended) = watching::ended(passed, &failed, began.elapsed(), card);
    let _ = watching::notify(&mut stage, &ended);

    match asked.dry_run {
        true => {
            println!("\n{}it would have run:{}", ink.yellow, ink.off);

            for command in &stage.done {
                println!("  {command}");
            }
        }
        false => {
            let Ok(()) = stage.close();
        }
    }

    Ok(())
}

fn here(check: &Check) -> Result<How, Unchecked> {
    let mut stage = Here::new()?;
    let Ok(timing) = timing(check);
    let Ok(how) = checking::here(check, &mut stage);
    let Ok(()) = timed(timing, HERE, &how);

    Ok(how)
}

fn timing(check: &Check) -> Result<Waiting, Never> {
    Waiting::here(Wait { who: "check", what: check.name })
}

fn timed(mut timing: Waiting, stage: &str, how: &How) -> Result<(), Never> {
    let Ok(went) = how.name();

    match how {
        How::Ok | How::Failed(_) => {
            let Ok(()) = timing.add_note(Note { name: "stage", said: stage });
            let Ok(()) = timing.add_note(Note { name: "went", said: went });

            timing.finish()
        }
        How::Skipped(_) | How::Would => Ok(()),
    }
}

fn run(asked: Arguments, ink: &Ansi) -> Result<std::process::ExitCode, Unchecked> {
    let Ok(checks) = select(&asked.only);

    match checks.is_empty() {
        true => return Err(Unchecked::NoSuchCheck),
        false => {}
    }

    match asked.list {
        true => {
            for check in &checks {
                println!("{:<28} {}", check.name, check.about);
            }

            return Ok(std::process::ExitCode::SUCCESS);
        }
        false => {}
    }

    let someones_machine = asked.stage == DEVICE && !(asked.yes || asked.dry_run);

    match someones_machine {
        true => {
            return Err(Unchecked::SomeonesMachine);
        }
        false => {}
    }

    let mut counted: BTreeMap<&'static str, u32> = BTreeMap::new();

    match asked.stage.as_str() {
        DEVICE => {
            on_the_device(&asked, checks, ink, &mut counted)?;
        }
        DESKTOP => {
            let Ok(mut stage) = Desktop::new();

            for check in checks {
                let Ok(timing) = timing(check);
                let Ok(how) = checking::desktop(check, &mut stage);
                let Ok(()) = timed(timing, DESKTOP, &how);

                let Ok(()) = say(&mut counted, ink, check, how);
            }

            let Ok(()) = stage.close();
        }
        _ => {
            let Ok(cores) = console_concurrency::Cores::counted();
            let ran = console_concurrency::map(cores, &checks, |check| here(check));
            let ran = ran.map_err(Unchecked::Concurrently)?;

            for (check, how) in checks.into_iter().zip(ran) {
                let how = how?;

                let Ok(()) = say(&mut counted, ink, check, how);
            }
        }
    }

    let many = |how: &str| match counted.get(how).copied() {
        Some(many) => many,
        None => NONE_OF_THEM,
    };
    let would = match many("would") {
        0 => String::new(),
        would => format!(", {would} would run"),
    };
    println!("\n{} ok, {} failed, {} skipped{would}", many("ok"), many("failed"), many("skipped"));

    let Ok(()) = console_response_times::flush();

    Ok(match many("failed") {
        0 => std::process::ExitCode::SUCCESS,
        _ => std::process::ExitCode::from(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_refuses_a_misspelt_dry_run_rather_than_running_on_the_device() {
        let read = parse_arguments(&["--stage", "device", "--dyr-run", "--yes"]);

        assert_eq!(
            read.map(|asked| asked.dry_run).map_err(|refusal| refusal.reason),
            Err(console_core_arguments::Reason::NoSuchFlag("--dyr-run".to_string()))
        );
    }

    #[test]
    fn the_line_asks_for_the_device_run_it_reads_as() -> Result<(), ValidationError> {
        let asked = parse_arguments(&["--stage", "device", "--dry-run", "brightness"])?;

        assert_eq!(asked.stage, DEVICE);
        assert_eq!((asked.dry_run, asked.yes, asked.all, asked.list), (true, false, false, false));
        assert_eq!(asked.only, ["brightness"]);

        Ok(())
    }

    #[test]
    fn with_no_stage_the_checks_run_here() -> Result<(), ValidationError> {
        let asked = parse_arguments::<&str>(&[])?;

        assert_eq!(asked.stage, HERE);

        Ok(())
    }
}
