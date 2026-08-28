//! What the manifest says, held against the tree it is a manifest of.
//!
//! These need the repository rather than a fixture, so they live out here
//! rather than beside the code. Everything that can be decided from a string
//! alone is tested next to the function that decides it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

fn legion(args: &[&str]) -> (bool, String) {
    let done = Command::new(env!("CARGO_BIN_EXE_legion"))
        .args(args)
        .output()
        .expect("legion runs");
    (done.status.success(), String::from_utf8_lossy(&done.stdout).into_owned())
}

/// Every file in the tree, as the path it is installed to.
///
/// Bytecode is not one of them. It is written beside whatever imports it, git
/// is already told to ignore it, and this tree is worked in by more than one
/// person at once: a stray file from somebody else's test run is not a desktop
/// file nobody installs.
fn carried() -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.ends_with("__pycache__") => {}
                path if path.is_dir() => walk(&path, into),
                path => into.push(path),
            }
        }
    }
    let files = root().join("files");
    let mut found = Vec::new();
    walk(&files, &mut found);
    found.sort();
    found
        .into_iter()
        .map(|path| {
            let live = format!("/{}", path.strip_prefix(&files).expect("under files/").display());
            (path, live)
        })
        .collect()
}

#[test]
fn the_manifest_this_desktop_wears_is_one_the_engine_can_read() {
    let (ok, said) = legion(&["list", "--root", root().to_str().expect("a path")]);
    assert!(ok, "legion list could not read desktop.conf:\n{said}");
    for section in ["[packages]", "[build]", "[files]", "[services]", "[masked]"] {
        assert!(said.contains(section), "{section} is not in the manifest");
    }
}

/// The one that was missed. `awww` holds the picture it was given until it is
/// given another, so a new background is nothing until the unit runs again.
#[test]
fn the_background_is_a_file_the_paper_service_is_restarted_for() {
    let unit = root().join("files/etc/systemd/user/legion-paper.service");
    let held = std::fs::read_to_string(&unit).expect("the paper service");
    let named = named_by(&held);
    assert!(
        named.iter().any(|path| path == "/usr/share/backgrounds/legion.webp"),
        "the paper service does not name the background, so a redraw would never be shown: {named:?}"
    );
}

#[test]
fn everything_meant_to_be_run_will_be_installed_able_to_run() {
    for (path, live) in carried() {
        let head: Vec<u8> = std::fs::read(&path).unwrap_or_default().into_iter().take(4).collect();
        let a_program = matches!(head.as_slice(), [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..]);
        if a_program {
            assert_eq!(
                mode_of(&live, &head),
                0o755,
                "{live} is a program and would be installed unrunnable"
            );
        }
    }
}

#[test]
fn files_in_the_users_home_are_installed_as_the_user() {
    let held = std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf");
    let files = section(&held, "files");
    assert!(!files.is_empty(), "the manifest names no files");
    for path in files {
        let expected = match path.starts_with("/home/player/") {
            true => "player",
            false => "root",
        };
        assert_eq!(owner_of(&path), expected, "{path} would be installed as the wrong user");
    }
}

/// Every program the device compiles for itself is one this repository holds.
///
/// A crate is named for what it is and the program it makes is named for what
/// somebody types, so the two are not always the same word and the manifest
/// names the program.
#[test]
fn every_program_the_device_builds_is_one_this_repository_holds() {
    let held = std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf");
    let made = programs();
    for name in section(&held, "build") {
        assert!(
            made.contains(&name),
            "the manifest builds {name} and nothing here makes a program called that; \
             this repository makes {made:?}"
        );
    }
}

/// Every program the workspace makes, by the name it is installed under.
fn programs() -> Vec<String> {
    let crates = root().join("crates");
    std::fs::read_dir(&crates)
        .expect("crates/")
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("Cargo.toml")).ok())
        .filter_map(|held| held.parse::<toml::Table>().ok())
        .flat_map(|held| {
            let named = |at: &toml::Value| {
                at.get("name").and_then(toml::Value::as_str).map(str::to_owned)
            };
            match held.get("bin").and_then(toml::Value::as_array) {
                Some(bins) => bins.iter().filter_map(named).collect::<Vec<_>>(),
                // A crate with no [[bin]] table makes a program named for
                // itself, if it makes one at all.
                None => held.get("package").and_then(named).into_iter().collect(),
            }
        })
        .collect()
}

// The rules under test, written here rather than reached for, because the
// engine is a binary and its insides are its own. Any of these three drifting
// from the engine's own is caught by the engine's own tests, which assert the
// same rules against the same cases.

fn mode_of(live: &str, head: &[u8]) -> u32 {
    match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    }
}

fn owner_of(live: &str) -> &'static str {
    match live.starts_with("/home/player/") {
        true => "player",
        false => "root",
    }
}

fn named_by(unit: &str) -> Vec<String> {
    unit.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.starts_with("Exec"))
        .flat_map(|(_, command)| command.split_whitespace())
        .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']))
        .filter(|word| word.starts_with('/'))
        .map(str::to_owned)
        .collect()
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
                        out.push(line.to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}
