//! A song opens in the music panel, whatever kind of song it is.
//!
//! Three places on this device name the types that music is, and all three
//! have to agree or a song opens somewhere surprising:
//!
//!   - `KINDS`, which is what the settings panel writes when somebody chooses
//!     what opens Music.
//!   - `console-music.desktop`, which is what the music panel claims it can
//!     open, and therefore whether it is offered on that list at all.
//!   - `/etc/xdg/mimeapps.list`, which is the answer a machine rebuilt from the
//!     manifest starts from, before anybody has chosen anything.
//!
//! They did not agree. `.opus` is `audio/x-opus+ogg` and not `audio/ogg`, and
//! none of the three had ever said so, so an opus file fell past all of them to
//! whatever claimed it last -- which on a machine with three browsers on it is
//! a browser. A song opened as a black rectangle with a scrubber in it.
//!
//! Nothing here needs the device. All three are files in this tree.

use std::path::{Path, PathBuf};

use console_settings::defaults::KINDS;

fn root() -> PathBuf {
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn read(at: &str) -> String {
    std::fs::read_to_string(root().join(at)).unwrap_or_else(|_| panic!("{at} is in the tree"))
}

fn music() -> &'static console_settings::defaults::Kind {
    KINDS.iter().find(|kind| kind.says == "Music").expect("a Music setting")
}

/// What a desktop file says it opens.
fn claimed(said: &str) -> Vec<String> {
    said.lines()
        .find_map(|line| line.strip_prefix("MimeType="))
        .expect("a MimeType line")
        .split(';')
        .filter(|kind| !kind.is_empty())
        .map(str::to_string)
        .collect()
}

/// The type an `.opus` file actually is, which is the whole of this.
const OPUS: &str = "audio/x-opus+ogg";

/// The setting is what it writes, and what it writes has to include the type
/// of the file somebody was complaining about.
#[test]
fn the_music_setting_names_the_type_an_opus_file_is() {
    let every: Vec<&str> = music().every().collect();
    assert!(every.contains(&OPUS), "the Music setting does not name opus: {every:?}");
    assert!(every.contains(&"audio/mpeg"), "nor mp3: {every:?}");
    assert!(every.contains(&"audio/flac"), "nor flac: {every:?}");
}

/// A program that does not claim a type is not offered for it and cannot be
/// set for it by anything that checks. The panel is told to set the family
/// whether or not it is claimed, so this is the half that keeps that honest.
#[test]
fn the_music_panel_claims_everything_the_setting_would_hand_it() {
    let claims = claimed(&read("files/usr/share/applications/console-music.desktop"));
    for kind in music().every() {
        assert!(claims.iter().any(|said| said == kind), "console-music.desktop does not open {kind}");
    }
}

/// And the answer a rebuilt machine starts from, before the panel has been
/// opened once. This is the one that decides it in practice: nobody goes to the
/// settings to say that a song is music.
#[test]
fn a_machine_that_has_chosen_nothing_still_opens_a_song_in_the_music_panel() {
    let said = read("files/etc/xdg/mimeapps.list");
    for kind in music().every() {
        let line = format!("{kind}=console-music.desktop");
        assert!(said.lines().any(|said| said.trim() == line), "mimeapps.list is missing: {line}");
    }
}

/// A type that is two kinds at once is a setting that fights another setting:
/// choosing Music would change what Video opens, and whichever was chosen last
/// would win without saying so.
#[test]
fn no_type_belongs_to_two_kinds() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for kind in &KINDS {
        for mime in kind.every() {
            if let Some((was, _)) = seen.iter().find(|(_, said)| *said == mime) {
                panic!("{mime} is both {was} and {}", kind.says);
            }
            seen.push((kind.says, mime));
        }
    }
}

/// The same type written twice in one family is harmless and is still somebody
/// having edited the list twice without reading it.
#[test]
fn no_kind_names_a_type_twice() {
    for kind in &KINDS {
        let every: Vec<&str> = kind.every().collect();
        let mut once = every.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), every.len(), "{} names a type twice: {every:?}", kind.says);
    }
}
