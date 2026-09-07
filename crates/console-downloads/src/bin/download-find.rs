//! Look for something, and write down what came back.
//!
//!     download-find --audio toto africa
//!     download-find --video https://youtu.be/FTQbiNvZqaY
//!
//! Off the panel, and not in it. A search is a question to a site over
//! somebody's tether: a second on a good day and fifteen on a bad one, and a
//! card that waited for it would stop answering the buttons for all of them. So
//! the panel starts this, goes on drawing, and reads what this leaves behind
//! when it ends.
//!
//! The pictures are fetched here too, for the same reason and before the file
//! is written: a list that arrived and then grew pictures a moment later is a
//! list that moves under a thumb already reaching for a row.

use std::path::Path;
use std::process::Command;

use console_downloads::looking::{self, Found, Looked, NO_YT_DLP};
use console_downloads::store::{self, Kind, SIDE};
use console_core_external_programs::Program;
use console_core_never::Never;
use gtk4::glib;

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let Some(kind) = words.first().and_then(|word| {
        let Ok(kind) = Kind::read(word);

        kind
    }) else {
        eprintln!("which kind: --audio or --video");
        return;
    };

    let asked = words.get(1..).unwrap_or_default().join(" ").trim().to_string();

    match asked.is_empty() {
        true => {
            eprintln!("what to look for");
            return;
        }
        false => {},
    }

    let cache = glib::user_cache_dir();
    let Ok(looked) = look(&asked);
    let Ok(pictures) = store::pictures(&cache);
    let _ = std::fs::create_dir_all(pictures);

    for found in &looked.found {
        let Ok(()) = picture(&cache, found);
    }

    let Ok(()) = wrote(&cache, kind, &looked);
}

fn look(asked: &str) -> Result<Looked, Never> {
    let Ok(argv) = looking::search(asked);
    let asked = asked.to_string();

    let Some((program, rest)) = argv.split_first() else {
        return Ok(Looked { asked, fault: NO_YT_DLP.to_string(), found: Vec::new() });
    };

    let Ok(done) = Command::new(program).args(rest).output() else {
        return Ok(Looked { asked, fault: NO_YT_DLP.to_string(), found: Vec::new() });
    };

    let said = String::from_utf8_lossy(&done.stdout);

    Ok(match done.status.success() {
        true => {
            let Ok(found) = looking::found_in(&said);

            Looked { asked, fault: String::new(), found }
        },
        false => {
            let Ok(fault) = looking::complaint(&String::from_utf8_lossy(&done.stderr));

            Looked { asked, fault, found: Vec::new() }
        },
    })
}

fn picture(cache: &Path, found: &Found) -> Result<(), Never> {
    let Ok(Some(at)) = store::picture_of(cache, &found.id) else { return Ok(()) };

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
    let part = at.with_extension("part");

    let said = match looking::written(looked) {
        Ok(said) => said,
        Err(why) => {
            eprintln!("{why}");
            return Ok(());
        },
    };

    match std::fs::write(&part, &said) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("writing down what the search found: {fault}");
            return Ok(());
        }
    }

    match std::fs::rename(&part, &at) {
        Ok(()) => {},
        Err(fault) => eprintln!("putting the search where the panel looks for it: {fault}"),
    }

    Ok(())
}
