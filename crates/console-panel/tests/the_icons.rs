//! Nothing in the tree spells an icon name.
//!
//! `console_panel::icons::Icon` is where they come from, and the two ways a
//! row or a button gets one -- `Picture::Named` and `ButtonPress::new` -- take the
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
use console_repository::sources::{Spelled, Word, of_every_crate, spells};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

fn sources() -> Vec<PathBuf> {
    let (ourself, declaring) = (root().join(file!()), root().join("crates/console-panel/src/icons.rs"));
    let Ok(every) = of_every_crate(&root(), &[&ourself, &declaring]);

    every
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
            for rest in said.split(door.as_str()).skip(1) {
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

#[test]
fn nothing_named_here_has_stopped_being_drawn() {
    let said: String = sources()
        .iter()
        .filter_map(|at| std::fs::read_to_string(at).ok())
        .collect::<Vec<_>>()
        .join("\n");
    let gone: Vec<&str> = EVERY
        .iter()
        .filter(|icon| spells(&said, Word(&format!("Icon::{icon:?}"))) == Ok(Spelled::No))
        .map(|icon| {
            let Ok(name) = icon.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing draws: {gone:?}");
}
