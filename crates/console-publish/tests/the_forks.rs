//! The boundary between what is ours and what is somebody else's, asked of
//! the tree rather than of a fixture.
//!
//! `tree.rs` proves that `is_fork` answers the way it is written to. That is
//! not the fault this desktop has actually had. Twice now the list and the
//! tree have simply stopped describing each other -- a binary renamed while
//! `FORKS` kept the old path, so the list protected a file nobody had and the
//! new one was carried; a source directory brought in with nothing naming it
//! at all -- and in both cases every unit test went on passing, because the
//! list was consistent with itself.
//!
//! So these ask the two of them together. Every compiled program in `files/`
//! has to be a fork the list names; every fork the list names has to be a
//! thing that is there; and the manifest has to name each one in a form the
//! filter can recognise, because the filter is what keeps it out of the copy.
//!
//! They run in the published copy too -- `console-publish` builds it and runs
//! the suite inside it -- so each one says what it means there as well, which
//! is the opposite: none of it should have arrived.

use std::path::{Path, PathBuf};
use std::process::Command;

use console_publish::tree::{FORKS, FORK_SOURCES, Fork, carried, is_fork, manifest};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether this is the published copy rather than the tree it was made from.
///
/// The copy is the one with `docs/forks.md` in it. The tree keeps that paper
/// as this crate's own `papers/forks.md` and writes it out under that name
/// only on the way out, so its presence at the root is the copy's signature
/// and needs nothing -- no git, no marker file -- to be true.
fn the_published_copy() -> bool {
    root().join("docs/forks.md").exists()
}

/// Where a fork's binary sits in the tree, from the path the manifest names.
fn in_the_tree(fork: &str) -> PathBuf {
    root().join("files").join(fork.trim_start_matches('/'))
}

/// Every file under a directory, however deep.
fn everything_under(holding: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut asking = vec![holding.to_path_buf()];
    while let Some(here) = asking.pop() {
        let Ok(inside) = std::fs::read_dir(&here) else { continue };
        for entry in inside.flatten() {
            let path = entry.path();
            match path.is_dir() {
                true => asking.push(path),
                false => found.push(path),
            }
        }
    }
    found.sort();
    found
}

/// Whether a file begins with ELF's four bytes, which is what a compiled
/// program on this machine begins with and what a config file does not.
fn is_a_compiled_program(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|held| held.starts_with(b"\x7fELF"))
}

/// The path as the exclusion asks about it: relative to the repository root,
/// which is the form `git ls-files` hands over.
fn as_tracked(path: &Path) -> String {
    let root = root().canonicalize().unwrap_or_else(|_| root());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.strip_prefix(&root)
        .unwrap_or(&path)
        .to_string_lossy()
        .into_owned()
}

/// Every compiled program in the tree is one the exclusion knows about.
///
/// This is the one that would have caught it. A binary is carried here only
/// because somebody else wrote the program and we only forked it, and the
/// moment one is in `files/` without being in `FORKS` the public copy carries
/// somebody's GPL program with no source beside it. Asked of what is on disk
/// rather than of the list, so adding a binary and forgetting the list is a
/// failing test rather than a quiet mirror.
#[test]
fn every_compiled_program_in_the_tree_is_a_fork_the_list_names() {
    let programs: Vec<PathBuf> = everything_under(&root().join("files"))
        .into_iter()
        .filter(|path| is_a_compiled_program(path))
        .collect();

    if the_published_copy() {
        assert!(
            programs.is_empty(),
            "the published copy carries compiled programs: {programs:?}. Every one of them is \
             somebody else's work published without its source."
        );
        return;
    }

    let loose: Vec<String> = programs
        .iter()
        .map(|path| as_tracked(path))
        .filter(|name| is_fork(name) == Fork::No)
        .collect();
    assert!(
        loose.is_empty(),
        "these are compiled programs in files/ that console_publish::tree does not exclude, so \
         the public copy would carry them: {loose:?}. Either add the path to FORKS, or do not \
         check a binary in."
    );
}

/// Every fork the list names is a file that is there.
///
/// A list naming a path nothing has protects nothing, and reads exactly like
/// a list that is working. This is the half of the last fault that the test
/// above does not cover: the binary was renamed and `FORKS` kept the old
/// path, so both halves were wrong and neither said so.
#[test]
fn every_fork_the_list_names_is_a_file_that_is_there() {
    for fork in FORKS {
        let at = in_the_tree(fork);
        match the_published_copy() {
            true => assert!(!at.exists(), "the copy carries {fork}, which is the whole of what \
                                           the exclusion is for"),
            false => assert!(
                at.exists(),
                "FORKS names {fork} and {} is not there. A fork list that names a path nothing \
                 has excludes nothing, and looks the same as one that works.",
                at.display()
            ),
        }
    }
}

