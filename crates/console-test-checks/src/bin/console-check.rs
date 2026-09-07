//! Everything this desktop has grown, tried again, oldest first.
//!
//! ```text
//! console-check                          here, against the emulator
//! console-check --list                   what there is, and what each is
//! console-check brightness               only the checks about that
//! console-check --stage device --dry     what it would do to the device
//! console-check --stage device --yes     do it
//! console-check --stage device --yes --all   every check written for it
//! ```
//!
//! The device is the last stage and it is somebody's machine. Nothing is sent to
//! it without --yes, and --dry prints every command first so it can be read
//! before it is run. The pressing goes through InputPlumber's own SendEvent,
//! which is how the hardware's own buttons arrive, so nothing is created on the
//! device and nothing is left behind if this stops halfway.
//!
//! It is somebody's machine in the other sense too, so the run gives it back
//! the way it found it. What was true before the first press -- the workspace,
//! the brightness, the volume, the profile -- is read once and put back after
//! the last check, along with anything the run opened, and the last line says
//! what was handed back. Ctrl-C is answered rather than fatal for the same
//! reason: a run stopped halfway is the one that would otherwise leave the
//! most behind. See `console_test_stages::putting_back`.
//!
//! It is also the slow stage, and most of what is written for it was already
//! answered here a second earlier. So asked for nothing in particular, the
//! machine is asked only what nothing else can answer, and says of the rest
//! where it was answered instead. --all is the whole tier for when the answer
//! wanted is about the hardware rather than the desktop. Naming a check is
//! asking for it: `--stage device brightness` runs brightness there whatever
//! the emulator thinks.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::time::Instant;

use console_test_checks::chosen;
use console_core_never::Never;
use console_test_stages::checking::{self, Check, How, Stage};
use console_test_stages::desktop::Desktop;
use console_test_stages::device::{self, Device, Dry};
use console_test_stages::lasting::{self, Ahead};
use console_test_stages::putting_back;
use console_test_stages::stopping::{self, Stop};
use console_test_stages::watching;
use console_test_stages::here::Here;

struct Asked {
    only: Vec<String>,
    stage: String,
    list: bool,
    dry: bool,
    yes: bool,
    all: bool,
}

fn asked(words: Vec<String>) -> Result<Asked, Never> {
    let said = |what: &str| words.iter().any(|word| word == what);
    let after = |what: &str| {
        words
            .iter()
            .position(|word| word == what)
            .and_then(|at| words.get(at.saturating_add(1)))
            .cloned()
    };
    Ok(Asked {
        only: words
            .iter()
            .filter(|word| !word.starts_with("--"))
            .filter(|word| Some(word.to_string()) != after("--stage"))
            .cloned()
            .collect(),
        stage: after("--stage").unwrap_or_else(|| "here".to_string()),
        list: said("--list"),
        dry: said("--dry"),
        yes: said("--yes"),
        all: said("--all"),
    })
}

struct Ink {
    green: &'static str,
    red: &'static str,
    dim: &'static str,
    yellow: &'static str,
    off: &'static str,
}

const COLOURED: Ink =
    Ink { green: "\x1b[32m", red: "\x1b[31m", dim: "\x1b[2m", yellow: "\x1b[33m", off: "\x1b[0m" };
const PLAIN: Ink = Ink { green: "", red: "", dim: "", yellow: "", off: "" };

impl Ink {
    fn mark(&self, how: &How) -> Result<String, Never> {
        let (colour, said) = match how {
            How::Ok => (self.green, "ok"),
            How::Failed(_) => (self.red, "failed"),
            How::Skipped(_) => (self.dim, "skipped"),
            How::Would => (self.yellow, "would run"),
        };
        Ok(format!("{colour}{said:<9}{}", self.off))
    }
}

