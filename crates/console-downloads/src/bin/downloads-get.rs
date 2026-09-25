//! Fetch one thing, and say so when it has arrived.
//!
//!     downloads-get --audio https://youtu.be/FTQbiNvZqaY
//!     downloads-get --video https://youtu.be/FTQbiNvZqaY
//!     downloads-get --book https://www.gutenberg.org/ebooks/84 Frankenstein
//!
//! A film is minutes of fetching, and the panel that started this is a card
//! someone has probably closed by the time it lands. So the arrival is said on
//! the screen rather than drawn: the panel's own word in the corner says it was
//! set going, and this says it is done.
//!
//! What is already in the folder is not fetched again, a book excepted. yt-dlp's answer to being
//! asked twice is to download the whole thing, hand it to a converter that will
//! not write over what is there, and fail at the last step with "Conversion
//! failed!" -- having left the metadata and a half-made `.temp.opus` in the
//! folder, which ends in an extension the music panel lists. So it is asked
//! here first, where the answer is one look at a folder.
//!
//! A book is pressed again to have it again, and there is only ever one of it
//! in Books: it arrives under a `.part` name, and only once it has arrived
//! whole are the copies already there taken away -- the same id, or the same
//! title from the other library -- and the new one put where they were. A
//! fetch that fails leaves the old copy standing.
//!
//! Nothing here tells an open library what arrived. The folder it was written
//! into is watched by the pool, so the books, the music and the films hear it
//! from the kernel, the same way they hear a file anybody else put there.

use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use console_downloads::getting;
use console_downloads::getting::Fetch;
use console_downloads::looking;
use console_downloads::store::Kind;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::running::{Notification, say};

const KIND: &str = "download";

