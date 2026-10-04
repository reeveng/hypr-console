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
use std::process::{Command, ExitCode};

use console_core_arguments::{Operands, Reason, ValidationError, read};

use console_downloads::covers::{self, Fetched};
use console_downloads::gutenberg;
use console_downloads::standard_ebooks;
use console_downloads::looking::{self, Found, Looked};
use console_downloads::store::{self, Kind, SIDE};
use console_core_external_programs::Program;
use console_core_never::Never;

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "downloads-find",
    about: "look for something of one kind, and write down what comes back",
    flags: &Kind::FLAGS,
    operands: Operands::Verbatim("WORDS"),
};

fn request(words: &[String]) -> Result<(Kind, String), ValidationError> {
    let read = read(&COMMAND, words);
    let line = read?;
    let choice = line.one_of(&Kind::FLAGS);
    let flag = choice?;
    let Ok(operands) = line.operands();
    let looking_for = operands.join(" ").trim().to_string();

    match (Kind::from_flag(flag.spelling), looking_for.is_empty()) {
        (Ok(Some(kind)), false) => Ok((kind, looking_for)),
        (Ok(Some(_kind)), true) => {
            let Ok(refusal) = line.refusal(Reason::MissingOperands(vec!["WORDS"]));

            Err(refusal)
        }
        (Ok(None), _empty) => {
            let Ok(refusal) = line.refusal(Reason::MissingFlag(Kind::FLAGS.iter().map(|flag| flag.spelling).collect()));

            Err(refusal)
        }
    }
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let (kind, asked) = match request(&words) {
        Ok(request) => request,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let Ok(()) = find(kind, &asked);

    ExitCode::SUCCESS
}

fn find(kind: Kind, asked: &str) -> Result<(), Never> {
    let Ok(cache) = store::cache();

    let cache = match cache {
        Some(cache) => cache,

        None => {
            eprintln!("downloads-find: no HOME, so there is nowhere to write what was found");

            return Ok(());
        }
    };

    let Ok(looked) = look(kind, asked);
    let Ok(pictures) = store::pictures(&cache);
    let _ = std::fs::create_dir_all(pictures);

    for found in &looked.found {
        let Ok(()) = picture(&cache, found);
    }

    let Ok(()) = wrote(&cache, kind, &looked);

    Ok(())
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
    let Ok(missing) = looking::missing_tool_message(kind);
    let asked = asked.to_string();

    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(Looked { asked, fault: missing.to_string(), found: Vec::new() }),
    };

    let done = match Command::new(program).args(rest).output() {
        Ok(done) => done,
        Err(_would_not_start) => return Ok(Looked { asked, fault: missing.to_string(), found: Vec::new() }),
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
    let Ok(fetched) = covers::fetched(&found.picture, &part);

    match fetched {
        Fetched::Arrived => {
            let Ok(()) = drawn_out(&part, &at);
        },
        Fetched::Failed => {},
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

    let said = match looking::serialize(looked) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn words(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    #[test]
    fn what_a_person_types_reads_as_words_after_two_dashes_even_when_it_starts_with_dashes() {
        let Ok(typed) = words(&["--audio", "--", "--toto africa"]);
        let Ok(plain) = words(&["--video", "toto", "africa"]);
        let Ok(no_kind) = words(&["toto"]);

        assert_eq!(request(&typed), Ok((Kind::Sound, "--toto africa".to_string())));
        assert_eq!(request(&plain), Ok((Kind::Film, "toto africa".to_string())));
        assert_eq!(
            request(&no_kind).map_err(|refusal| refusal.reason),
            Err(Reason::MissingFlag(vec!["--audio", "--video", "--book"]))
        );
    }
}
