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

use gtk4::glib;
use console_download::looking::{self, Found, Looked, NO_YT_DLP};
use console_download::store::{self, Kind, SIDE};

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Some(kind) = words.first().and_then(|word| Kind::read(word)) else {
        eprintln!("which kind: --audio or --video");
        return;
    };
    // Joined rather than taken one at a time: what was typed is a sentence and
    // a shell has already taken it apart by the time it arrives here.
    let asked = words[1..].join(" ").trim().to_string();
    if asked.is_empty() {
        eprintln!("what to look for");
        return;
    }
    let cache = glib::user_cache_dir();
    let looked = look(&asked);
    let _ = std::fs::create_dir_all(store::pictures(&cache));
    for found in &looked.found {
        picture(&cache, found);
    }
    wrote(&cache, kind, &looked);
}

/// yt-dlp, asked for the list.
///
/// A search that fails comes back as a search that failed rather than as
/// nothing at all. Nothing is the same shape as a word nobody has ever uploaded
/// anything about, and the two want different rows.
fn look(asked: &str) -> Looked {
    let argv = looking::search(asked);
    let asked = asked.to_string();
    let Ok(done) = Command::new(&argv[0]).args(&argv[1..]).output() else {
        return Looked { asked, fault: NO_YT_DLP.to_string(), found: Vec::new() };
    };
    let said = String::from_utf8_lossy(&done.stdout);
    match done.status.success() {
        true => Looked { asked, fault: String::new(), found: looking::found_in(&said) },
        false => Looked {
            asked,
            fault: looking::complaint(&String::from_utf8_lossy(&done.stderr)),
            found: Vec::new(),
        },
    }
}

/// The picture of one thing, fetched once and kept.
///
/// Fetched beside itself and drawn out into place, because the panel reads this
/// folder the moment this program ends and half a picture is drawn as GTK's
/// mark for a broken one.
fn picture(cache: &Path, found: &Found) {
    let Some(at) = store::picture_of(cache, &found.id) else { return };
    if at.exists() || found.picture.is_empty() {
        return;
    }
    let part = at.with_extension("part");
    let fetched = Command::new("curl")
        .args(["--silent", "--location", "--max-time", "20", "--output"])
        .arg(&part)
        .arg(&found.picture)
        .status();
    if fetched.is_ok_and(|how| how.success()) {
        drawn_out(&part, &at);
    }
    let _ = std::fs::remove_file(&part);
}

/// What was fetched, written out as something this desktop can draw.
///
/// The name a site gives a picture is not what the picture is: YouTube's end in
/// .jpg and every one of them arrives as a webp, which GTK here has no loader
/// for and draws as nothing at all -- a row keeping room for a picture that was
/// fetched, kept, and never seen. ffmpeg is asked what it actually is, and it
/// is written out small enough for a row while it is open anyway.
fn drawn_out(part: &Path, at: &Path) {
    let done = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(part)
        .args([
            "-vf",
            &format!("scale={SIDE}:{SIDE}:force_original_aspect_ratio=decrease"),
        ])
        .arg(at)
        .status();
    if !done.is_ok_and(|how| how.success()) {
        let _ = std::fs::remove_file(at);
    }
}

/// What came back, where the panel looks for it.
fn wrote(cache: &Path, kind: Kind, looked: &Looked) {
    let _ = std::fs::create_dir_all(store::folder(cache));
    let at = store::found_at(cache, kind);
    let part = at.with_extension("part");
    if std::fs::write(&part, looking::written(looked)).is_ok() {
        let _ = std::fs::rename(&part, &at);
    }
}
