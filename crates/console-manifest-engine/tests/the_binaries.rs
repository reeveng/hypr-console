//! What a program in `[build]` is called, and which crate it came out of.
//!
//! There are two kinds of name in that list and only one of them ever had a
//! rule. A binary is named for what someone types is true of `console` itself
//! and of the handful a person reaches for by hand, and false of `bar-clock`,
//! `home-square`, `panel-pictures` and `files-thumbnails`, which no one has ever
//! typed: they are reached for by a unit, by the bar's configuration, by a
//! keybinding or by another program of ours. With no rule covering them each
//! was named by whoever wrote it, which is the whole of why `[build]` reads
//! like twenty separate ideas.
//!
//! So: a **command** is typed and opens with `console-` -- or is `console`
//! itself, which is the one program named for the whole desktop rather than for
//! a part of it -- and a **part** is
//! reached for by something else and opens with a word its own crate's name
//! holds. A part then says which crate it came out of, and `[build]` sorted
//! is `ls crates/` sorted -- which is what makes the list readable rather than
//! a pile.
//!
//! The word rather than the whole name, because a crate's name is its job in
//! as few words as say it and a binary is one thing that crate does: `bar-clock`
//! out of `console-status-bar` and `home-square` out of `console-home-screen`
//! are each a part of what the crate is for, and demanding the whole subject
//! would name them `status-bar-clock` and `home-screen-square`. The family word
//! comes off first, because `console-input-keyboard` is in the `input` family
//! and its subject is the keyboard.
//!
//! What this cannot ask is whether the word a part opens with is the one worth
//! opening with. `bar-` names what consumes the output rather than what
//! produces it, and it reads as a family only because three crates happen to
//! agree; the test catches the two that do not agree and a reader has to catch
//! the rest. That is the same half a name test always leaves behind, and it is
//! the reason the crate rule is a walk rather than a lint.

mod reading;

use reading::section;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const THE_ENGINE: &str = "console";

const FAMILIES: [&str; 5] = ["core", "input", "manifest", "program", "test"];

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    from.canonicalize().unwrap_or(from)
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

fn built_by() -> BTreeMap<String, String> {
    let mut held = BTreeMap::new();
    let crates = std::fs::read_dir(root().join("crates")).expect("crates");

    for at in crates.flatten().map(|entry| entry.path()) {
        let named = match at.file_name().and_then(|named| named.to_str()) {
            Some(named) => named.to_string(),
            None => continue,
        };

        let said = match std::fs::read_to_string(at.join("Cargo.toml")) {
            Ok(said) => said,
            Err(_nothing_there) => continue,
        };

        let mut inside = false;

        for line in said.lines().map(str::trim) {
            match line.starts_with('[') {
                true => inside = line == "[[bin]]",
                false => match inside.then(|| line.strip_prefix("name")).flatten() {
                    Some(rest) => match rest.trim_start_matches([' ', '=']).trim_matches('"') {
                        "" => {},
                        binary => {
                            held.insert(binary.to_string(), named.clone());
                        },
                    },
                    None => {},
                },
            }
        }
    }

    held
}

fn subject(crate_: &str) -> Vec<String> {
    let said = crate_.strip_prefix("console-").unwrap_or(crate_);
    let words: Vec<&str> = said.split('-').collect();

    let kept = match words.split_first() {
        Some((first, rest)) if FAMILIES.contains(first) && !rest.is_empty() => rest.to_vec(),
        _ => words,
    };

    kept.into_iter().map(str::to_string).collect()
}

#[test]
fn every_program_the_manifest_builds_is_one_a_crate_here_declares() {
    let built = built_by();
    let strange: Vec<String> =
        section(&manifest(), "build").into_iter().filter(|named| !built.contains_key(named)).collect();

    assert!(
        strange.is_empty(),
        "[build] names what no crate builds: {strange:?}",
    );
}

#[test]
fn a_part_is_named_for_the_crate_it_came_out_of() {
    let built = built_by();

    let strange: Vec<String> = section(&manifest(), "build")
        .into_iter()
        .filter(|named| named != THE_ENGINE && !named.starts_with("console-"))
        .filter_map(|named| built.get(&named).map(|crate_| (named.clone(), crate_.clone())))
        .filter(|(named, crate_)| {
            let opening = named.split('-').next().unwrap_or(named);

            !subject(crate_).iter().any(|word| word == opening)
        })
        .map(|(named, crate_)| format!("{named}, out of {crate_}"))
        .collect();

    assert!(
        strange.is_empty(),
        "a program in [build] is either a command, which is typed and opens with console-, \
         or a part, which is reached for by something else and opens with a word of the crate \
         it came out of -- these are neither: {strange:?}",
    );
}