fn main() -> std::process::ExitCode {
    let ink = match std::io::stdout().is_terminal() {
        true => COLOURED,
        false => PLAIN,
    };

    let Ok(asked) = asked(std::env::args().skip(1).collect());

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

fn on_the_device(
    asked: &Asked,
    checks: Vec<&'static Check>,
    ink: &Ink,
    said: &mut dyn FnMut(&Check, How),
) -> Result<(), String> {
    let touching = match asked.dry {
        true => Dry::Pretend,
        false => Dry::Really,
    };
    let host = device::host()?;
    let mut stage = Device::new(&host, touching)?;
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

                said(check, How::Skipped(format!("{name} answers this")));
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

    let was = match asked.dry {
        true => None,
        false => {
            let Ok(was) = putting_back::found(&mut stage);

            Some(was)
        }
    };
    let Ok(starting) = watching::starting(ordered.len(), &ahead);
    let Ok(card) = watching::said(&mut stage, &starting);
    let began = Instant::now();
    let mut passed: usize = 0;
    let mut failed: Vec<String> = Vec::new();
    let Ok(watch) = watching::Watching::of(ordered.len());
    let watch = std::sync::Arc::new(std::sync::Mutex::new(watch));
    let Ok(()) = stage.watching(std::sync::Arc::clone(&watch));
    let Ok(drawing) = watching::drawing(std::sync::Arc::clone(&watch));

    for check in ordered {
        let Ok(()) = watching::on(&watch, &ahead, check.name);
        let Ok(()) = watching::showing(&mut stage, &ahead, check.name);

        let started = Instant::now();
        let Ok(how) = checking::device(check, &mut stage);
        let took = started.elapsed();
        let Ok(stop) = stopping::asked();

        match stop {
            Stop::Asked => {
                let Ok(()) = watching::quietly(&watch, || {
                    said(check, How::Skipped("stopped".to_string()));
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

        let Ok(()) = watching::quietly(&watch, || said(check, how));
    }

    let Ok(()) = watching::ending(&watch);
    let _ = drawing.join();

    let Ok(()) = stopping::no_longer();

    match was {
        Some(was) => {
            let Ok(handed) = putting_back::back(&mut stage, &was);
            let Ok(said) = putting_back::said(&handed);

            println!("{}{said}{}", ink.dim, ink.off);
        }
        None => {},
    }

    let Ok(()) = watching::done_showing(&mut stage);
    let Ok(()) = lasting::keep(&mut stage, &lengths);
    let Ok(ended) = watching::ended(passed, &failed, began.elapsed(), card);
    let _ = watching::said(&mut stage, &ended);

    match asked.dry {
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

fn run(asked: Asked, ink: &Ink) -> Result<std::process::ExitCode, String> {
    let Ok(checks) = chosen(&asked.only);

    match checks.is_empty() {
        true => return Err("no checks by that name".to_string()),
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

    let somebodys_machine = asked.stage == "device" && !(asked.yes || asked.dry);

    match somebodys_machine {
        true => {
            return Err("that is somebody's machine. Add --dry to see what would happen, \
                        or --yes to do it."
                .to_string());
        }
        false => {}
    }

    let mut counted: BTreeMap<&str, usize> = BTreeMap::new();
    let mut said = |check: &Check, how: How| {
        let Ok(name) = how.name();
        let Ok(why) = how.why();

        let tally = counted.entry(name).or_insert(0);
        *tally = tally.saturating_add(1);

        let aside =
            match why.is_empty() {
                true => String::new(),
                false => format!("{}{why}{}", ink.dim, ink.off),
            };
        let Ok(mark) = ink.mark(&how);

        println!("{:<28} {mark} {aside}", check.name);
    };

    match asked.stage.as_str() {
        "device" => {
            on_the_device(&asked, checks, ink, &mut said)?;
        }
        "desktop" => {
            let Ok(mut stage) = Desktop::new();

            for check in checks {
                let Ok(how) = checking::desktop(check, &mut stage);

                said(check, how);
            }

            let Ok(()) = stage.close();
        }
        _ => {
            for check in checks {
                let mut stage = Here::new()?;
                let Ok(how) = checking::here(check, &mut stage);

                said(check, how);
            }
        }
    }

    let many = |how: &str| counted.get(how).copied().unwrap_or_default();
    let would = match many("would") {
        0 => String::new(),
        would => format!(", {would} would run"),
    };
    println!("\n{} ok, {} failed, {} skipped{would}", many("ok"), many("failed"), many("skipped"));
    Ok(match many("failed") {
        0 => std::process::ExitCode::SUCCESS,
        _ => std::process::ExitCode::from(1),
    })
}
