//! Speak, and have it typed.
//!
//! One press starts listening and the next one writes down what was said, so
//! the same button is the whole of it: there is nothing to hold and nothing to
//! aim at, which is what a button on the back of a device has to be.
//!
//! **Both presses say what they cost.** The first is the machine getting a
//! microphone running, which is a wait someone stands through wondering
//! whether the button did anything; the second is the whole of the model's
//! work, which is the longest thing this desktop asks of the processor with a
//! person waiting on it. Neither number can be had by standing at a terminal
//! with a stopwatch -- the press is on the back of the device -- so both are
//! written down where they happen, and the second one names its stretches, so a
//! sentence that took a while is answered by which of the model, the recording
//! or the typing took it.
//!
//! The recording is what says which press this is. A file in the runtime
//! directory holds the microphone's own process, and a press that finds it
//! stops. Nothing is remembered anywhere else, so a session that ends in the
//! middle of a sentence leaves a desktop that is not listening rather than one
//! that thinks it still is.

use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use console_core_internal_programs::InternalProgram;
use console_program_lifetime::let_go;
use console_input_dictation::{
    VoiceActivity, detect_speech, cloning, compiling, configuring, fetching, hearing, languages, made, making,
    Note, model, recording, said, taken, taking, tidy, told_by, typing, whisper,
};
use console_core_external_programs::Program;
use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_response_times::{Wait, Waiting};
use console_waiting::{Schedule, Ready, until};
use rustix::process::{Pid, kill_process, Signal};
use std::path::PathBuf;

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    match asked.first().map(String::as_str) {
        Some("--fetch") => {
            match fetched() {
                Ok(()) => {},
                Err(why) => {
                    let Ok(()) = report("model", Failure {
                        summary: "Couldn't download the language",
                        body: &why.to_string(),
                    });
                }
            }

            match built() {
                Ok(()) => {},
                Err(why) => {
                    let Ok(()) = report("hearing", Failure {
                        summary: "Couldn't set up dictation",
                        body: &why.to_string(),
                    });
                }
            }

            ExitCode::SUCCESS
        }
        Some("--build") => {
            match built() {
                Ok(()) => {},
                Err(why) => {
                    let Ok(()) = report("hearing", Failure {
                        summary: "Couldn't set up dictation",
                        body: &why.to_string(),
                    });
                }
            }

            ExitCode::SUCCESS
        },
        Some(word) => {
            eprintln!("console-dictate: {word} is not a word this takes");

            ExitCode::from(2)
        }
        None => {
            let Ok(taken) = listening();

            match taken {
                Claimed::Yes => {
                    let Ok(()) = wrote_down();
                }
                Claimed::No => {
                    let Ok(()) = listen();
                }
            }

            ExitCode::SUCCESS
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Claimed {
    Yes,
    No,
}

fn listening() -> Result<Claimed, Never> {
    let Ok(holder) = holder();

    Ok(match holder.is_some() {
        true => Claimed::Yes,
        false => Claimed::No,
    })
}

fn holder() -> Result<Option<(i32, u32)>, Never> {
    let Ok(at) = taking();

    let Ok(held) = console_core_atomic_writes::read(&at);

    let note = match held {
        Stored::Text(note) => note,
        Stored::Absent => return Ok(None),

        Stored::Failed(fault) => {
            eprintln!("{}: reading who is holding the microphone: {fault}", at.display());
            return Ok(None);
        }
    };
    let Ok(told) = told_by(&note);

    let (pid, press) = match told {
        Some((pid, press)) => (pid, press),
        None => return Ok(None),
    };

    Ok(Path::new(&format!("/proc/{pid}")).exists().then_some((pid, press)))
}

fn listen() -> Result<(), Never> {
    let Ok(mut waiting) = Waiting::on(Wait { who: "console-dictate", what: "listening" });
    let press = std::process::id();
    let Ok(into) = said(press);

    match into.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {},
    }

    let Ok(arguments) = recording(&into);

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => {
            let Ok(()) = report("microphone", Failure {
                summary: "Couldn't record",
                body: "No recording app is installed.",
            });

            return Ok(());
        }
    };

    let mut starting = Command::new(program);
    starting.args(rest).stdin(Stdio::null()).stdout(Stdio::null());

    let started = let_go(&mut starting);

    match started {
        Err(why) => {
            let Ok(()) = report("microphone", Failure {
                summary: "Couldn't record",
                body: &why.to_string(),
            });
        }
        Ok(child) => {
            let Ok(id) = child.id();
            let Ok(at) = taking();
            let Ok(note) = taken(Note { recorder: id, press });
            let _ = console_core_atomic_writes::whole(&at, note.as_bytes());
            let Ok(()) = waiting.mark("microphone");
            let Ok(()) = told("Listening", UNTIL_IT_CHANGES);
            let Ok(()) = waiting.done();
            let Ok(()) = console_response_times::settled();
        }
    }

    Ok(())
}

