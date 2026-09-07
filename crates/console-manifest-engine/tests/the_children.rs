//! Nothing holds a child process without saying how long it means to hold it.
//!
//! The fault this is here for did not look like a fault for six days. A
//! settings panel opened in a nested desktop starts `pactl subscribe` to hear
//! about the volume, and a nested desktop is torn down rather than closed, so
//! the panel died on a path that never reached the line that stopped its
//! watcher. One orphan per run, sixty-five runs, and then pipewire-pulse --
//! which serves sixty-four clients and refuses the sixty-fifth -- turned away
//! every `pactl` on the machine. What that looked like from a keyboard was the
//! volume keys not working.
//!
//! The panel's mistake is not visible at the line that made it. `spawn` hands
//! back a `Child`, a `Child` that is dropped is not killed, and a field holding
//! one says nothing about whether that is meant. `console-program-lifetime` has
//! the two answers as types -- `Alongside`, which dies with whoever started it,
//! and `LetGo`, which is meant not to -- and both of them survive the paths
//! nobody thought about, because one is a kernel signal and the other is a
//! drop. What this test does is make them the only answers available: a
//! `Child` named anywhere else is a third answer, given by not answering.
//!
//! Two things it deliberately does not look at.
//!
//! `tests/` is not read. A test child is bounded by the `cargo test` run that
//! started it, and the harness ends the run; what these rules are about is a
//! program that stays up for days on a handheld with nobody watching it.
//!
//! And it reads the type, not `spawn`. A command that is spawned, read to the
//! end and waited for inside one function is over before that function returns
//! and cannot be left behind -- `ffmpeg` decoding a wallpaper is that, and so
//! is the `notify-send` that is asked a question with a timeout on it. What is
//! kept is what has to be answered for, and keeping one means naming the type.

use std::path::{Path, PathBuf};

const EXCUSED: [&str; 2] = ["console-program-lifetime", "console-test-desktop"];

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn sources() -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else { return };

        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.is_dir() => walk(&path, into),
                path if path.extension().is_some_and(|end| end == "rs") => into.push(path),
                _ => {}
            }
        }
    }

    let mut found = Vec::new();
    let Ok(crates) = std::fs::read_dir(root().join("crates")) else { return Vec::new() };

    for crate_ in crates.flatten().map(|entry| entry.path()) {
        let named = crate_.file_name().and_then(|name| name.to_str()).unwrap_or("").to_string();

        match EXCUSED.contains(&named.as_str()) {
            true => continue,
            false => {}
        }

        walk(&crate_.join("src"), &mut found);
    }

    found.sort();
    found
        .into_iter()
        .filter_map(|at| std::fs::read_to_string(&at).ok().map(|said| (at, said)))
        .collect()
}

fn without_comments(said: &str) -> String {
    said.lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<&str>>()
        .join("\n")
}

fn says_the_word(said: &str, word: &str) -> bool {
    said.match_indices(word).any(|(at, _)| {
        let before = said
            .get(..at)
            .and_then(|earlier| earlier.chars().next_back())
            .is_none_or(|letter| !letter.is_alphanumeric() && letter != '_');
        let after = said
            .get(at.saturating_add(word.len())..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|letter| !letter.is_alphanumeric() && letter != '_');

        before && after
    })
}

#[test]
fn nothing_holds_a_child_without_saying_how_long_for() {
    let word = format!("{}{}", "Chi", "ld");
    let holding: Vec<String> = sources()
        .into_iter()
        .filter(|(_, said)| says_the_word(&without_comments(said), &word))
        .map(|(at, _)| at.display().to_string())
        .collect();

    assert!(
        holding.is_empty(),
        "these hold a process without saying whether it outlives them; ask \
         console_program_lifetime for an Alongside or a LetGo: {holding:?}"
    );
}

#[test]
fn the_crate_that_declares_them_still_does() {
    let declaring = root().join("crates/console-program-lifetime/src/lib.rs");
    let said = std::fs::read_to_string(declaring).expect("the crate that holds the two answers");

    assert!(said.contains("pub struct Alongside"), "the one that dies with us is gone");
    assert!(said.contains("pub struct LetGo"), "the one that is meant not to is gone");
    assert!(
        said.contains("PR_SET_PDEATHSIG"),
        "the kernel's half is gone, and the drop is alone again"
    );
}

#[test]
fn every_excused_crate_is_a_crate() {
    let missing: Vec<&str> = EXCUSED
        .iter()
        .filter(|named| !root().join("crates").join(named).is_dir())
        .copied()
        .collect();

    assert!(missing.is_empty(), "excused from a rule but not in the tree: {missing:?}");
}
