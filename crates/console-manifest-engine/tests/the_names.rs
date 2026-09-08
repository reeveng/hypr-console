//! A desktop entry is named for something this tree still uses.
//!
//! The filename of a `.desktop` file is not decoration and it is not the name
//! anybody reads: `Name=` inside it is what a person sees. The filename is the
//! identity every other file points at. `mimeapps.list` names one to say what
//! opens a song, and the menu reads the directory. So it is a name, in the
//! sense every other name here is, and it goes stale the same way -- silently,
//! and only on the machines that already applied it, because an apply installs
//! a name and has never removed one.
//!
//! Two had gone stale before anything looked. `console-music.desktop` named a
//! crate that had been renamed, and `console-dictate.desktop` named nothing on
//! the machine at all -- the crate is `console-input-dictation`, the binary is
//! `dictate`, and `console-dictate` lived in that filename and nowhere else.
//! Neither was a fault anybody could see, and that is the argument for reading
//! it here rather than noticing it.
//!
//! What counts as still used is deliberately wide: a crate, a binary the
//! manifest builds, or a package it installs. All three are names somebody can
//! look up, and a rule that demanded only crates would rename
//! `console-buttons.desktop` -- which names the binary a person types -- into
//! something nobody has ever called it. The rule is that the name exists, not
//! that it came from one place.
//!
//! The mime list is read for this desktop's own entries only. It also names
//! `librewolf.desktop`, which a package installs and this tree has no copy of,
//! and asking whether that file exists would be asking the machine a question
//! the manifest cannot answer.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    from.canonicalize().unwrap_or(from)
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

fn section(held: &str, wanted: &str) -> Vec<String> {
    held.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .fold((Vec::new(), None), |(mut out, at), line| {
            match line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                Some(name) => (out, Some(name.to_string())),
                None => {
                    if at.as_deref() == Some(wanted) {
                        out.push(line.split_whitespace().next().unwrap_or("").to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}

fn crates() -> Vec<String> {
    let mut found = Vec::new();
    let entries = std::fs::read_dir(root().join("crates")).expect("crates");

    for at in entries.flatten().map(|entry| entry.path()) {
        if at.is_dir() {
            if let Some(name) = at.file_name().and_then(|name| name.to_str()) {
                found.push(name.to_string());
            }
        }
    }

    found
}

fn entries() -> Vec<String> {
    let at = root().join("files/usr/share/applications");
    let mut found = Vec::new();

    for path in std::fs::read_dir(at).expect("the applications").flatten() {
        let path = path.path();

        if path.extension().and_then(|kind| kind.to_str()) == Some("desktop") {
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                found.push(stem.to_string());
            }
        }
    }

    found.sort();

    found
}

fn known() -> Vec<String> {
    let held = manifest();
    let mut names = crates();

    names.extend(section(&held, "build"));
    names.extend(section(&held, "packages"));
    names.extend(section(&held, "packages").iter().map(|name| format!("console-{name}")));

    names
}

#[test]
fn every_desktop_entry_is_named_for_something_this_tree_still_uses() {
    let known = known();
    let stale: Vec<String> =
        entries().into_iter().filter(|name| !known.contains(name)).collect();

    assert!(
        stale.is_empty(),
        "these name nothing on the machine: {stale:?}\n\
         a desktop entry's filename is what mimeapps.list and the menu point at, \
         so rename it to a crate, a built binary or a package -- and sweep the old \
         path in migrations/, because an apply installs a name and never removes one",
    );
}

#[test]
fn every_entry_the_mime_list_points_at_is_one_this_tree_installs() {
    let list = std::fs::read_to_string(root().join("files/etc/xdg/mimeapps.list"))
        .expect("mimeapps.list");
    let held = entries();

    let missing: Vec<String> = list
        .lines()
        .filter_map(|line| line.split_once('='))
        .flat_map(|(_kind, said)| said.split(';').map(str::trim).map(str::to_string))
        .filter(|said| said.starts_with("console-") && said.ends_with(".desktop"))
        .filter(|said| !held.contains(&said.trim_end_matches(".desktop").to_string()))
        .collect();

    assert!(
        missing.is_empty(),
        "mimeapps.list points at entries this tree does not install: {missing:?}",
    );
}

#[test]
fn the_manifest_installs_every_entry_that_is_in_the_tree() {
    let held = manifest();
    let installed = section(&held, "files");

    let unnamed: Vec<String> = entries()
        .into_iter()
        .filter(|name| {
            !installed.contains(&format!("/usr/share/applications/{name}.desktop"))
        })
        .collect();

    assert!(unnamed.is_empty(), "in the tree and not in [files]: {unnamed:?}");
}
