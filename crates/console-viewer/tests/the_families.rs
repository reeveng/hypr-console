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

use console_settings::defaults::KINDS;

/// The two families this panel is for.
const SHOWN: [&str; 2] = ["Pictures", "Video"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the tree")
}

fn desktop_file() -> String {
    let at = root().join("files/usr/share/applications/console-viewer.desktop");
    std::fs::read_to_string(&at).unwrap_or_else(|fault| panic!("{}: {fault}", at.display()))
}

/// What the desktop file says it opens.
fn claimed() -> BTreeSet<String> {
    desktop_file()
        .lines()
        .find_map(|line| line.strip_prefix("MimeType="))
        .expect("a MimeType line")
        .split(';')
        .filter(|said| !said.trim().is_empty())
        .map(|said| said.trim().to_string())
        .collect()
}

/// Every type in the families this panel is for.
fn family() -> BTreeSet<String> {
    KINDS
        .iter()
        .filter(|kind| SHOWN.contains(&kind.says))
        .flat_map(|kind| kind.every())
        .map(|said| said.to_string())
        .collect()
}

/// The one that would let an `.avif` fall through to a browser.
#[test]
fn every_type_in_the_family_is_one_this_panel_claims() {
    let (family, claimed) = (family(), claimed());
    let missing: Vec<&String> = family.difference(&claimed).collect();

    assert!(
        missing.is_empty(),
        "the settings would set these onto this panel and its desktop file does not claim them: {missing:?}"
    );
}

/// The other way round. A claim the settings will never write is a panel
/// offered for something nothing will ever choose it for.
#[test]
fn every_type_this_panel_claims_is_one_the_settings_would_set() {
    let (family, claimed) = (family(), claimed());
    let extra: Vec<&String> = claimed.difference(&family).collect();

    assert!(
        extra.is_empty(),
        "this panel claims types that are in no family the settings knows: {extra:?}"
    );
}

/// Both halves, and neither of them empty -- a crossing between two empty sets
/// passes and says nothing.
#[test]
fn both_families_are_actually_in_it() {
    let claimed = claimed();

    assert!(claimed.iter().any(|said| said.starts_with("image/")), "no pictures: {claimed:?}");
    assert!(claimed.iter().any(|said| said.starts_with("video/")), "no film: {claimed:?}");
    assert!(claimed.len() >= 10, "too few to be both families: {claimed:?}");
}

/// Every type it claims is one this panel would actually draw something for.
/// A desktop file claiming `audio/mpeg` would take music off the music panel.
#[test]
fn nothing_claimed_is_a_kind_this_panel_cannot_show() {
    for said in claimed() {
        assert_eq!(
            console_viewer::kinds::shows(&said),
            console_viewer::kinds::Shows::It,
            "{said} is claimed and cannot be shown"
        );
    }
}

/// It runs the panel and is handed the file, which is what `%f` is. Without it
/// the panel opens on nothing and the press appears to do nothing at all.
#[test]
fn the_desktop_file_hands_the_panel_the_file_that_was_opened() {
    let exec = desktop_file()
        .lines()
        .find_map(|line| line.strip_prefix("Exec="))
        .expect("an Exec line")
        .to_string();

    assert!(exec.contains("viewer-panel"), "{exec}");
    assert!(exec.trim_end().ends_with("%f"), "{exec}");
}

/// The third copy, and the one most likely to drift.
///
/// `files/etc/xdg/mimeapps.list` is what this desktop ships as its own answer,
/// the way it already does for music. It names every type again, so it is a
/// third list of the same family -- and the whole argument of the entry this
/// settles is that the copy nobody crossed is the one with the hole in it.
///
/// What is asserted is all of the family or none of it, and the *none* is not
/// a softening. It is the state this ships in until the card is drawn: a
/// default pointing at a program that starts and shows nothing is worse than
/// the foreign one it replaces, so the lines go in with the drawing. What must
/// never happen is the third state -- some of the family switched over and the
/// rest left to whatever claims it last -- because that is exactly the shape
/// the `.opus` fault had, and it looks like it works for every type somebody
/// happens to try.
#[test]
fn this_desktop_takes_the_whole_family_or_none_of_it() {
    let at = root().join("files/etc/xdg/mimeapps.list");
    let held = std::fs::read_to_string(&at).unwrap_or_else(|fault| panic!("{}: {fault}", at.display()));

    let ours: BTreeSet<String> = held
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(_, opens)| opens.trim() == "console-viewer.desktop")
        .map(|(mime, _)| mime.trim().to_string())
        .collect();

    if ours.is_empty() {
        return;
    }

    let claimed = claimed();

    let unclaimed: Vec<&String> = ours.difference(&claimed).collect();
    assert!(unclaimed.is_empty(), "set onto this panel and not claimed by it: {unclaimed:?}");

    let unset: Vec<&String> = claimed.difference(&ours).collect();
    assert!(
        unset.is_empty(),
        "half the family is switched over and the rest is left to whatever claims it last: {unset:?}"
    );
}