/// And every fork source it names is a directory that is there, with
/// something in it.
#[test]
fn every_fork_source_the_list_names_is_a_directory_with_something_in_it() {
    for source in FORK_SOURCES {
        let at = root().join(source);
        match the_published_copy() {
            true => assert!(!at.exists(), "the copy carries the source tree {source}"),
            false => {
                assert!(at.is_dir(), "FORK_SOURCES names {source}, which is not a directory here");
                assert!(
                    !everything_under(&at).is_empty(),
                    "{source} is named as a fork's source and holds nothing"
                );
            },
        }
    }
}

/// A vendored fork keeps the licence it came with.
///
/// The exclusion is a decision about publishing, not a way out of the
/// licence: the source is GPL-3 wherever it sits, and a copy of somebody's
/// GPL program with their licence file dropped is the one thing that would
/// actually be wrong.
#[test]
fn a_vendored_fork_keeps_the_licence_it_came_with() {
    if the_published_copy() {
        return;
    }
    for source in FORK_SOURCES {
        let held = everything_under(&root().join(source));
        let licences: Vec<&PathBuf> = held
            .iter()
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| matches!(name.to_string_lossy().as_ref(),
                                                 "COPYING" | "LICENSE" | "LICENCE"))
            })
            .collect();
        assert!(
            !licences.is_empty(),
            "{source} is somebody else's source and carries no COPYING or LICENSE"
        );
        for licence in licences {
            let held = std::fs::read_to_string(licence).unwrap_or_default();
            assert!(!held.trim().is_empty(), "{} is empty", licence.display());
        }
    }
}

/// The manifest names each fork in a form the filter can recognise.
///
/// `tree::manifest` drops a line by matching the trimmed line against
/// `FORKS`. That is a string comparison, so a path written in `desktop.conf`
/// any other way -- renamed, or with anything after it on the line -- is a
/// fork the copy publishes with no test anywhere going red. Asked of the real
/// manifest, both ways round.
#[test]
fn the_real_manifest_names_every_fork_in_a_form_the_filter_recognises() {
    let held = std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf");

    if the_published_copy() {
        let (files, elsewhere) = held.split_once("[elsewhere]").expect("the copy says elsewhere");
        for fork in FORKS {
            assert!(
                !files.lines().any(|line| line.trim() == fork),
                "the published manifest still has {fork} in [files]"
            );
            assert!(elsewhere.contains(fork), "the published manifest does not say where {fork} went");
        }
        return;
    }

    for fork in FORKS {
        assert!(
            held.lines().any(|line| line.trim() == fork),
            "desktop.conf does not have {fork} on a line of its own, so tree::manifest will not \
             take it out and the public copy will name a binary it does not carry."
        );
    }

    // And the filter, run on the real manifest, does take every one of them.
    let written = manifest(&held);
    let (files, elsewhere) = written.split_once("[elsewhere]").expect("a note about the forks");
    for fork in FORKS {
        assert!(!files.lines().any(|line| line.trim() == fork), "{fork} survived into [files]");
        assert!(elsewhere.contains(fork), "{fork} went out without being named");
    }
}

/// Nothing a fork owns survives the copy being built.
///
/// The last two ask about the list. This asks about the answer: the real set
/// of tracked files, put through the real filter, with nothing of either fork
/// left in it. It is the only test here that speaks for what would actually
/// be pushed.
#[test]
fn nothing_a_fork_owns_survives_being_carried() {
    if the_published_copy() {
        return;
    }
    let listed = Command::new("git")
        .args(["-C", &root().to_string_lossy(), "ls-files"])
        .output();
    let Ok(listed) = listed else { return };
    if !listed.status.success() {
        return;
    }
    let tracked: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    assert!(!tracked.is_empty(), "git listed nothing, so this test asked nothing");

    let kept = carried(tracked.clone());
    assert!(kept.len() < tracked.len(), "the filter took nothing out of a tree that has forks in it");

    for name in &kept {
        for fork in FORKS {
            assert_ne!(
                name.as_str(),
                fork.trim_start_matches('/'),
                "a fork binary survived being carried"
            );
            assert!(!name.ends_with(fork), "{name} survived being carried");
        }
        for source in FORK_SOURCES {
            assert!(
                !name.starts_with(&format!("{source}/")) && name != source,
                "{name} is under the fork source {source} and survived being carried"
            );
        }
    }
}
