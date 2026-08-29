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

use console_voice::{fetching, hearing, model, recording, said, taking, tidy, typing};

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    match asked.first().map(String::as_str) {
        Some("--fetch") => {
            if let Err(why) = fetched() {
                fell("model", "The words could not be fetched", &why);
            }
        }
        Some(word) => {
            eprintln!("dictate: {word} is not a word this takes");
            std::process::exit(2);
        }
        None => match listening() {
            true => wrote_down(),
            false => listen(),
        },
    }
}

/// Whether the microphone is already being taken.
fn listening() -> bool {
    holder().is_some()
}

/// Which process is holding the microphone, if any is.
///
/// A pid file outlives the thing it names, so the process is asked about
/// rather than believed: a recording killed with the session leaves a number
/// behind, and a press reading that number alone would stop a recording that
/// was never started and write down a file from an hour ago.
fn holder() -> Option<i32> {
    let said = std::fs::read_to_string(taking()).ok()?;
    let pid: i32 = said.trim().parse().ok()?;
    Path::new(&format!("/proc/{pid}")).exists().then_some(pid)
}

/// Take the microphone until the next press.
fn listen() {
    if let Some(parent) = said().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let argv = recording(&said());
    let started = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn();
    match started {
        Err(why) => fell("microphone", "Nothing is listening", &why.to_string()),
        Ok(child) => {
            let _ = std::fs::write(taking(), child.id().to_string());
            told("Listening");
        }
    }
}

/// Stop listening, and write down what was said.
fn wrote_down() {
    let Some(pid) = holder() else { return };
    // SAFETY: a signal to a pid this desktop started and has not reaped.
    unsafe { libc::kill(pid, libc::SIGINT) };
    gone(pid);
    let _ = std::fs::remove_file(taking());

    if let Err(why) = fetched() {
        fell("model", "The words could not be fetched", &why);
        return;
    }
    told("Writing it down");
    match heard() {
        Err(why) => fell("hearing", "What was said could not be read", &why),
        Ok(words) if words.is_empty() => told("Nothing was said"),
        Ok(words) => write(&words),
    }
    let _ = std::fs::remove_file(said());
}

/// Wait for the recording to finish the file it is writing.
///
/// A wav says how long it is in its first bytes, and the length is only known
/// once the recording stops. Read while the microphone still holds it, the
/// file is a header claiming nothing follows it, and the hearing answers with
/// silence for a sentence that is sitting on the disk.
fn gone(pid: i32) {
    let at = format!("/proc/{pid}");
    for _ in 0..100 {
        if !Path::new(&at).exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// What the recording says, in words.
fn heard() -> Result<String, String> {
    let argv = hearing(&model(), &said());
    let answered = Command::new(&argv[0])
        .args(&argv[1..])
        .stderr(Stdio::null())
        .output()
        .map_err(|why| why.to_string())?;
    if !answered.status.success() {
        return Err(format!("whisper-cli said no: {}", answered.status));
    }
    Ok(tidy(&String::from_utf8_lossy(&answered.stdout)))
}

/// Type what was said into whatever holds the focus.
fn write(words: &str) {
    let argv = typing(words);
    match Command::new(&argv[0]).args(&argv[1..]).status() {
        Err(why) => fell("typing", "What was said could not be typed", &why.to_string()),
        Ok(status) if !status.success() => {
            fell("typing", "What was said could not be typed", &status.to_string());
        }
        Ok(_) => (),
    }
}

/// The model, fetched if this machine does not have it yet.
///
/// Once, and on the press that needs it rather than at login: a desktop that
/// spends half a gigabyte at every start is a desktop that costs somebody
/// their morning for a button they may never press. `dictate --fetch` is the
/// same thing asked for in advance.
fn fetched() -> Result<(), String> {
    let model = model();
    if model.exists() {
        return Ok(());
    }
    let Some(parent) = model.parent() else { return Err("nowhere to keep it".to_string()) };
    std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;
    told("Fetching the words, once");

    // Beside the model rather than over it, so a fetch that is interrupted
    // leaves nothing that looks like a model to the press after it.
    let coming = parent.join("coming.bin");
    let argv = fetching(&coming);
    let answered =
        Command::new(&argv[0]).args(&argv[1..]).status().map_err(|why| why.to_string())?;
    if !answered.success() {
        let _ = std::fs::remove_file(&coming);
        return Err(format!("curl said no: {answered}"));
    }
    std::fs::rename(&coming, &model).map_err(|why| why.to_string())
}

/// Say something on the screen, where somebody with no terminal is.
///
/// Every press replaces what the last one put up, because these are the states
/// of one thing happening rather than a list of events: listening, writing it
/// down, and then the words themselves in the window they belong to.
fn told(what: &str) {
    let _ = Command::new("notify-send")
        .args([
            "--app-name=Console",
            "--urgency=low",
            "--expire-time=2000",
            "--hint=string:x-canonical-private-synchronous:dictate",
            "--icon=audio-input-microphone",
            "--",
            what,
        ])
        .status();
}

/// Say that something went wrong, the way everything else here says it.
fn fell(kind: &str, summary: &str, body: &str) {
    eprintln!("dictate: {summary}: {body}");
    let _ = Command::new("console-say").args([kind, summary, body]).status();
}
