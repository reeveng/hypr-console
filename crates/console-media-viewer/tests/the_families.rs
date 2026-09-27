//! What this panel's desktop file claims, held against the one place the
//! families are written down.
//!
//! `console_settings::defaults::KINDS` is that place, and the entry beside it
//! says why it has to be one place: a kind of thing is a family of types, and
//! the type left out of a second copy is the one that opens somewhere
//! surprising. An `.opus` file opening in a browser is what that cost last
//! time, and it cost it for a year, because the settings tab said the right
//! thing and the machine did something else.
//!
//! So this crossing is written both ways round. A type in the family that the
//! desktop file does not claim is a file this panel will not be offered for; a
//! type the desktop file claims that is not in the family is a claim the
//! settings panel will never write, so the panel would be offered and never
//! chosen. Both are silent, and both are a failing test here.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_core_ini_files::{Key, Under};
use console_core_never::Never;
use console_settings::defaults::KINDS;

type Failure = Box<dyn std::error::Error>;

const SHOWN: [&str; 2] = ["Pictures", "Video"];

const ENTRY: Under<'static> = Under("Desktop Entry");

fn root() -> Result<PathBuf, Failure> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()?;

    Ok(root)
}

fn read(inside: &str) -> Result<String, Failure> {
    let root = root()?;
    let at = root.join(inside);

    match std::fs::read_to_string(&at) {
        Ok(held) => Ok(held),
        Err(fault) => Err(Failure::from(format!("{}: {fault}", at.display()))),
    }
}

fn desktop_file() -> Result<String, Failure> {
    read("files/usr/share/applications/console-media-viewer.desktop")
}

fn mimeapps() -> Result<String, Failure> {
    read("files/etc/xdg/mimeapps.list")
}

fn claimed() -> Result<BTreeSet<String>, Failure> {
    let held = desktop_file()?;
    let Ok(said) = console_core_ini_files::field(&held, ENTRY, Key("MimeType"));
    let types = said.ok_or("a MimeType line")?;

    Ok(types.split(';').map(str::trim).filter(|said| !said.is_empty()).map(str::to_string).collect())
}

fn family() -> Result<BTreeSet<String>, Never> {
    let mut family = BTreeSet::new();

    for kind in KINDS.iter().filter(|kind| SHOWN.contains(&kind.says)) {
        let Ok(every) = kind.every();

        family.extend(every.map(str::to_string));
    }

    Ok(family)
}

#[test]
fn every_type_in_the_family_is_one_this_panel_claims() -> Result<(), Failure> {
    let Ok(family) = family();
    let claimed = claimed()?;
    let missing: Vec<&String> = family.difference(&claimed).collect();

    assert!(
        missing.is_empty(),
        "the settings would set these onto this panel and its desktop file does not claim them: {missing:?}"
    );

    Ok(())
}

#[test]
fn every_type_this_panel_claims_is_one_the_settings_would_set() -> Result<(), Failure> {
    let Ok(family) = family();
    let claimed = claimed()?;
    let extra: Vec<&String> = claimed.difference(&family).collect();

    assert!(
        extra.is_empty(),
        "this panel claims types that are in no family the settings knows: {extra:?}"
    );

    Ok(())
}

#[test]
fn both_families_are_actually_in_it() -> Result<(), Failure> {
    let claimed = claimed()?;

    assert!(claimed.iter().any(|said| said.starts_with("image/")), "no pictures: {claimed:?}");
    assert!(claimed.iter().any(|said| said.starts_with("video/")), "no film: {claimed:?}");
    assert!(claimed.len() >= 10, "too few to be both families: {claimed:?}");

    Ok(())
}

#[test]
fn nothing_claimed_is_a_kind_this_panel_cannot_show() -> Result<(), Failure> {
    let claimed = claimed()?;

    for said in claimed {
        assert_eq!(
            console_media_viewer::kinds::shows(&said),
            Ok(console_media_viewer::kinds::Shows::It),
            "{said} is claimed and cannot be shown"
        );
    }

    Ok(())
}

