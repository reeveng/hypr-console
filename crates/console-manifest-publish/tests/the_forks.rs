//! The boundary between what is ours and what is somebody else's, asked of the
//! tree rather than of a fixture.  `tree.rs` proves that `is_fork` answers the
//! way it is written to. That is not the fault this desktop has actually had.
//! Twice now the list and the tree have simply stopped describing each other --
//! a binary renamed while `FORKS` kept the old path, so the list protected a
//! file nobody had and the new one was carried; a source directory brought in
//! with nothing naming it at all -- and in both cases every unit test went on
//! passing, because the list was consistent with itself.  So these ask the two
//! of them together. Every compiled program in `files/` has to be a fork the
//! list names; every fork the list names has to be a thing that is there; and
//! the manifest has to name each one in a form the filter can recognise,
//! because the filter is what keeps it out of the copy.  They run in the
//! published copy too -- `console-manifest-publish` builds it and runs the
//! suite inside it -- so each one says what it means there as well, which is
//! the opposite: none of it should have arrived.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_manifest_publish::tree::{FORKS, Fork, VENDORED, carried, is_fork, manifest};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn the_published_copy() -> bool {
    root().join("docs/forks.md").exists()
}

fn in_the_tree(fork: &str) -> PathBuf {
    root().join("files").join(fork.trim_start_matches('/'))
}

fn everything_under(holding: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut asking = vec![holding.to_path_buf()];
    while let Some(here) = asking.pop() {
        let inside = match std::fs::read_dir(&here) {
            Ok(inside) => inside,
            Err(_fault) => continue,
        };
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

fn is_a_compiled_program(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|held| held.starts_with(b"\x7fELF"))
}

fn as_tracked(path: &Path) -> String {
    let root = root().canonicalize().unwrap_or_else(|_| root());
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    path.strip_prefix(&root)
        .unwrap_or(&path)
        .to_string_lossy()
        .into_owned()
}

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
        .filter(|name| {
            let Ok(fork) = is_fork(name);

            fork == Fork::No
        })
        .collect();
    assert!(
        loose.is_empty(),
        "these are compiled programs in files/ that console_manifest_publish::tree does not exclude, so \
         the public copy would carry them: {loose:?}. Either add the path to FORKS, or do not \
         check a binary in."
    );
}

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

#[test]
fn every_vendored_crate_the_list_names_is_a_directory_with_something_in_it() {
    for source in VENDORED {
        let at = root().join(source);

        assert!(at.is_dir(), "VENDORED names {source}, which is not a directory here");
        assert!(
            !everything_under(&at).is_empty(),
            "{source} is named as somebody else's work and holds nothing"
        );
    }
}

#[test]
fn a_vendored_fork_keeps_the_licence_it_came_with() {
    for source in VENDORED {
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

    let Ok(written) = manifest(&held);
    let (files, elsewhere) = written.split_once("[elsewhere]").expect("a note about the forks");
    for fork in FORKS {
        assert!(!files.lines().any(|line| line.trim() == fork), "{fork} survived into [files]");
        assert!(elsewhere.contains(fork), "{fork} went out without being named");
    }
}

#[test]
fn nothing_a_fork_owns_survives_being_carried() {
    if the_published_copy() {
        return;
    }
    let Ok(mut git) = Program::Git.command();
    let listed = git
        .args(["-C", &root().to_string_lossy(), "ls-files"])
        .output();
    let listed = match listed {
        Ok(listed) => listed,
        Err(_fault) => return,
    };
    if !listed.status.success() {
        return;
    }
    let tracked: Vec<String> = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    assert!(!tracked.is_empty(), "git listed nothing, so this test asked nothing");

    let Ok(kept) = carried(tracked.clone());
    assert_eq!(
        kept.len(),
        tracked.len() - FORKS.len(),
        "the filter took out a number of files the list does not account for"
    );

    for name in &kept {
        for fork in FORKS {
            assert_ne!(
                name.as_str(),
                fork.trim_start_matches('/'),
                "a fork binary survived being carried"
            );
            assert!(!name.ends_with(fork), "{name} survived being carried");
        }
    }
}
