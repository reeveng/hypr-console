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

use console_downloads::getting;
use console_downloads::looking::{self, NO_YT_DLP};
use console_downloads::store::Kind;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::running::say;

const KIND: &str = "download";

const NOTHING: &str = "was not fetched";

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let Some(kind) = words.first().and_then(|word| {
        let Ok(kind) = Kind::read(word);

        kind
    }) else {
        eprintln!("which kind: --audio or --video");
        return;
    };

    let Some(url) = words.get(1) else {
        eprintln!("which link");
        return;
    };

    let called = words.get(2).cloned();
    let Ok(into) = getting::into(kind);

    match std::fs::create_dir_all(&into) {
        Ok(()) => {},
        Err(fault) => {
            let why = format!("{} cannot be written to: {fault}", into.display());
            let Ok(named) = named(&called, NOTHING);

            let Ok(()) = say(KIND, &named, &why);
            return;
        }
    }

    let Ok(already) = already(&into, url);

    match already {
        Some(have) => {
            let Ok(()) = told(&called.unwrap_or(have), &into, ALREADY);
            return;
        }
        None => {},
    }

    let began = SystemTime::now();
    let Ok(argv) = getting::argv(kind, url, &into);
    let Ok(nothing) = named(&called, NOTHING);

    let Some((program, rest)) = argv.split_first() else {
        let Ok(()) = say(KIND, &nothing, NO_YT_DLP);
        return;
    };

    let Ok(done) = Command::new(program).args(rest).output() else {
        let Ok(()) = say(KIND, &nothing, NO_YT_DLP);
        return;
    };

    match done.status.success() {
        true => {
            let Ok(arrived) = arrived(&String::from_utf8_lossy(&done.stdout));
            let Ok(()) = told(&arrived, &into, IS_IN);
        },
        false => {
            let Ok(()) = swept(&into, began);
            let Ok(why) = looking::complaint(&String::from_utf8_lossy(&done.stderr));

            let Ok(()) = say(KIND, &nothing, &why);
        }
    }
}

const IS_IN: &str = "is in";
const ALREADY: &str = "was already in";

fn named(called: &Option<String>, said: &str) -> Result<String, Never> {
    Ok(match called {
        Some(called) => format!("{called} {said}"),
        None => format!("It {said}"),
    })
}

fn already(into: &Path, url: &str) -> Result<Option<String>, Never> {
    let Ok(Some(id)) = getting::id_in(url) else { return Ok(None) };

    let reading = match std::fs::read_dir(into) {
        Ok(reading) => reading,
        Err(fault) => {
            eprintln!("looking in {} for what is already there: {fault}", into.display());
            return Ok(None);
        },
    };
    let holding = reading.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(litter) = getting::leftover(&name);

        (litter == getting::Litter::No && name.contains(&format!("[{id}]"))).then_some(name)
    });

    let Some(holding) = holding else { return Ok(None) };

    let named = console_music::library::named(&holding)?;

    Ok(Some(named))
}

fn arrived(said: &str) -> Result<String, Never> {
    let path = said.lines().map(str::trim).rfind(|line| !line.is_empty());
    let name = path
        .map(Path::new)
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(match name.is_empty() {
        true => "It".to_string(),
        false => console_music::library::named(&name)?,
    })
}

fn swept(into: &Path, began: SystemTime) -> Result<(), Never> {
    let Ok(reading) = std::fs::read_dir(into) else { return Ok(()) };

    for entry in reading.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        let Ok(made) = entry.metadata().and_then(|about| about.modified()) else { continue };

        let Ok(litter) = getting::leftover(&name);

        match litter == getting::Litter::Yes && made >= began {
            true => {
                let _ = std::fs::remove_file(entry.path());
            }
            false => {},
        }
    }

    Ok(())
}

fn told(name: &str, into: &Path, said: &str) -> Result<(), Never> {
    let where_ = into.file_name().map(|name| name.to_string_lossy().to_string());
    let Ok(mut notifysend) = Program::NotifySend.command();

    let started = notifysend
        .args(["--app-name=Console", "--icon=folder-download", "--"])
        .arg(name)
        .arg(format!("{said} {}", where_.unwrap_or_else(|| into.display().to_string())))
        .status();

    match started {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("telling somebody it landed: {fault}");
            println!("{name} {said} {}", into.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_arrived_is_the_last_path_printed_said_as_a_title() {
        let said = "/home/ada/Music/Africa [FTQbiNvZqaY].opus\n";

        assert_eq!(arrived(said), Ok("Africa".to_string()));
        assert_eq!(arrived(""), Ok("It".to_string()));
    }

    #[test]
    fn what_is_said_about_a_fault_names_the_thing_it_is_about() {
        assert_eq!(named(&Some("Africa".to_string()), NOTHING), Ok("Africa was not fetched".to_string()));
        assert_eq!(named(&None, NOTHING), Ok("It was not fetched".to_string()));
    }
}