#[test]
fn the_desktop_file_hands_the_panel_the_file_that_was_opened() -> Result<(), Failure> {
    let held = desktop_file()?;
    let Ok(said) = console_core_ini_files::field(&held, ENTRY, Key("Exec"));
    let exec = said.ok_or("an Exec line")?;

    assert!(exec.contains("viewer"), "{exec}");
    assert!(exec.ends_with("%f"), "{exec}");

    Ok(())
}

#[test]
fn this_desktop_takes_the_whole_family_or_none_of_it() -> Result<(), Failure> {
    let held = mimeapps()?;
    let ours: BTreeSet<String> = held
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(_, opens)| opens.trim() == "console-media-viewer.desktop")
        .map(|(mime, _)| mime.trim().to_string())
        .collect();

    match ours.is_empty() {
        true => Ok(()),
        false => {
            let claimed = claimed()?;
            let unclaimed: Vec<&String> = ours.difference(&claimed).collect();
            let unset: Vec<&String> = claimed.difference(&ours).collect();

            assert!(unclaimed.is_empty(), "set onto this panel and not claimed by it: {unclaimed:?}");
            assert!(
                unset.is_empty(),
                "half the family is switched over and the rest is left to whatever claims it last: {unset:?}"
            );

            Ok(())
        },
    }
}

#[test]
fn no_type_is_handed_to_two_panels() -> Result<(), Failure> {
    let held = mimeapps()?;
    let mut seen: BTreeSet<&str> = BTreeSet::new();

    for (mime, _) in held.lines().filter_map(|line| line.split_once('=')) {
        let mime = mime.trim();

        assert!(seen.insert(mime), "{mime} is set twice");
    }

    Ok(())
}

#[test]
fn every_decoder_a_claimed_type_needs_is_a_package_the_manifest_names() -> Result<(), Failure> {
    let held = read("desktop.conf")?;
    let Ok(lines) = console_core_ini_files::lines(&held, Under("packages"));
    let named: BTreeSet<&str> = lines.into_iter().collect();
    let Ok(packages) = console_media_viewer::decoding::packages();
    let missing: Vec<&str> = packages.into_iter().filter(|one| !named.contains(one)).collect();

    assert!(
        missing.is_empty(),
        "these decode something this panel claims and the manifest does not install them: {missing:?}"
    );

    Ok(())
}

#[test]
fn every_type_this_panel_claims_says_what_decodes_it() -> Result<(), Failure> {
    let claimed = claimed()?;
    let undecodable: Vec<String> = claimed
        .into_iter()
        .filter(|mime| matches!(console_media_viewer::decoding::decoder(mime), Ok(None)))
        .collect();

    assert!(undecodable.is_empty(), "claimed with no decoder named: {undecodable:?}");

    Ok(())
}

#[test]
fn nothing_is_installed_for_a_type_this_panel_does_not_claim() -> Result<(), Failure> {
    let claimed = claimed()?;
    let spare: Vec<&str> = console_media_viewer::decoding::DECODERS
        .iter()
        .map(|one| one.mime)
        .filter(|mime| !claimed.contains(*mime))
        .collect();

    assert!(spare.is_empty(), "decoders named for types nothing claims: {spare:?}");

    Ok(())
}

#[test]
fn the_panel_is_shown_and_set_as_the_default_together_or_not_at_all() -> Result<(), Failure> {
    let held = read("desktop.conf")?;
    let desktop = desktop_file()?;
    let defaults = mimeapps()?;
    let listed = held
        .lines()
        .any(|line| line.trim() == "/usr/share/applications/console-media-viewer.desktop");
    let Ok(hidden) = console_core_ini_files::field(&desktop, ENTRY, Key("NoDisplay"));
    let shown = !hidden.is_some_and(|said| said.eq_ignore_ascii_case("true"));
    let is_default = defaults.lines().any(|line| line.trim().ends_with("=console-media-viewer.desktop"));

    assert!(listed, "nothing may sit under files/ unclaimed, so the entry has to be listed");
    assert_eq!(
        shown, is_default,
        "this panel is shown on the menu: {shown}; it is the default for what it opens: {is_default}. \
         Both or neither -- a card on the home screen that opens nothing is the worse half, \
         because it needs no one to go looking for it."
    );

    Ok(())
}