/// Nothing this panel is set for is a type the music panel is also set for.
/// Two defaults for one type is a machine picking by the order of a file.
#[test]
fn no_type_is_handed_to_two_panels() {
    let at = root().join("files/etc/xdg/mimeapps.list");
    let held = std::fs::read_to_string(&at).unwrap_or_else(|fault| panic!("{}: {fault}", at.display()));

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (mime, _) in held.lines().filter_map(|line| line.split_once('=')) {
        let mime = mime.trim().to_string();
        assert!(seen.insert(mime.clone()), "{mime} is set twice");
    }
}

/// What a claimed type needs, held against what the manifest installs.
///
/// The fault this exists for is invisible on the machine that develops this,
/// by construction: a developer's laptop has everything, so every type decodes
/// and every test passes. `libheif` was exactly that -- an optional dependency
/// of glycin, present here because something else wanted it, absent on a
/// device rebuilt from the manifest alone, and the loader for the two types a
/// phone camera writes.
#[test]
fn every_decoder_a_claimed_type_needs_is_a_package_the_manifest_names() {
    let at = root().join("desktop.conf");
    let held = std::fs::read_to_string(&at).unwrap_or_else(|fault| panic!("{}: {fault}", at.display()));

    let mut named: BTreeSet<&str> = BTreeSet::new();
    let mut inside = false;
    for line in held.lines() {
        if line.starts_with('[') {
            inside = line.trim() == "[packages]";
            continue;
        }
        let line = line.trim();
        if inside && !line.is_empty() && !line.starts_with('#') {
            named.insert(line);
        }
    }

    let missing: Vec<&str> =
        console_viewer::decoding::packages().into_iter().filter(|one| !named.contains(one)).collect();

    assert!(
        missing.is_empty(),
        "these decode something this panel claims and the manifest does not install them: {missing:?}"
    );
}

/// The other way. A type claimed with nothing said about what decodes it is a
/// file that opens a card and then cannot fill it.
#[test]
fn every_type_this_panel_claims_says_what_decodes_it() {
    let undecodable: Vec<String> = claimed()
        .into_iter()
        .filter(|mime| console_viewer::decoding::decoder(mime).is_none())
        .collect();

    assert!(undecodable.is_empty(), "claimed with no decoder named: {undecodable:?}");
}

/// And nothing in the table is for a type this panel does not claim, which
/// would be a package in the manifest for a file nobody can open here.
#[test]
fn nothing_is_installed_for_a_type_this_panel_does_not_claim() {
    let claimed = claimed();
    let spare: Vec<&str> = console_viewer::decoding::DECODERS
        .iter()
        .map(|one| one.mime)
        .filter(|mime| !claimed.contains(*mime))
        .collect();

    assert!(spare.is_empty(), "decoders named for types nothing claims: {spare:?}");
}

/// The viewer reaches a person all at once, or not at all.
///
/// Twice now this panel has nearly shipped in pieces, and the second one was
/// worse than the first. Pointing `mimeapps.list` at a binary that does not
/// draw needed somebody to open a photograph before anything looked wrong.
/// The `.desktop` file needed nothing at all: the menu does not read mimeapps,
/// it walks every `.desktop` under the applications directories and lists
/// whatever does not say `NoDisplay`, so the entry would have appeared as
/// **Pictures** on the home screen the desktop opens into, and pressing it
/// would have printed to a stderr nobody can see.
///
/// What is asserted is what a person can reach, not what the manifest says.
/// The file has to be in `[files]` regardless -- `the_tree.rs` holds that
/// nothing sits under `files/` unclaimed, and it is right to -- so being
/// listed is not the question. Being *shown* is. Either this panel is on the
/// menu and set as the default for what it opens, or it is neither, and the
/// commit that draws the card is the commit that changes both.
///
/// `[build]` is deliberately not part of it. A binary in `/usr/local/bin` that
/// nothing names is one nobody can reach, and having it built is worth
/// something on its own: it means `console apply` compiles this crate on the
/// device rather than finding out it cannot the day it is wanted.
#[test]
fn the_panel_is_shown_and_set_as_the_default_together_or_not_at_all() {
    let manifest = root().join("desktop.conf");
    let held = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|fault| panic!("{}: {fault}", manifest.display()));

    let listed = held
        .lines()
        .any(|line| line.trim() == "/usr/share/applications/console-viewer.desktop");
    assert!(listed, "nothing may sit under files/ unclaimed, so the entry has to be listed");

    let hidden = desktop_file()
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("NoDisplay=true"));

    let mimeapps = root().join("files/etc/xdg/mimeapps.list");
    let defaults = std::fs::read_to_string(&mimeapps)
        .unwrap_or_else(|fault| panic!("{}: {fault}", mimeapps.display()));
    let is_default = defaults.lines().any(|line| line.trim().ends_with("=console-viewer.desktop"));

    assert_eq!(
        !hidden, is_default,
        "this panel is shown on the menu: {}; it is the default for what it opens: {is_default}. \
         Both or neither -- a card on the home screen that opens nothing is the worse half, \
         because it needs nobody to go looking for it.",
        !hidden
    );
}