const NOTHING: &str = "couldn't be downloaded";

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let kind = match words.first().and_then(|word| {
        let Ok(kind) = Kind::read(word);

        kind
    }) {
        Some(kind) => kind,
        None => {
            eprintln!("which kind: --audio, --video or --book");
            return;
        }
    };

    let url = match words.get(1) {
        Some(url) => url,
        None => {
            eprintln!("which link");
            return;
        }
    };

    let called = words.get(2).cloned();
    let Ok(into) = getting::into(kind);

    match std::fs::create_dir_all(&into) {
        Ok(()) => {},
        Err(fault) => {
            let why = format!("{} cannot be written to: {fault}", into.display());
            let Ok(named) = named(&called, NOTHING);

            let Ok(()) = say(KIND, Notification { summary: &named, body: &why });
            return;
        }
    }

    let Ok(already) = already(&into, url);

    match (kind, already) {
        (Kind::Sound | Kind::Film, Some(have)) => {
            let named = match called {
                Some(ref called) => called.clone(),
                None => have,
            };

            let Ok(()) = told(&named, &into, ALREADY);
            return;
        }
        (Kind::Book, _) | (Kind::Sound | Kind::Film, None) => {},
    }

    let began = SystemTime::now();
    let title = match &called {
        Some(called) => called.as_str(),
        None => "",
    };
    let Ok(arguments) = getting::arguments(Fetch { kind, url, into: &into, title });
    let Ok(nothing) = named(&called, NOTHING);
    let Ok(missing) = looking::missing(kind);

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => {
            let Ok(()) = say(KIND, Notification { summary: &nothing, body: missing });
            return;
        }
    };

    let done = match Command::new(program).args(rest).output() {
        Ok(done) => done,
        Err(_fault) => {
            let Ok(()) = say(KIND, Notification { summary: &nothing, body: missing });
            return;
        }
    };

    match done.status.success() {
        true => {
            let said = String::from_utf8_lossy(&done.stdout).to_string();

            let said = match kind {
                Kind::Book => {
                    let Ok(id) = id_of(url);
                    let Ok(settled) = settled(&into, &said, getting::Book { id: &id, title });

                    settled
                },
                Kind::Sound | Kind::Film => said,
            };

            let Ok(arrived) = arrived(&said);
            let Ok(()) = told(&arrived, &into, IS_IN);
        },
        false => {
            let Ok(()) = swept(&into, began);
            let Ok(why) = looking::complaint(&String::from_utf8_lossy(&done.stderr));

            let Ok(()) = say(KIND, Notification { summary: &nothing, body: &why });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Landed<'a>(&'a str);

const IS_IN: Landed<'static> = Landed("is in");
const ALREADY: Landed<'static> = Landed("was already in");

fn named(called: &Option<String>, said: &str) -> Result<String, Never> {
    Ok(match called {
        Some(called) => format!("{called} {said}"),
        None => format!("It {said}"),
    })
}

fn already(into: &Path, url: &str) -> Result<Option<String>, Never> {
    let id = match getting::id_in(url) {
        Ok(Some(id)) => id,
        Ok(None) | Err(_) => return Ok(None),
    };

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

    let holding = match holding {
        Some(holding) => holding,
        None => return Ok(None),
    };

    let named = console_music_player::library::named(&holding)?;

    Ok(Some(named))
}

fn id_of(url: &str) -> Result<String, Never> {
    Ok(match getting::id_in(url) {
        Ok(Some(id)) => id,
        Ok(None) | Err(_) => String::new(),
    })
}

fn settled(into: &Path, said: &str, book: getting::Book<'_>) -> Result<String, Never> {
    let part = match said.lines().map(str::trim).rfind(|line| !line.is_empty()) {
        Some(part) => part,
        None => return Ok(said.to_string()),
    };

    let whole = match getting::finished(part) {
        Ok(Some(whole)) => whole,
        Ok(None) | Err(_) => return Ok(said.to_string()),
    };

    let names = match std::fs::read_dir(into) {
        Ok(reading) => reading.flatten().map(|entry| entry.file_name().to_string_lossy().to_string()).collect(),
        Err(_fault) => Vec::new(),
    };
    let Ok(copies) = getting::copies(names, book);
    let keeping = Path::new(whole).file_name().map(|name| name.to_string_lossy().to_string());

    for copy in copies {
        match Some(&copy) == keeping.as_ref() {
            true => {},
            false => {
                let _ = std::fs::remove_file(into.join(&copy));
            },
        }
    }

    match std::fs::rename(part, whole) {
        Ok(()) => {},
        Err(fault) => eprintln!("putting {whole} in place: {fault}"),
    }

    Ok(whole.to_string())
}

fn written(said: &str) -> Result<Option<&str>, Never> {
    Ok(said.lines().map(str::trim).rfind(|line| !line.is_empty()))
}

fn arrived(said: &str) -> Result<String, Never> {
    let Ok(path) = written(said);
    let named =
        path.map(Path::new).and_then(Path::file_name).map(|name| name.to_string_lossy().to_string());

    let name = match named {
        Some(name) => name,
        None => String::new(),
    };

    Ok(match name.is_empty() {
        true => "It".to_string(),
        false => console_music_player::library::named(&name)?,
    })
}

fn swept(into: &Path, began: SystemTime) -> Result<(), Never> {
    let reading = match std::fs::read_dir(into) {
        Ok(reading) => reading,
        Err(_fault) => return Ok(()),
    };

    for entry in reading.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        let made = match entry.metadata().and_then(|about| about.modified()) {
            Ok(made) => made,
            Err(_fault) => continue,
        };

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

fn told(name: &str, into: &Path, said: Landed<'_>) -> Result<(), Never> {
    let said = said.0;
    let where_ = into.file_name().map(|name| name.to_string_lossy().to_string());
    let Ok(mut notifysend) = Program::NotifySend.command();

    let started = notifysend
        .args(["--app-name=Console", "--icon=folder-download", "--"])
        .arg(name)
        .arg(format!("{said} {}", match where_ {
            Some(where_) => where_,
            None => into.display().to_string(),
        }))
        .status();

    match started {
        Ok(_) => {},
        Err(fault) => {
            eprintln!("telling someone it landed: {fault}");
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
        assert_eq!(named(&Some("Africa".to_string()), NOTHING), Ok("Africa couldn't be downloaded".to_string()));
        assert_eq!(named(&None, NOTHING), Ok("It couldn't be downloaded".to_string()));
    }
}
