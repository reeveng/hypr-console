//! A song opens in the music panel, whatever kind of song it is.
//!
//! Three places on this device name the types that music is, and all three
//! have to agree or a song opens somewhere surprising:
//!
//!   - `KINDS`, which is what the settings panel writes when someone chooses
//!     what opens Music.
//!   - `console-music.desktop`, which is what the music panel claims it can
//!     open, and therefore whether it is offered on that list at all.
//!   - `/etc/xdg/mimeapps.list`, which is the answer a machine rebuilt from the
//!     manifest starts from, before anyone has chosen anything.
//!
//! They did not agree. `.opus` is `audio/x-opus+ogg` and not `audio/ogg`, and
//! none of the three had ever said so, so an opus file fell past all of them to
//! whatever claimed it last -- which on a machine with three browsers on it is
//! a browser. A song opened as a black rectangle with a scrubber in it.
//!
//! Nothing here needs the device. All three are files in this tree.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use console_core_ini_files::{Key, Under};
use console_settings::defaults::{KINDS, Kind};

type Failure = Box<dyn std::error::Error>;

const OPUS: &str = "audio/x-opus+ogg";

fn read(inside: &str) -> Result<String, Failure> {
    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;
    let at = tree.join(inside);

    match std::fs::read_to_string(&at) {
        Ok(held) => Ok(held),
        Err(fault) => Err(Failure::from(format!("{inside} is not in the tree: {fault}"))),
    }
}

fn music() -> Result<&'static Kind, Failure> {
    let music = KINDS.iter().find(|kind| kind.says == "Music").ok_or("no Music setting")?;

    Ok(music)
}

fn claimed(said: &str) -> Result<BTreeSet<&str>, Failure> {
    let Ok(types) = console_core_ini_files::field(said, Under("Desktop Entry"), Key("MimeType"));
    let types = types.ok_or("no MimeType line")?;

    Ok(types.split(';').filter(|kind| !kind.is_empty()).collect())
}

#[test]
fn the_music_setting_names_the_type_an_opus_file_is() -> Result<(), Failure> {
    let music = music()?;
    let Ok(names) = music.every();
    let every: BTreeSet<&str> = names.collect();

    assert!(every.contains(OPUS), "the Music setting does not name opus: {every:?}");
    assert!(every.contains("audio/mpeg"), "nor mp3: {every:?}");
    assert!(every.contains("audio/flac"), "nor flac: {every:?}");

    Ok(())
}

#[test]
fn the_music_panel_claims_everything_the_setting_would_hand_it() -> Result<(), Failure> {
    let desktop = read("files/usr/share/applications/console-music.desktop")?;
    let claims = claimed(&desktop)?;
    let music = music()?;
    let Ok(every) = music.every();

    for kind in every {
        assert!(claims.contains(kind), "console-music.desktop does not open {kind}");
    }

    Ok(())
}

#[test]
fn a_machine_that_has_chosen_nothing_still_opens_a_song_in_the_music_panel() -> Result<(), Failure> {
    let said = read("files/etc/xdg/mimeapps.list")?;
    let lines: BTreeSet<&str> = said.lines().map(str::trim).collect();
    let music = music()?;
    let Ok(every) = music.every();

    for kind in every {
        let line = format!("{kind}=console-music.desktop");

        assert!(lines.contains(line.as_str()), "mimeapps.list is missing: {line}");
    }

    Ok(())
}

#[test]
fn no_type_belongs_to_two_kinds() {
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();

    for kind in &KINDS {
        let Ok(every) = kind.every();

        for mime in every {
            let was = seen.insert(mime, kind.says);

            assert_eq!(was, None, "{mime} is both {was:?} and {}", kind.says);
        }
    }
}

#[test]
fn no_kind_names_a_type_twice() {
    for kind in &KINDS {
        let Ok(names) = kind.every();
        let every: Vec<&str> = names.collect();
        let once: BTreeSet<&str> = every.iter().copied().collect();

        assert_eq!(once.len(), every.len(), "{} names a type twice: {every:?}", kind.says);
    }
}
