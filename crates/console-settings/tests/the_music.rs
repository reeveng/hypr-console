//! A song opens in the music panel, whatever kind of song it is.
//!
//! Three places on this device name the types that music is, and all three
//! have to agree or a song opens somewhere surprising:
//!
//!   - `KINDS`, which is what the settings panel writes when somebody chooses
//!     what opens Music.
//!   - `console-music-panel.desktop`, which is what the music panel claims it can
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

fn claimed(said: &str) -> Vec<String> {
    said.lines()
        .find_map(|line| line.strip_prefix("MimeType="))
        .expect("a MimeType line")
        .split(';')
        .filter(|kind| !kind.is_empty())
        .map(str::to_string)
        .collect()
}

const OPUS: &str = "audio/x-opus+ogg";

#[test]
fn the_music_setting_names_the_type_an_opus_file_is() {
    let Ok(names) = music().every();
    let every: Vec<&str> = names.collect();

    assert!(every.contains(&OPUS), "the Music setting does not name opus: {every:?}");
    assert!(every.contains(&"audio/mpeg"), "nor mp3: {every:?}");
    assert!(every.contains(&"audio/flac"), "nor flac: {every:?}");
}

#[test]
fn the_music_panel_claims_everything_the_setting_would_hand_it() {
    let claims = claimed(&read("files/usr/share/applications/console-music-panel.desktop"));
    let Ok(every) = music().every();

    for kind in every {
        assert!(claims.iter().any(|said| said == kind), "console-music-panel.desktop does not open {kind}");
    }
}

#[test]
fn a_machine_that_has_chosen_nothing_still_opens_a_song_in_the_music_panel() {
    let said = read("files/etc/xdg/mimeapps.list");
    let Ok(every) = music().every();

    for kind in every {
        let line = format!("{kind}=console-music-panel.desktop");
        assert!(said.lines().any(|said| said.trim() == line), "mimeapps.list is missing: {line}");
    }
}

#[test]
fn no_type_belongs_to_two_kinds() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for kind in &KINDS {
        let Ok(every) = kind.every();

        for mime in every {
            if let Some((was, _)) = seen.iter().find(|(_, said)| *said == mime) {
                panic!("{mime} is both {was} and {}", kind.says);
            }
            seen.push((kind.says, mime));
        }
    }
}

#[test]
fn no_kind_names_a_type_twice() {
    for kind in &KINDS {
        let Ok(names) = kind.every();
        let every: Vec<&str> = names.collect();

        let mut once = every.clone();
        once.sort_unstable();
        once.dedup();
        assert_eq!(once.len(), every.len(), "{} names a type twice: {every:?}", kind.says);
    }
}
