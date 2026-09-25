//! Which hearing this device should be using, measured on this device.
//!
//!     voice-compare --record     say the sixteen clips, once
//!     voice-compare --models     fetch the models
//!     voice-compare --build      build the second engine, which is slow
//!     voice-compare --fetch      both of those
//!     voice-compare              read every clip with every model
//!  This runs ON the handheld, as the person whose session it is -- not as root
//! over ssh. Recording needs the microphone, and the microphone belongs to a
//! PipeWire that belongs to a session.  What is here is everything that touches
//! this machine, and it is the three effects
//! `console_input_dictation::comparing` decided but could not name: engines
//! that are binaries at paths rather than programs on the machine, a run that
//! has to be timed rather than only answered, and a recording that one press
//! begins and the next one ends.

use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

use console_program_lifetime::{BoundToParent, alongside};
use console_input_dictation::comparing::{Compare, Found, CompareEvent, CompareEffect, Candidate, Timing};
use console_core_external_programs::Program as ExternalProgram;
use console_core_atomic_writes::{Stored, read};
use console_core_never::Never;
use console_program_contract::{Arguments, Event};
use console_program_runtime::Interpreter;

const NOTHING_TIMED: u128 = 0;


const TRIES: [u8; 3] = [1, 2, 3];

const NAMED: &str = "/proc/sys/kernel/hostname";

struct Machine {
    recording: Option<BoundToParent>,
}

impl Interpreter for Machine {
    type Event = CompareEvent;
    type Effect = CompareEffect;

    fn interpret(&mut self, acts: &CompareEffect) -> Vec<Event<CompareEvent>> {
        match acts {
            CompareEffect::Look(every) => {
                let seen = every
                    .iter()
                    .map(|at| {
                        let Ok(found) = found(at);

                        Candidate { at: at.clone(), is: found }
                    })
                    .collect();

                vec![Event::Custom(CompareEvent::Looked(seen))]
            }

            CompareEffect::Ran(arguments) => {
                let Ok(said) = said(arguments);

                vec![Event::Custom(CompareEvent::Output(said))]
            }

            CompareEffect::Timed(arguments) => {
                let Ok(took) = timed(arguments);

                vec![Event::Custom(CompareEvent::Timed(took))]
            }

            CompareEffect::Record(into) => {
                let Ok(mut recording) = ExternalProgram::PwRecord.command();

                recording
                    .args(["--rate", "16000", "--channels", "1", "--format", "s16"])
                    .arg(into)
                    .stdout(Stdio::null());

                self.recording = match alongside(&mut recording) {
                    Ok(started) => Some(started),
                    Err(fault) => {
                        eprintln!("voice-compare: nothing is listening: {fault}");

                        None
                    }
                };

                vec![Event::Custom(CompareEvent::Finished)]
            }

            CompareEffect::Enough => {
                self.recording = None;

                vec![Event::Custom(CompareEvent::Finished)]
            }
        }
    }
}

fn found(at: &std::path::Path) -> Result<Found, Never> {
    use std::os::unix::fs::PermissionsExt;

    Ok(match std::fs::metadata(at) {
        Err(_not_there) => Found::Absent,
        Ok(what) => match (what.is_file(), what.permissions().mode() & 0o111) {
            (true, 0) | (false, _) => Found::There,
            (true, _) => Found::Runnable,
        },
    })
}

fn starting(arguments: &[String]) -> Result<Option<Command>, Never> {
    let first = match arguments.first() {
        Some(first) => first,
        None => return Ok(None),
    };

    let mut starting = Command::new(first);

    starting.args(arguments.iter().skip(1));

    Ok(Some(starting))
}

fn said(arguments: &[String]) -> Result<String, Never> {
    let Ok(starting) = starting(arguments);

    Ok(match starting {
        None => String::new(),
        Some(mut starting) => match starting.stderr(Stdio::piped()).output() {
            Err(_would_not_start) => String::new(),
            Ok(done) => format!(
                "{}{}",
                String::from_utf8_lossy(&done.stdout),
                String::from_utf8_lossy(&done.stderr)
            ),
        },
    })
}

fn timed(arguments: &[String]) -> Result<Timing, Never> {
    let mut first = None;
    let mut best = None;
    let mut heard = String::new();

    for _ in TRIES {
        let began = Instant::now();

        let Ok(starting) = starting(arguments);

        heard = match starting {
            None => String::new(),
            Some(mut starting) => match starting.stderr(Stdio::null()).output() {
                Err(_would_not_start) => String::new(),
                Ok(done) => String::from_utf8_lossy(&done.stdout).to_string(),
            },
        };

        let took = began.elapsed().as_millis();

        first = first.or(Some(took));
        best = Some(best.map_or(took, |best: u128| best.min(took)));
    }

    let first = match first {
        Some(first) => first,
        None => NOTHING_TIMED,
    };

    let best = match best {
        Some(best) => best,
        None => NOTHING_TIMED,
    };

    Ok(Timing { first, best, said: heard })
}

fn here() -> Result<String, Never> {
    let Ok(held) = read(std::path::Path::new(NAMED));

    Ok(match held {
        Stored::Text(said) => said.trim().to_string(),
        Stored::Absent => String::new(),
        Stored::Failed(fault) => {
            eprintln!("voice-compare: {NAMED}: {fault}");

            String::new()
        }
    })
}

fn stamped() -> Result<String, Never> {
    let Ok(asking) = ExternalProgram::Date.arguments(&["+%Y-%m-%d-%H%M"]);

    Ok(match console_core_external_programs::printed(&asking) {
        Ok(said) => said.trim().to_string(),
        Err(_unprinted) => String::new(),
    })
}

fn main() -> ExitCode {
    let Ok(kept) = console_input_dictation::kept();

    let kept = match kept {
        Some(kept) => kept,
        None => {
            eprintln!("voice-compare: nothing says where the recordings are kept");

            return ExitCode::FAILURE;
        }
    };

    let Ok(here) = here();
    let Ok(stamped) = stamped();
    let mut words = vec![kept.to_string_lossy().to_string(), here, stamped];

    words.extend(std::env::args().skip(1));

    let given: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut machine = Machine { recording: None };

    let Ok(arguments) = Arguments::of(&given);
    let Ok(code) =
        console_program_runtime::run::<Compare, Machine>("voice-compare", &arguments, &mut machine);

    code
}
