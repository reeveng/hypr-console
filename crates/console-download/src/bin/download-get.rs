//! Fetch one thing, and say so when it has arrived.
//!
//!     download-get --audio https://youtu.be/FTQbiNvZqaY
//!     download-get --video https://youtu.be/FTQbiNvZqaY
//!
//! A film is minutes of fetching, and the panel that started this is a card
//! somebody has probably closed by the time it lands. So the arrival is said on
//! the screen rather than drawn: the panel's own word in the corner says it was
//! set going, and this says it is done.
//!
//! What is already in the folder is not fetched again. yt-dlp's answer to being
//! asked twice is to download the whole thing, hand it to a converter that will
//! not write over what is there, and fail at the last step with "Conversion
//! failed!" -- having left the metadata and a half-made `.temp.opus` in the
//! folder, which ends in an extension the music panel lists. So it is asked
//! here first, where the answer is one look at a folder.

use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use console_download::getting;
use console_download::looking::{self, NO_YT_DLP};
use console_download::store::Kind;
use console_panel::running::say;

/// What a fault here is counted as, so a tether that has gone cannot become a
/// wall of notifications.
const KIND: &str = "download";

/// What a fault here is said as. What went wrong is the line under it.
const NOTHING: &str = "was not fetched";

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let Some(kind) = words.first().and_then(|word| Kind::read(word)) else {
        eprintln!("which kind: --audio or --video");
        return;
    };

    let Some(url) = words.get(1) else {
        eprintln!("which link");
        return;
    };

    // What it is called, where the panel has said. Only for what is said about
    // it: the link is what is fetched.
    let called = words.get(2).cloned();
    let into = getting::into(kind);

    if let Err(fault) = std::fs::create_dir_all(&into) {
        let why = format!("{} cannot be written to: {fault}", into.display());
        say(KIND, &named(&called, NOTHING), &why);
        return;
    }

    if let Some(have) = already(&into, url) {
        told(&called.unwrap_or(have), &into, ALREADY);
        return;
    }

    let began = SystemTime::now();
    let argv = getting::argv(kind, url, &into);

    let Ok(done) = Command::new(&argv[0]).args(&argv[1..]).output() else {
        say(KIND, &named(&called, NOTHING), NO_YT_DLP);
        return;
    };

    match done.status.success() {
        true => told(&arrived(&String::from_utf8_lossy(&done.stdout)), &into, IS_IN),
        false => {
            swept(&into, began);
            let why = looking::complaint(&String::from_utf8_lossy(&done.stderr));
            say(KIND, &named(&called, NOTHING), &why);
        }
    }
}

/// What is said about a thing that has landed, and about one that was there
/// already.
const IS_IN: &str = "is in";
const ALREADY: &str = "was already in";

/// The name of the thing this is about, where anything said it, and the fault
/// on its own where nothing did.
///
/// A notification saying only that something was not fetched is one a person
/// cannot act on: two of them side by side are the same card twice.
fn named(called: &Option<String>, said: &str) -> String {
    match called {
        Some(called) => format!("{called} {said}"),
        None => format!("It {said}"),
    }
}

/// What the folder is already holding, if it is holding this.
///
/// The name it is under rather than merely that it is there, so what is said
/// about it is the thing itself and not a pronoun.
fn already(into: &Path, url: &str) -> Option<String> {
    let id = getting::id_in(url)?;
    let reading = match std::fs::read_dir(into) {
        Ok(reading) => reading,
        // A folder that will not open is not a folder without the song in it.
        // Nothing here can put that right, and the fetch that follows meets
        // the same fault with somewhere to say it, so this says it and steps
        // back rather than reporting an empty folder.
        Err(fault) => {
            eprintln!("looking in {} for what is already there: {fault}", into.display());
            return None;
        },
    };
    let holding = reading.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        (getting::leftover(&name) == getting::Litter::No
            && name.contains(&format!("[{id}]")))
        .then_some(name)
    })?;
    Some(console_music::library::named(&holding))
}

/// What arrived, by the name it arrived under.
///
/// yt-dlp prints the path it wrote, and the last line of it is the file rather
/// than anything it was made out of. The title is what a person recognises, so
/// the id in square brackets comes off it the same way the music library takes
/// it off a row.
fn arrived(said: &str) -> String {
    let path = said.lines().map(str::trim).rfind(|line| !line.is_empty());
    let name = path
        .map(Path::new)
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    match name.is_empty() {
        true => "It".to_string(),
        false => console_music::library::named(&name),
    }
}

/// Take away what this run left behind when it failed.
///
/// Only what this run made: a `.part` from something else being fetched at the
/// same time is not this one's to remove. A failed fetch that leaves a
/// half-made file in Music is worse than one that leaves nothing, because the
/// half-made one is listed as a song.
fn swept(into: &Path, began: SystemTime) {
    let Ok(reading) = std::fs::read_dir(into) else { return };

    for entry in reading.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // A file that will not say when it was made is left where it is.
        // This takes away what this run left behind and nothing else, and a
        // file that cannot be dated is not something this run can claim.
        let Ok(made) = entry.metadata().and_then(|about| about.modified()) else { continue };

        if getting::leftover(&name) == getting::Litter::Yes && made >= began {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Say it has landed, where somebody who is not looking at a panel sees it.
///
/// Not `console-say`, which is the shape faults are said in: this is a thing
/// that worked, and a critical card with a warning triangle on it is how a
/// person learns to dismiss the ones that matter.
fn told(name: &str, into: &Path, said: &str) {
    let where_ = into.file_name().map(|name| name.to_string_lossy().to_string());
    let started = Command::new("notify-send")
        .args(["--app-name=Console", "--icon=folder-download", "--"])
        .arg(name)
        .arg(format!("{said} {}", where_.unwrap_or_else(|| into.display().to_string())))
        .status();

    // Nowhere to put a card, so it is said on the way out instead: whoever
    // asked for this is watching a terminal or reading the journal.
    if let Err(fault) = started {
        eprintln!("telling somebody it landed: {fault}");
        println!("{name} {said} {}", into.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_arrived_is_the_last_path_printed_said_as_a_title() {
        let said = "/home/ada/Music/Africa [FTQbiNvZqaY].opus\n";
        assert_eq!(arrived(said), "Africa");
        assert_eq!(arrived(""), "It");
    }

    /// A card saying only that something was not fetched is a card a person
    /// cannot act on, and two of them are the same card twice.
    #[test]
    fn what_is_said_about_a_fault_names_the_thing_it_is_about() {
        assert_eq!(named(&Some("Africa".to_string()), NOTHING), "Africa was not fetched");
        assert_eq!(named(&None, NOTHING), "It was not fetched");
    }
}
