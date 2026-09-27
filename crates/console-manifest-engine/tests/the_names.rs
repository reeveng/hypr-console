//! A desktop entry is named for something this tree still uses.
//!
//! The filename of a `.desktop` file is not decoration and it is not the name
//! anyone reads: `Name=` inside it is what a person sees. The filename is the
//! identity every other file points at. `mimeapps.list` names one to say what
//! opens a song, and the menu reads the directory. So it is a name, in the
//! sense every other name here is, and it goes stale the same way -- silently,
//! and only on the machines that already applied it, because an apply installs
//! a name and has never removed one.
//!
//! Two had gone stale before anything looked. `console-music.desktop` named a
//! crate that had been renamed, and `console-dictate.desktop` named nothing on
//! the machine at all -- the crate was `console-input-dictation`, the binary
//! was `dictate`, and that filename was the only place the third name existed.
//! Neither was a fault anyone could see, and that is the argument for reading
//! it here rather than noticing it.
//!
//! Both of those names have since come round: the crate is `console-music`
//! again and the binary a person runs is `console-dictate`, so the two entries
//! this test was written about are the two it now passes. That is the shape of
//! the thing rather than a coincidence. A filename is right for as long as
//! something crosses it against what the tree says today, and a rename that
//! moves the other half is exactly how it stops being.
//!
//! What counts as still used is deliberately wide: a crate, a binary the
//! manifest builds, or a package it installs. All three are names someone can
//! look up, and a rule that demanded only crates would rename
//! `console-buttons.desktop` -- which names the binary a person types -- into
//! something no one has ever called it. The rule is that the name exists, not
//! that it came from one place.
//!
//! The mime list is read for this desktop's own entries only. It also names
//! `librewolf.desktop`, which a package installs and this tree has no copy of,
//! and asking whether that file exists would be asking the machine a question
//! the manifest cannot answer.

mod reading;

use std::collections::BTreeSet;

use reading::{Failure, Section, section, read, root};

fn crates() -> Result<Vec<String>, Failure> {
    let Ok(root) = root();
    let mut found = Vec::new();

    let entries = std::fs::read_dir(root.join("crates"))?;


    for entry in entries {
        let entry = entry?;
        let at = entry.path();

        match (at.is_dir(), at.file_name().and_then(|name| name.to_str())) {
            (true, Some(name)) => found.push(name.to_string()),
            (true, None) | (false, _) => {},
        }
    }

    Ok(found)
}

fn entries() -> Result<Vec<String>, Failure> {
    let Ok(root) = root();
    let mut found = Vec::new();

    let entries = std::fs::read_dir(root.join("files/usr/share/applications"))?;


    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let desktop = path.extension().and_then(|kind| kind.to_str()) == Some("desktop");

        match (desktop, path.file_stem().and_then(|stem| stem.to_str())) {
            (true, Some(stem)) => found.push(stem.to_string()),
            (true, None) | (false, _) => {},
        }
    }

    found.sort();

    Ok(found)
}

fn known() -> Result<BTreeSet<String>, Failure> {
    let held = read("desktop.conf")?;
    let built = section(&held, Section::Build)?;
    let packages = section(&held, Section::Packages)?;
    let crates = crates()?;
    let mut names: BTreeSet<String> = crates.into_iter().collect();

    names.extend(built);
    names.extend(packages.iter().map(|name| format!("console-{name}")));
    names.extend(packages);

    Ok(names)
}

#[test]
fn every_desktop_entry_is_named_for_something_this_tree_still_uses() -> Result<(), Failure> {
    let known = known()?;
    let entries = entries()?;
    let stale: Vec<String> = entries.into_iter().filter(|name| !known.contains(name)).collect();

    assert!(
        stale.is_empty(),
        "these name nothing on the machine: {stale:?}\n\
         a desktop entry's filename is what mimeapps.list and the menu point at, \
         so rename it to a crate, a built binary or a package -- and sweep the old \
         path in a migration, because an apply installs a name and never removes one",
    );

    Ok(())
}

#[test]
fn every_entry_the_mime_list_points_at_is_one_this_tree_installs() -> Result<(), Failure> {
    let list = read("files/etc/xdg/mimeapps.list")?;
    let entries = entries()?;
    let held: BTreeSet<String> = entries.into_iter().collect();

    let missing: Vec<String> = list
        .lines()
        .filter_map(|line| line.split_once('='))
        .flat_map(|(_kind, said)| said.split(';').map(str::trim).map(str::to_string))
        .filter(|said| said.starts_with("console-") && said.ends_with(".desktop"))
        .filter(|said| !held.contains(said.trim_end_matches(".desktop")))
        .collect();

    assert!(
        missing.is_empty(),
        "mimeapps.list points at entries this tree does not install: {missing:?}",
    );

    Ok(())
}

#[test]
fn the_manifest_installs_every_entry_that_is_in_the_tree() -> Result<(), Failure> {
    let held = read("desktop.conf")?;
    let files = section(&held, Section::Files)?;
    let installed: BTreeSet<String> = files.into_iter().collect();

    let entries = entries()?;
    let unnamed: Vec<String> = entries
        .into_iter()
        .filter(|name| !installed.contains(&format!("/usr/share/applications/{name}.desktop")))
        .collect();

    assert!(unnamed.is_empty(), "in the tree and not in [files]: {unnamed:?}");

    Ok(())
}
