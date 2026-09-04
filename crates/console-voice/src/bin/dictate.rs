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

use console_voice::{
    Heard, anything_said, cloning, compiling, configuring, fetching, hearing, languages, made, making,
    model, recording, said, taken, taking, tidy, told_by, typing, whisper,
};
use std::path::PathBuf;

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    match asked.first().map(String::as_str) {
        Some("--fetch") => {
            if let Err(why) = fetched() {
                fell("model", "The words could not be fetched", &why);
            }

            if let Err(why) = built() {
                fell("hearing", "The hearing could not be built", &why);
            }
        }
        Some("--build") => {
            if let Err(why) = built() {
                fell("hearing", "The hearing could not be built", &why);
            }
        }
        Some(word) => {
            eprintln!("dictate: {word} is not a word this takes");
            std::process::exit(2);
        }
        None => match listening() {
            Taken::Yes => wrote_down(),
            Taken::No => listen(),
        },
    }
}

/// Whether the microphone is already being taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Taken {
    /// Something is recording, so this press is the one that ends it.
    Yes,
    /// Nothing is, so this press is the one that starts it.
    No,
}

fn listening() -> Taken {
    match holder().is_some() {
        true => Taken::Yes,
        false => Taken::No,
    }
}

/// Which process is holding the microphone, and what it is filling.
///
/// A pid file outlives the thing it names, so the process is asked about
/// rather than believed: a recording killed with the session leaves a number
/// behind, and a press reading that number alone would stop a recording that
/// was never started and write down a file from an hour ago.
fn holder() -> Option<(i32, u32)> {
    let at = taking();

    let note = match std::fs::read_to_string(&at) {
        Ok(note) => note,
        // No note is nobody holding the microphone, which is the ordinary
        // answer and the one this is asked for.
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return None,

        Err(fault) => {
            eprintln!("{}: reading who is holding the microphone: {fault}", at.display());
            return None;
        }
    };

    let (pid, press) = told_by(&note)?;
    Path::new(&format!("/proc/{pid}")).exists().then_some((pid, press))
}

/// Take the microphone until the next press.
///
/// The recording is named after this press rather than after the button, so
/// the press that stops it is the only one that will ever touch the file.
fn listen() {
    let press = std::process::id();
    let into = said(press);

    if let Some(parent) = into.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let argv = recording(&into);
    let started = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn();

    match started {
        Err(why) => fell("microphone", "Nothing is listening", &why.to_string()),
        Ok(child) => {
            let _ = std::fs::write(taking(), taken(child.id(), press));
            told("Listening", UNTIL_IT_CHANGES);
        }
    }
}

/// Stop listening, and write down what was said.
///
/// The note is taken away before the hearing starts rather than after it
/// finishes, because the hearing takes as long as it takes and a thumb pressing
/// again inside that time should start the next sentence, not wait behind this
/// one. It can: the recording being read here is this press's own, and the
/// press that starts the next one writes into a name of its own.
fn wrote_down() {
    let Some((pid, press)) = holder() else { return };

    // SAFETY: a signal to a pid this desktop started and has not reaped.
    unsafe { libc::kill(pid, libc::SIGINT) };

    gone(pid);
    let _ = std::fs::remove_file(taking());

    // The recording is taken away here rather than at the end of the reading,
    // so that it is taken away however the reading ends. It used to be the last
    // line of that work, which meant every way out of it that was not the happy
    // one -- a model that could not be fetched, most of all -- left somebody's
    // voice sitting in the runtime directory until they logged out.
    let recorded = said(press);
    read_out(&recorded);
    let _ = std::fs::remove_file(&recorded);
}