fn wrote_down() -> Result<(), Never> {
    let Ok(mut waiting) = Waiting::on(Wait { who: "console-dictate", what: "writing it down" });
    let Ok(holder) = holder();

    let (pid, press) = match holder {
        Some((pid, press)) => (pid, press),
        None => return Ok(()),
    };

    match Pid::from_raw(pid) {
        Some(holding) => {
            let _ = kill_process(holding, Signal::INT);
        }
        None => return Ok(()),
    }

    let Ok(()) = gone(pid);
    let Ok(at) = taking();
    let _ = std::fs::remove_file(at);

    let Ok(()) = waiting.mark("stopped");
    let Ok(recorded) = said(press);
    let Ok(()) = read_out(&recorded, &mut waiting);
    let _ = std::fs::remove_file(&recorded);

    let Ok(()) = waiting.done();
    let Ok(()) = console_response_times::settled();

    Ok(())
}

fn read_out(recorded: &Path, waiting: &mut Waiting) -> Result<(), Never> {
    match fetched() {
        Ok(()) => {},
        Err(why) => {
            let Ok(()) = report("model", Failure {
                summary: "Couldn't download the language",
                body: &why.to_string(),
            });

            return Ok(());
        }
    }

    let Ok(()) = waiting.mark("model");
    let Ok(()) = told("Transcribing…", UNTIL_IT_CHANGES);

    match heard(recorded) {
        Err(why) => {
            let Ok(()) = told("Couldn't understand that", BRIEFLY);
            let Ok(()) = report("hearing", Failure {
                summary: "Couldn't understand that",
                body: &why.to_string(),
            });
        }
        Ok(words) => match words.is_empty() {
            true => {
                let Ok(()) = waiting.mark("heard");
                let Ok(()) = told("No speech detected", BRIEFLY);
            }
            false => {
                let Ok(()) = waiting.mark("heard");
                let Ok(letters) = console_core_number_conversion::fitted::<_, u64>(words.chars().count());
                let Ok(()) = waiting.counted("letters", letters);
                let Ok(()) = write(&words);
                let Ok(()) = waiting.mark("typed");
                let Ok(()) = told(&words, BRIEFLY);
            }
        },
    }

    Ok(())
}

fn gone(pid: i32) -> Result<(), Never> {
    let at = format!("/proc/{pid}");
    let Ok(patience) = Schedule::asking_every(LEAVING, std::time::Duration::from_millis(20));
    let Ok(_went) = until(patience, || {
        Ok(match Path::new(&at).exists() {
            true => Ready::NotYet,
            false => Ready::Yes,
        })
    });

    Ok(())
}

#[derive(Debug)]
enum DictationError {
    Machine(std::io::Error),
    NoModel,
    NoProgram(&'static str),
    CommandFailed(&'static str, std::process::ExitStatus),
    Nowhere,
}

impl std::fmt::Display for DictationError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DictationError::Machine(fault) => write!(to, "{fault}"),
            DictationError::NoModel => write!(to, "nothing says where the model is kept"),
            DictationError::NoProgram(what) => write!(to, "there is no program to {what} with"),
            DictationError::CommandFailed(name, status) => write!(to, "{name} said no: {status}"),
            DictationError::Nowhere => write!(to, "nowhere to keep it"),
        }
    }
}

