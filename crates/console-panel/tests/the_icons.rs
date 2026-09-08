//! Nothing in the tree spells an icon name.
//!
//! `console_panel::icons::Icon` is where they come from, and the two ways a
//! row or a button gets one -- `Picture::Named` and `Press::new` -- take the
//! enum rather than a string, so the compiler does most of this. What it
//! cannot reach is a GTK call made directly: `set_icon_name` and
//! `from_icon_name` both take a `&str`, and a name handed to either is a name
//! no check would ever see.
//!
//! So this reads the tree for those two calls and asks that neither is ever
//! given a string literal. Every one of them names an `Icon` instead, which is
//! the whole point: the device-tier check crosses the enum against the theme
//! the machine has, and a name that never reached the enum is a broken square
//! nothing was watching for.

use std::path::{Path, PathBuf};

use console_panel::icons::EVERY;

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn sources() -> Vec<PathBuf> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(at) {
            Ok(entries) => entries,
            Err(_fault) => return,
        };
        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.is_dir() => walk(&path, into),
                path if path.extension().is_some_and(|end| end == "rs") => into.push(path),
                _ => {}
            }
        }
    }
    let ourself = root().join(file!());
    let declaring = root().join("crates/console-panel/src/icons.rs");
    let mut found = Vec::new();
    let crates = match std::fs::read_dir(root().join("crates")) {
        Ok(crates) => crates,
        Err(_fault) => return found,
    };
    for crate_ in crates.flatten().map(|entry| entry.path()) {
        for held in ["src", "tests", "examples"] {
            walk(&crate_.join(held), &mut found);
        }
    }
    found.retain(|at| at != &ourself && at != &declaring);
    found.sort();
    found
}

#[test]
fn nothing_hands_gtk_an_icon_name_it_spelled_itself() {
    let doors = [format!("{}_icon_name(", "set"), format!("{}_icon_name(", "from")];
    let mut strange: Vec<String> = Vec::new();

    for at in sources() {
        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(_fault) => continue,
        };

        for door in &doors {
            for (found, _) in said.match_indices(door.as_str()) {
                let from = found.saturating_add(door.len());
                let rest = said.get(from..).unwrap_or_default();
                let spelled = rest.trim_start().starts_with('"')
                    || rest.trim_start().starts_with("Some(\"");

                match spelled {
                    true => strange.push(format!("{}: {door}", at.display())),
                    false => {},
                }
            }
        }
    }

    assert!(
        strange.is_empty(),
        "these spell an icon name instead of asking console_panel::icons for one: {strange:?}"
    );
}

fn said_exactly(said: &str, what: &str) -> bool {
    said.match_indices(what).any(|(at, _)| {
        said.get(at.saturating_add(what.len())..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|letter| !letter.is_alphanumeric() && letter != '_')
    })
}

#[test]
fn nothing_named_here_has_stopped_being_drawn() {
    let said: String = sources()
        .iter()
        .filter_map(|at| std::fs::read_to_string(at).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let gone: Vec<&str> = EVERY
        .iter()
        .filter(|icon| !said_exactly(&said, &format!("Icon::{icon:?}")))
        .map(|icon| {
            let Ok(name) = icon.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing draws: {gone:?}");
}