/// Read the recording, and put what it says where a keyboard would have.
fn read_out(recorded: &Path) {
    if let Err(why) = fetched() {
        fell("model", "The words could not be fetched", &why);
        return;
    }

    told("Writing it down", UNTIL_IT_CHANGES);

    match heard(recorded) {
        Err(why) => {
            // Both, and in this order. console-say raises its own notification
            // and that one does not replace this one, so without a word here
            // the desktop is left saying it is still writing down a sentence it
            // has already given up on.
            told("What was said could not be read", BRIEFLY);
            fell("hearing", "What was said could not be read", &why);
        }
        Ok(words) if words.is_empty() => told("Nothing was said", BRIEFLY),
        Ok(words) => {
            write(&words);
            told(&words, BRIEFLY);
        }
    }
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
///
/// A recording with nothing in it is not asked about, because whisper does not
/// answer an empty room with an empty answer. It answers with "Thank you.",
/// confidently, every time -- and this types what it is told into whatever
/// holds the focus, so an unasked question is the difference between a paddle
/// that does nothing when nothing was said and one that writes a stranger's
/// politeness into somebody's message.
fn heard(recorded: &Path) -> Result<String, String> {
    let wav = std::fs::read(recorded).map_err(|why| why.to_string())?;

    if anything_said(&wav) == Heard::Nothing {
        return Ok(String::new());
    }

    let argv = hearing(&engine(), &model(), recorded, &languages::chosen());
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

/// Which hearing to run: this machine's own, or the packaged one.
///
/// Ours is the same whisper.cpp pointed at the graphics card, and on this
/// device that is the difference between a sentence arriving in under a second
/// and one arriving in five. The packaged build knows nothing but the
/// processor.
///
/// If ours is not there yet, the packaged one answers this press and the
/// building is started behind it. A paddle is not the place to find out that a
/// C++ project takes four minutes: the sentence somebody just spoke is worth
/// more than the speed of the one after it.
fn engine() -> PathBuf {
    let ours = whisper();

    if ours.exists() {
        return ours;
    }

    let _ = Command::new("dictate")
        .arg("--build")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    PathBuf::from("whisper-cli")
}

/// The hearing, built for this machine's graphics card if it has not been.
///
/// Once. Two gigabytes of build tree in the runtime directory, one file
/// carried out of it, and the tree taken down again.
fn built() -> Result<(), String> {
    let ours = whisper();

    if ours.exists() {
        return Ok(());
    }

    let Some(parent) = ours.parent() else { return Err("nowhere to keep it".to_string()) };

    std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;

    // One builder. A press that arrives while the last one is still compiling
    // should use the packaged hearing and say nothing, not start a second
    // four-minute build over the top of the first.
    let alone = parent.join("building.lock");

    match std::fs::OpenOptions::new().write(true).create_new(true).open(&alone) {
        Ok(_) => {}
        // The lock is already held: somebody is four minutes into the build
        // this press would have started. Saying nothing is the point of it.
        Err(fault) if fault.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),

        // Any other reason the lock will not be taken -- nowhere to write it,
        // no room left -- is not a build in progress, and it used to be read
        // as one. It still declines to build, because a build nothing can lock
        // is a build that races the next press, but it says why first.
        Err(fault) => {
            eprintln!("{}: {fault}", alone.display());
            return Ok(());
        }
    }

    let answer = build(&ours);
    let _ = std::fs::remove_file(&alone);
    answer
}

/// The building itself, so the lock above is released whichever way it ends.
fn build(ours: &Path) -> Result<(), String> {
    let at = making();
    let _ = std::fs::remove_dir_all(&at);

    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;
    }

    told("Building the hearing, once", UNTIL_IT_CHANGES);

    for argv in [cloning(&at), configuring(&at), compiling(&at)] {
        let answered = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|why| format!("{} could not be run: {why}", argv[0]))?;

        if !answered.success() {
            let _ = std::fs::remove_dir_all(&at);
            return Err(format!("{} said no: {answered}", argv[0]));
        }
    }

    // Beside the name rather than over it, so a build interrupted halfway
    // leaves nothing a press would try to run.
    let coming = ours.with_extension("coming");
    std::fs::copy(made(&at), &coming).map_err(|why| why.to_string())?;
    std::fs::rename(&coming, ours).map_err(|why| why.to_string())?;
    let _ = std::fs::remove_dir_all(&at);
    told("The hearing is ready", BRIEFLY);
    Ok(())
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
    told("Fetching the words, once", UNTIL_IT_CHANGES);

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

/// How long something this says stays up.
///
/// A state stays until the state changes. Listening is true until the next
/// press, and writing it down is true until the words are there, and a
/// notification that takes itself down after two seconds while the thing it
/// describes is still going on is a desktop saying it has stopped doing
/// something it is still doing.
///
/// That is what a two-second "Writing it down" was: the hearing takes about
/// three, so the message left before the words came, and the wait it was there
/// to explain was the part somebody sat through with nothing on the screen.
const UNTIL_IT_CHANGES: &str = "0";

/// And an ending stays for a moment and goes: the words are in the box by
/// then, and the box is the answer. Nobody needs telling twice.
const BRIEFLY: &str = "2000";

/// Say something on the screen, where somebody with no terminal is.
///
/// Every press replaces what the last one put up, because these are the states
/// of one thing happening rather than a list of events: listening, writing it
/// down, and then the words themselves in the window they belong to.
///
/// Started and not waited for. Saying something is never worth the thing that
/// said it, and this is the button that proved why: with no notification
/// daemon on the machine, D-Bus answered every one of these by trying to start
/// a service that could not start and failing fifty seconds later. Waited for,
/// that was fifty seconds of a paddle doing nothing before the microphone was
/// even asked for, and fifty more before the words were read -- around a
/// sentence somebody had already finished speaking. The recording ran the
/// whole time and every word of it was typed out in the end, nearly two
/// minutes late, into whatever had the focus by then.
fn told(what: &str, until: &str) {
    let _ = Command::new("notify-send")
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
        .stderr(Stdio::null())
        .spawn();
}

/// Say that something went wrong, the way everything else here says it.
///
/// Not waited for either, and for the same reason: this is called from a path
/// that has already failed, and the journal has the line above whatever the
/// screen manages.
fn fell(kind: &str, summary: &str, body: &str) {
    eprintln!("dictate: {summary}: {body}");
    let _ = Command::new("console-say")
        .args([kind, summary, body])
        .stdin(Stdio::null())
        .spawn();
}
