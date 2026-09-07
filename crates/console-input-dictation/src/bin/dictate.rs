//! Speak, and have it typed.
//!
//! One press starts listening and the next one writes down what was said, so
//! the same button is the whole of it: there is nothing to hold and nothing to
//! aim at, which is what a button on the back of a device has to be.
//!
//! The recording is what says which press this is. A file in the runtime
//! directory holds the microphone's own process, and a press that finds it
//! stops. Nothing is remembered anywhere else, so a session that ends in the
//! middle of a sentence leaves a desktop that is not listening rather than one
//! that thinks it still is.

use std::path::Path;
use std::process::{Command, Stdio};

use console_program_lifetime::let_go;
use console_input_dictation::{
    Heard, anything_said, cloning, compiling, configuring, fetching, hearing, languages, made, making,
    model, recording, said, taken, taking, tidy, told_by, typing, whisper,
};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_waiting::{Patience, Seen, until};
use std::path::PathBuf;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    match asked.first().map(String::as_str) {
        Some("--fetch") => {
            match fetched() {
                Ok(()) => {},
                Err(why) => {
                    let Ok(()) = fell("model", "Couldn't download the language", &why);
                }
            }

            match built() {
                Ok(()) => {},
                Err(why) => {
                    let Ok(()) = fell("hearing", "Couldn't set up dictation", &why);
                }
            }
        }
        Some("--build") => match built() {
            Ok(()) => {},
            Err(why) => {
                let Ok(()) = fell("hearing", "Couldn't set up dictation", &why);
            }
        },
        Some(word) => {
            eprintln!("dictate: {word} is not a word this takes");
            std::process::exit(2);
        }
        None => {
            let Ok(taken) = listening();

            match taken {
                Taken::Yes => {
                    let Ok(()) = wrote_down();
                }
                Taken::No => {
                    let Ok(()) = listen();
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Taken {
    Yes,
    No,
}

fn listening() -> Result<Taken, Never> {
    let Ok(holder) = holder();

    Ok(match holder.is_some() {
        true => Taken::Yes,
        false => Taken::No,
    })
}

fn holder() -> Result<Option<(i32, u32)>, Never> {
    let Ok(at) = taking();

    let note = match std::fs::read_to_string(&at) {
        Ok(note) => note,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return Ok(None),

        Err(fault) => {
            eprintln!("{}: reading who is holding the microphone: {fault}", at.display());
            return Ok(None);
        }
    };
    let Ok(told) = told_by(&note);

    let Some((pid, press)) = told else { return Ok(None) };

    Ok(Path::new(&format!("/proc/{pid}")).exists().then_some((pid, press)))
}

fn listen() -> Result<(), Never> {
    let press = std::process::id();
    let Ok(into) = said(press);

    match into.parent() {
        Some(parent) => {
            let _ = std::fs::create_dir_all(parent);
        }
        None => {},
    }

    let Ok(argv) = recording(&into);

    let Some((program, rest)) = argv.split_first() else {
        let Ok(()) = fell("microphone", "Couldn't record", "there is no program to record with");

        return Ok(());
    };

    let mut starting = Command::new(program);
    starting.args(rest).stdin(Stdio::null()).stdout(Stdio::null());

    let started = let_go(&mut starting);

    match started {
        Err(why) => {
            let Ok(()) = fell("microphone", "Couldn't record", &why.to_string());
        }
        Ok(child) => {
            let Ok(id) = child.id();
            let Ok(at) = taking();
            let Ok(note) = taken(id, press);
            let _ = std::fs::write(at, note);
            let Ok(()) = told("Listening", UNTIL_IT_CHANGES);
        }
    }

    Ok(())
}

fn wrote_down() -> Result<(), Never> {
    let Ok(holder) = holder();

    let Some((pid, press)) = holder else { return Ok(()) };

    // SAFETY: a signal to a pid this desktop started and has not reaped.
    unsafe { libc::kill(pid, libc::SIGINT) };

    let Ok(()) = gone(pid);
    let Ok(at) = taking();
    let _ = std::fs::remove_file(at);

    let Ok(recorded) = said(press);
    let Ok(()) = read_out(&recorded);
    let _ = std::fs::remove_file(&recorded);

    Ok(())
}

fn read_out(recorded: &Path) -> Result<(), Never> {
    match fetched() {
        Ok(()) => {},
        Err(why) => {
            let Ok(()) = fell("model", "Couldn't download the language", &why);

            return Ok(());
        }
    }

    let Ok(()) = told("Writing it down", UNTIL_IT_CHANGES);

    match heard(recorded) {
        Err(why) => {
            let Ok(()) = told("Couldn't make out the words", BRIEFLY);
            let Ok(()) = fell("hearing", "Couldn't make out the words", &why);
        }
        Ok(words) if words.is_empty() => {
            let Ok(()) = told("Nothing was said", BRIEFLY);
        }
        Ok(words) => {
            let Ok(()) = write(&words);
            let Ok(()) = told(&words, BRIEFLY);
        }
    }

    Ok(())
}

fn gone(pid: i32) -> Result<(), Never> {
    let at = format!("/proc/{pid}");
    let Ok(patience) = Patience::asking_every(LEAVING, std::time::Duration::from_millis(20));
    let Ok(_went) = until(patience, || {
        Ok(match Path::new(&at).exists() {
            true => Seen::NotYet,
            false => Seen::Yes,
        })
    });

    Ok(())
}

fn heard(recorded: &Path) -> Result<String, String> {
    let wav = std::fs::read(recorded).map_err(|why| why.to_string())?;

    let Ok(heard) = anything_said(&wav);

    match heard {
        Heard::Nothing => return Ok(String::new()),
        Heard::Something => {},
    }

    let Ok(engine) = engine();
    let Ok(model) = model();
    let Ok(language) = languages::chosen();
    let Ok(argv) = hearing(&engine, &model, recorded, &language);

    let Some((program, rest)) = argv.split_first() else {
        return Err("there is no program to hear with".to_string());
    };

    let answered = Command::new(program)
        .args(rest)
        .stderr(Stdio::null())
        .output()
        .map_err(|why| why.to_string())?;

    match answered.status.success() {
        true => {},
        false => return Err(format!("whisper-cli said no: {}", answered.status)),
    }

    let Ok(tidy) = tidy(&String::from_utf8_lossy(&answered.stdout));

    Ok(tidy)
}

fn write(words: &str) -> Result<(), Never> {
    let Ok(argv) = typing(words);

    let Some((program, rest)) = argv.split_first() else {
        let Ok(()) = fell("typing", "Couldn't type the words", "there is no program to type with");

        return Ok(());
    };

    match Command::new(program).args(rest).status() {
        Err(why) => {
            let Ok(()) = fell("typing", "Couldn't type the words", &why.to_string());
        }
        Ok(status) if !status.success() => {
            let Ok(()) = fell("typing", "Couldn't type the words", &status.to_string());
        }
        Ok(_) => (),
    }

    Ok(())
}

fn engine() -> Result<PathBuf, Never> {
    let Ok(ours) = whisper();

    match ours.exists() {
        true => return Ok(ours),
        false => {},
    }

    let mut building = Command::new("dictate");
    building.arg("--build").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());

    let _ = let_go(&mut building);

    let Ok(whisper_cli) = Program::WhisperCli.name();

    Ok(PathBuf::from(whisper_cli))
}

fn built() -> Result<(), String> {
    let Ok(ours) = whisper();

    match ours.exists() {
        true => return Ok(()),
        false => {},
    }

    let Some(parent) = ours.parent() else { return Err("nowhere to keep it".to_string()) };

    std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;

    let alone = parent.join("building.lock");

    match std::fs::OpenOptions::new().write(true).create_new(true).open(&alone) {
        Ok(_) => {}
        Err(fault) if fault.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),

        Err(fault) => {
            eprintln!("{}: {fault}", alone.display());
            return Ok(());
        }
    }

    let answer = build(&ours);
    let _ = std::fs::remove_file(&alone);
    answer
}

fn build(ours: &Path) -> Result<(), String> {
    let Ok(at) = making();
    let _ = std::fs::remove_dir_all(&at);

    match at.parent() {
        Some(parent) => std::fs::create_dir_all(parent).map_err(|why| why.to_string())?,
        None => {},
    }

    let Ok(()) = told("Setting up dictation, once", UNTIL_IT_CHANGES);
    let Ok(cloning) = cloning(&at);
    let Ok(configuring) = configuring(&at);
    let Ok(compiling) = compiling(&at);

    for argv in [cloning, configuring, compiling] {
        let Some((program, rest)) = argv.split_first() else {
            return Err("a step of the build named no program to run".to_string());
        };

        let answered = Command::new(program)
            .args(rest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|why| format!("{program} could not be run: {why}"))?;

        match answered.success() {
            true => {},
            false => {
                let _ = std::fs::remove_dir_all(&at);
                return Err(format!("{program} said no: {answered}"));
            }
        }
    }

    let coming = ours.with_extension("coming");
    let Ok(made) = made(&at);

    std::fs::copy(made, &coming).map_err(|why| why.to_string())?;
    std::fs::rename(&coming, ours).map_err(|why| why.to_string())?;

    let _ = std::fs::remove_dir_all(&at);
    let Ok(()) = told("Dictation is ready", BRIEFLY);

    Ok(())
}

fn fetched() -> Result<(), String> {
    let Ok(model) = model();

    match model.exists() {
        true => return Ok(()),
        false => {},
    }

    let Some(parent) = model.parent() else { return Err("nowhere to keep it".to_string()) };

    std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;

    let Ok(()) = told("Downloading the language, once", UNTIL_IT_CHANGES);
    let coming = parent.join("coming.bin");
    let Ok(argv) = fetching(&coming);

    let Some((program, rest)) = argv.split_first() else {
        return Err("there is no program to fetch the words with".to_string());
    };

    let answered = Command::new(program).args(rest).status().map_err(|why| why.to_string())?;

    match answered.success() {
        true => {},
        false => {
            let _ = std::fs::remove_file(&coming);
            return Err(format!("curl said no: {answered}"));
        }
    }

    std::fs::rename(&coming, &model).map_err(|why| why.to_string())
}

const LEAVING: std::time::Duration = std::time::Duration::from_secs(2);

const UNTIL_IT_CHANGES: &str = "0";

const BRIEFLY: &str = "2000";

fn told(what: &str, until: &str) -> Result<(), Never> {
    let Ok(mut saying) = Program::NotifySend.command();

    saying
        .args([
            "--app-name=Console",
            "--urgency=low",
            &format!("--expire-time={until}"),
            "--hint=string:x-canonical-private-synchronous:dictate",
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

fn fell(kind: &str, summary: &str, body: &str) -> Result<(), Never> {
    eprintln!("dictate: {summary}: {body}");

    let mut saying = Command::new("console-say");
    saying.args([kind, summary, body]).stdin(Stdio::null());

    let _ = let_go(&mut saying);

    Ok(())
}
