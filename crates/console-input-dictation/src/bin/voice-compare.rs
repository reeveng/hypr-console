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

use console_program_lifetime::{Alongside, alongside};
use console_input_dictation::comparing::{Compare, Found, Heard, Its, Seen, Took};
use console_core_external_programs::Program as Theirs;
use console_core_atomic_writes::{Held, read};
use console_core_never::Never;
use console_program_contract::{Argv, Word};
use console_program_runtime::Carrying;

const TRIES: [u8; 3] = [1, 2, 3];

const NAMED: &str = "/proc/sys/kernel/hostname";

struct Machine {
    recording: Option<Alongside>,
}

impl Carrying for Machine {
    type Hears = Heard;
    type Does = Its;

    fn its(&mut self, doing: &Its) -> Vec<Word<Heard>> {
        match doing {
            Its::Look(every) => {
                let seen = every
                    .iter()
                    .map(|at| {
                        let Ok(found) = found(at);

                        Seen { at: at.clone(), is: found }
                    })
                    .collect();

                vec![Word::Its(Heard::Looked(seen))]
            }

            Its::Ran(argv) => {
                let Ok(said) = said(argv);

                vec![Word::Its(Heard::Said(said))]
            }

            Its::Timed(argv) => {
                let Ok(took) = timed(argv);

                vec![Word::Its(Heard::Took(took))]
            }

            Its::Listen(into) => {
                let Ok(mut recording) = Theirs::PwRecord.command();

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

                vec![Word::Its(Heard::Done)]
            }

            Its::Enough => {
                self.recording = None;

                vec![Word::Its(Heard::Done)]
            }
        }
    }
}

fn found(at: &std::path::Path) -> Result<Found, Never> {
    use std::os::unix::fs::PermissionsExt;

    Ok(match std::fs::metadata(at) {
        Err(_) => Found::Missing,
        Ok(what) => match (what.is_file(), what.permissions().mode() & 0o111) {
            (true, 0) | (false, _) => Found::There,
            (true, _) => Found::Runnable,
        },
    })
}

fn starting(argv: &[String]) -> Result<Option<Command>, Never> {
    let Some(first) = argv.first() else { return Ok(None) };

    let mut starting = Command::new(first);

    starting.args(argv.iter().skip(1));

    Ok(Some(starting))
}

fn said(argv: &[String]) -> Result<String, Never> {
    let Ok(starting) = starting(argv);

    Ok(match starting {
        None => String::new(),
        Some(mut starting) => match starting.stderr(Stdio::piped()).output() {
            Err(_) => String::new(),
            Ok(done) => format!(
                "{}{}",
                String::from_utf8_lossy(&done.stdout),
                String::from_utf8_lossy(&done.stderr)
            ),
        },
    })
}

fn timed(argv: &[String]) -> Result<Took, Never> {
    let mut first = None;
    let mut best = None;
    let mut heard = String::new();

    for _ in TRIES {
        let began = Instant::now();

        let Ok(starting) = starting(argv);

        heard = match starting {
            None => String::new(),
            Some(mut starting) => match starting.stderr(Stdio::null()).output() {
                Err(_) => String::new(),
                Ok(done) => String::from_utf8_lossy(&done.stdout).to_string(),
            },
        };

        let took = began.elapsed().as_millis();

        first = first.or(Some(took));
        best = Some(best.map_or(took, |best: u128| best.min(took)));
    }

    Ok(Took { first: first.unwrap_or_default(), best: best.unwrap_or_default(), said: heard })
}

fn here() -> Result<String, Never> {
    let Ok(held) = read(std::path::Path::new(NAMED));

    Ok(match held {
        Held::Said(said) => said.trim().to_string(),
        Held::Nothing => String::new(),
        Held::Unreadable(fault) => {
            eprintln!("voice-compare: {NAMED}: {fault}");

            String::new()
        }
    })
}

fn stamped() -> Result<String, Never> {
    let Ok(mut asking) = Theirs::Date.command();

    let said = asking.arg("+%Y-%m-%d-%H%M").output();

    Ok(match said {
        Ok(said) => String::from_utf8_lossy(&said.stdout).trim().to_string(),
        Err(_) => String::new(),
    })
}

fn main() -> ExitCode {
    let Ok(kept) = console_input_dictation::kept();
    let Ok(here) = here();
    let Ok(stamped) = stamped();
    let mut words = vec![kept.to_string_lossy().to_string(), here, stamped];

    words.extend(std::env::args().skip(1));

    let given: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut machine = Machine { recording: None };

    let Ok(argv) = Argv::of(&given);
    let Ok(code) =
        console_program_runtime::run::<Compare, Machine>("voice-compare", &argv, &mut machine);

    code
}