impl std::error::Error for DictationError {}

fn heard(recorded: &Path) -> Result<String, DictationError> {
    let wav = std::fs::read(recorded).map_err(DictationError::Machine)?;

    let Ok(heard) = detect_speech(&wav);

    match heard {
        VoiceActivity::Silence => return Ok(String::new()),
        VoiceActivity::Speech => {},
    }

    let Ok(engine) = engine();
    let Ok(model) = model();

    let model = match model {
        Some(model) => model,
        None => return Err(DictationError::NoModel),
    };

    let Ok(language) = languages::chosen();
    let Ok(arguments) = hearing(&engine, &model, recorded, &language);

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Err(DictationError::NoProgram("hear")),
    };

    let answered = Command::new(program)
        .args(rest)
        .stderr(Stdio::null())
        .output()
        .map_err(DictationError::Machine)?;

    match answered.status.success() {
        true => {},
        false => return Err(DictationError::CommandFailed("whisper-cli", answered.status)),
    }

    let Ok(tidy) = tidy(&String::from_utf8_lossy(&answered.stdout));

    Ok(tidy)
}

fn write(words: &str) -> Result<(), Never> {
    let Ok(arguments) = typing(words);

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => {
            let Ok(()) = report("typing", Failure {
                summary: "Couldn't type the words",
                body: "No typing app is installed.",
            });

            return Ok(());
        }
    };

    match Command::new(program).args(rest).status() {
        Err(why) => {
            let Ok(()) = report("typing", Failure {
                summary: "Couldn't type the words",
                body: &why.to_string(),
            });
        }
        Ok(status) => match status.success() {
            true => (),
            false => {
                let Ok(()) = report("typing", Failure {
                    summary: "Couldn't type the words",
                    body: &status.to_string(),
                });
            }
        },
    }

    Ok(())
}

fn engine() -> Result<PathBuf, Never> {
    let Ok(ours) = whisper();

    match ours {
        Some(ours) => match ours.exists() {
            true => return Ok(ours),
            false => {},
        },
        None => {},
    }

    let Ok(mut building) = InternalProgram::Dictate.command();
    building.arg("--build").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

    let _ = let_go(&mut building);

    let Ok(whisper_cli) = Program::WhisperCli.name();

    Ok(PathBuf::from(whisper_cli))
}

#[derive(Debug)]
enum BuildError {
    NoWhisper,
    Nowhere,
    Machine(std::io::Error),
    NoStep,
    NoProgram(String, std::io::Error),
    CommandFailed(String, std::process::ExitStatus),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NoWhisper => write!(to, "nothing says where whisper is kept"),
            BuildError::Nowhere => write!(to, "nowhere to keep it"),
            BuildError::Machine(fault) => write!(to, "{fault}"),
            BuildError::NoStep => write!(to, "a step of the build named no program to run"),
            BuildError::NoProgram(program, fault) => {
                write!(to, "{program} could not be run: {fault}")
            }
            BuildError::CommandFailed(program, status) => write!(to, "{program} said no: {status}"),
        }
    }
}

impl std::error::Error for BuildError {}

fn built() -> Result<(), BuildError> {
    let Ok(ours) = whisper();

    let ours = match ours {
        Some(ours) => ours,
        None => return Err(BuildError::NoWhisper),
    };

    match ours.exists() {
        true => return Ok(()),
        false => {},
    }

    let parent = match ours.parent() {
        Some(parent) => parent,
        None => return Err(BuildError::Nowhere),
    };

    std::fs::create_dir_all(parent).map_err(BuildError::Machine)?;

    let alone = parent.join("building.lock");

    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "the lock one build of the model takes, whose whole point is that making it fails when someone else has: a file written beside and renamed over would succeed for both of them"
        )
    )]
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&alone) {
        Ok(_) => {}
        Err(fault) => match fault.kind() == std::io::ErrorKind::AlreadyExists {
            true => return Ok(()),
            false => {
                eprintln!("{}: {fault}", alone.display());
                return Ok(());
            }
        },
    }

    let answer = build(&ours);
    let _ = std::fs::remove_file(&alone);
    answer
}

