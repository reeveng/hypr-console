//! Look for something, and write down what came back.
//!
//!     downloads-find --audio toto africa
//!     downloads-find --video https://youtu.be/FTQbiNvZqaY
//!     downloads-find --book frankenstein
//!
//! Off the panel, and not in it. A search is a question to a site over
//! someone's tether: a second on a good day and fifteen on a bad one, and a
//! card that waited for it would stop answering the buttons for all of them. So
//! the panel starts this, goes on drawing, and reads what this leaves behind
//! when it ends.
//!
//! The pictures are fetched here too, for the same reason and before the file
//! is written: a list that arrived and then grew pictures a moment later is a
//! list that moves under a thumb already reaching for a row.

use std::path::Path;
use std::process::Command;

use console_downloads::gutenberg;
use console_downloads::standard_ebooks;
use console_downloads::looking::{self, Found, Looked};
use console_downloads::store::{self, Kind, SIDE};
use console_core_external_programs::Program;
use console_core_never::Never;

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

    let asked = match words.get(1..) {
        Some(after) => after.join(" ").trim().to_string(),
        None => String::new(),
    };

    match asked.is_empty() {
        true => {
            eprintln!("what to look for");
            return;
        }
        false => {},
    }

    let Ok(cache) = store::cache();

    let cache = match cache {
        Some(cache) => cache,

        None => {
            eprintln!("downloads-find: no HOME, so there is nowhere to write what was found");

            return;
        }
    };

    let Ok(looked) = look(kind, &asked);
    let Ok(pictures) = store::pictures(&cache);
    let _ = std::fs::create_dir_all(pictures);

    for found in &looked.found {
        let Ok(()) = picture(&cache, found);
    }

    let Ok(()) = wrote(&cache, kind, &looked);
}

fn look(kind: Kind, asked: &str) -> Result<Looked, Never> {
    match kind {
        Kind::Sound | Kind::Film => {
            let Ok(arguments) = looking::search(asked);

            ran(Search { kind, asked, arguments: &arguments, read: looking::found_in })
        },
        Kind::Book => books(asked),
    }
}

fn books(asked: &str) -> Result<Looked, Never> {
    let Ok(standard) = standard_ebooks::search(asked);
    let Ok(standard) = ran(Search { kind: Kind::Book, asked, arguments: &standard, read: standard_ebooks::found_in });
    let Ok(plain) = gutenberg::search(asked);
    let Ok(plain) = ran(Search { kind: Kind::Book, asked, arguments: &plain, read: gutenberg::found_in });

    let fault = match (standard.fault.is_empty(), plain.fault.is_empty()) {
        (false, false) => gutenberg::NOT_ANSWERING.to_string(),
        (true, _) | (_, true) => String::new(),
    };

    let found = standard.found.into_iter().chain(plain.found).collect();

    Ok(Looked { asked: asked.to_string(), fault, found })
}

#[derive(Clone, Copy)]
struct Search<'a> {
    kind: Kind,
    asked: &'a str,
    arguments: &'a [String],
    read: fn(&str) -> Result<Vec<Found>, Never>,
}

fn ran(search: Search<'_>) -> Result<Looked, Never> {
    let Search { kind, asked, arguments, read } = search;
    let Ok(missing) = looking::missing(kind);
    let asked = asked.to_string();

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Looked { asked, fault: missing.to_string(), found: Vec::new() }),
    };

    let done = match Command::new(program).args(rest).output() {
        Ok(done) => done,
        Err(_fault) => return Ok(Looked { asked, fault: missing.to_string(), found: Vec::new() }),
    };

    let said = String::from_utf8_lossy(&done.stdout);

    Ok(match done.status.success() {
        true => {
            let Ok(found) = read(&said);

            Looked { asked, fault: String::new(), found }
        },
        false => {
            let Ok(fault) = looking::complaint(&String::from_utf8_lossy(&done.stderr));

            Looked { asked, fault, found: Vec::new() }
        },
    })
}

fn picture(cache: &Path, found: &Found) -> Result<(), Never> {
    let at = match store::picture_of(cache, &found.id) {
        Ok(Some(at)) => at,
        Ok(None) | Err(_) => return Ok(()),
    };

    match at.exists() || found.picture.is_empty() {
        true => return Ok(()),
        false => {},
    }

    let part = at.with_extension("part");
    let Ok(mut curl) = Program::Curl.command();

    let fetched = curl
        .args(["--silent", "--location", "--max-time", "20", "--output"])
        .arg(&part)
        .arg(&found.picture)
        .status();

    match fetched.is_ok_and(|how| how.success()) {
        true => {
            let Ok(()) = drawn_out(&part, &at);
        },
        false => {},
    }

    let _ = std::fs::remove_file(&part);

    Ok(())
}

fn drawn_out(part: &Path, at: &Path) -> Result<(), Never> {
    let Ok(mut ffmpeg) = Program::Ffmpeg.command();

    let done = ffmpeg
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(part)
        .args([
            "-vf",
            &format!("scale={SIDE}:{SIDE}:force_original_aspect_ratio=decrease"),
        ])
        .arg(at)
        .status();

    match done.is_ok_and(|how| how.success()) {
        true => {},
        false => {
            let _ = std::fs::remove_file(at);
        }
    }

    Ok(())
}

fn wrote(cache: &Path, kind: Kind, looked: &Looked) -> Result<(), Never> {
    let Ok(folder) = store::folder(cache);
    let _ = std::fs::create_dir_all(folder);
    let Ok(at) = store::found_at(cache, kind);

    let said = match looking::written(looked) {
        Ok(said) => said,
        Err(why) => {
            eprintln!("{why}");
            return Ok(());
        },
    };

    match console_core_atomic_writes::whole(&at, said.as_bytes()) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("writing down what the search found: {fault}");
            return Ok(());
        }
    }

    Ok(())
}
