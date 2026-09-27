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

use console_core_never::Never;
use console_panel::icons::EVERY;
use console_repository::sources::{Spelled, Word, of_every_crate, spells};

type Failure = Box<dyn std::error::Error>;

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn sources() -> Result<Vec<PathBuf>, Never> {
    let root = Path::new(ROOT);
    let ourself = root.join(file!());
    let declaring = root.join("crates/console-panel/src/icons.rs");

    of_every_crate(root, &[&ourself, &declaring])
}

#[test]
fn nothing_hands_gtk_an_icon_name_it_spelled_itself() -> Result<(), Failure> {
    let doors = [format!("{}_icon_name(", "set"), format!("{}_icon_name(", "from")];
    let mut strange: Vec<String> = Vec::new();
    let Ok(every) = sources();

    for at in every {
        let said = std::fs::read_to_string(&at)?;

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

    Ok(())
}

#[test]
fn nothing_named_here_has_stopped_being_drawn() -> Result<(), Failure> {
    let Ok(every) = sources();
    let read: Vec<String> = every.iter().map(std::fs::read_to_string).collect::<Result<_, _>>()?;
    let said = read.join("\n");
    let gone: Vec<&str> = EVERY
        .iter()
        .filter(|icon| spells(&said, Word(&format!("Icon::{icon:?}"))) == Ok(Spelled::No))
        .map(|icon| {
            let Ok(name) = icon.name();

            name
        })
        .collect();

    assert!(gone.is_empty(), "the enum names what nothing draws: {gone:?}");

    Ok(())
}
