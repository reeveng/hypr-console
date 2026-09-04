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
//! It is also the slow stage, and most of what is written for it was already
//! answered here a second earlier. So asked for nothing in particular, the
//! machine is asked only what nothing else can answer, and says of the rest
//! where it was answered instead. --all is the whole tier for when the answer
//! wanted is about the hardware rather than the desktop. Naming a check is
//! asking for it: `--stage device brightness` runs brightness there whatever
//! the emulator thinks.

use std::collections::BTreeMap;
use std::io::IsTerminal;

use console_checks::chosen;
use console_stage::checking::{self, Check, How};
use console_stage::desktop::Desktop;
use console_stage::device::{self, Device, Dry};
use console_stage::here::Here;

/// What was asked for on the command line.
struct Asked {
    only: Vec<String>,
    stage: String,
    list: bool,
    dry: bool,
    yes: bool,
    all: bool,
}

fn asked(words: Vec<String>) -> Asked {
    let said = |what: &str| words.iter().any(|word| word == what);
    let after = |what: &str| {
        words.iter().position(|word| word == what).and_then(|at| words.get(at + 1)).cloned()
    };
    Asked {
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
    }
}

/// The ink, where there is somebody to read it.
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
    fn mark(&self, how: &How) -> String {
        let (colour, said) = match how {
            How::Ok => (self.green, "ok"),
            How::Failed(_) => (self.red, "failed"),
            How::Skipped(_) => (self.dim, "skipped"),
            How::Would => (self.yellow, "would run"),
        };
        format!("{colour}{said:<9}{}", self.off)
    }
}

fn main() -> std::process::ExitCode {
    let ink = match std::io::stdout().is_terminal() {
        true => COLOURED,
        false => PLAIN,
    };

    match run(asked(std::env::args().skip(1).collect()), &ink) {
        Ok(code) => code,
        Err(why) => {
            eprintln!("{why}");
            std::process::ExitCode::from(1)
        }
    }
}

/// One check on the machine, unless somewhere cheaper is written for it.
///
/// The tier is minutes of somebody's handheld, and most of it is a second
/// question about a feature the emulator answered before the deploy went out.
/// What is left is what only the hardware knows. A spared check still prints,
/// because a line that says where it was answered is the difference between a
/// short tier and a tier with holes nobody can see.
fn spared(check: &Check, stage: &mut Device) -> How {
    match check.without_the_device() {
        None => checking::device(check, stage),
        Some(where_) => How::Skipped(format!("{} answers this", where_.name())),
    }
}

fn run(asked: Asked, ink: &Ink) -> Result<std::process::ExitCode, String> {
    let checks = chosen(&asked.only);

    if checks.is_empty() {
        return Err("no checks by that name".to_string());
    }

    if asked.list {
        for check in checks {
            println!("{:<28} {}", check.name, check.about);
        }

        return Ok(std::process::ExitCode::SUCCESS);
    }

    if asked.stage == "device" && !(asked.yes || asked.dry) {
        return Err("that is somebody's machine. Add --dry to see what would happen, \
                    or --yes to do it."
            .to_string());
    }

    let mut counted: BTreeMap<&str, usize> = BTreeMap::new();
    let mut said = |check: &Check, how: How| {
        *counted.entry(how.name()).or_insert(0) += 1;
        let why = how.why();
        let aside =
            match why.is_empty() {
                true => String::new(),
                false => format!("{}{why}{}", ink.dim, ink.off),
            };
        println!("{:<28} {} {aside}", check.name, ink.mark(&how));
    };

    match asked.stage.as_str() {
        "device" => {
            let touching = match asked.dry {
                true => Dry::Pretend,
                false => Dry::Really,
            };
            let mut stage = Device::new(&device::host()?, touching)?;
            let whole = asked.all || !asked.only.is_empty();

            for check in checks {
                said(check, match whole {
                    true => checking::device(check, &mut stage),
                    false => spared(check, &mut stage),
                });
            }

            if asked.dry {
                println!("\n{}it would have run:{}", ink.yellow, ink.off);

                for command in &stage.done {
                    println!("  {command}");
                }
            } else {
                stage.close();
            }
        }
        "desktop" => {
            let mut stage = Desktop::new();

            for check in checks {
                said(check, checking::desktop(check, &mut stage));
            }

            stage.close();
        }
        // Here is cheap and holds nothing anybody else wants, so each check gets
        // one of its own rather than a stage the check before it lived in.
        _ => {
            for check in checks {
                let mut stage = Here::new()?;
                said(check, checking::here(check, &mut stage));
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