fn build(ours: &Path) -> Result<(), BuildError> {
    let Ok(at) = making();
    let _ = std::fs::remove_dir_all(&at);

    match at.parent() {
        Some(parent) => std::fs::create_dir_all(parent).map_err(BuildError::Machine)?,
        None => {},
    }

    let Ok(()) = told("Setting up dictation…", UNTIL_IT_CHANGES);
    let Ok(cloning) = cloning(&at);
    let Ok(configuring) = configuring(&at);
    let Ok(compiling) = compiling(&at);

    for arguments in [cloning, configuring, compiling] {
        let (program, rest) = match arguments.split_first() {
            Some((program, rest)) => (program, rest),
            None => return Err(BuildError::NoStep),
        };

        let answered = Command::new(program)
            .args(rest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|why| BuildError::NoProgram(program.clone(), why))?;

        match answered.success() {
            true => {},
            false => {
                let _ = std::fs::remove_dir_all(&at);
                return Err(BuildError::CommandFailed(program.clone(), answered));
            }
        }
    }

    let coming = ours.with_extension("coming");
    let Ok(made) = made(&at);

    std::fs::copy(made, &coming).map_err(BuildError::Machine)?;
    std::fs::rename(&coming, ours).map_err(BuildError::Machine)?;

    let _ = std::fs::remove_dir_all(&at);
    let Ok(()) = told("Dictation is ready", BRIEFLY);

    Ok(())
}

fn fetched() -> Result<(), DictationError> {
    let Ok(model) = model();

    let model = match model {
        Some(model) => model,
        None => return Err(DictationError::NoModel),
    };

    match model.exists() {
        true => return Ok(()),
        false => {},
    }

    let parent = match model.parent() {
        Some(parent) => parent,
        None => return Err(DictationError::Nowhere),
    };

    std::fs::create_dir_all(parent).map_err(DictationError::Machine)?;

    let Ok(()) = told("Downloading the language…", UNTIL_IT_CHANGES);
    let coming = parent.join("coming.bin");
    let Ok(arguments) = fetching(&coming);

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Err(DictationError::NoProgram("fetch the words")),
    };

    let answered = Command::new(program)
        .args(rest)
        .status()
        .map_err(DictationError::Machine)?;

    match answered.success() {
        true => {},
        false => {
            let _ = std::fs::remove_file(&coming);
            return Err(DictationError::CommandFailed("curl", answered));
        }
    }

    std::fs::rename(&coming, &model).map_err(DictationError::Machine)
}

const LEAVING: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Until(u32);

const UNTIL_IT_CHANGES: Until = Until(0);

const BRIEFLY: Until = Until(2_000);

fn told(what: &str, until: Until) -> Result<(), Never> {
    let Ok(mut saying) = Program::NotifySend.command();

    saying
        .args([
            "--app-name=Console",
            "--urgency=low",
            &format!("--expire-time={}", until.0),
            "--hint=string:x-canonical-private-synchronous:console-dictate",
            "--icon=audio-input-microphone",
            "--",
            what,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let _ = let_go(&mut saying);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failure<'a> {
    summary: &'a str,
    body: &'a str,
}

fn report(kind: &str, fault: Failure<'_>) -> Result<(), Never> {
    let Failure { summary, body } = fault;

    eprintln!("console-dictate: {summary}: {body}");

    let Ok(mut saying) = InternalProgram::ConsoleSay.command();
    saying.args([kind, summary, body]).stdin(Stdio::null());

    let _ = let_go(&mut saying);

    Ok(())
}
