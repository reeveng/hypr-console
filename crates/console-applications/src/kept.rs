//! The applications the menu found last time it was opened.
//!
//! Reading the machine is every desktop file under three directories, a look
//! down `PATH` for each one that names a program it might not have, and a
//! picture found for each one that is left. That is quick on a warm machine and
//! it is not quick on the first menu after a boot, which is exactly when it is
//! opened.
//!
//! What it found is the same list it is about to find again, though: an
//! application is installed once and opened for months. So the list is written
//! down as it is read, and the menu opens on what was written down while it
//! reads the machine behind that. The applications land in a card that is
//! already the right height, and the rows do not move under a thumb that has
//! started down them.
//!
//! Under the cache, beside the icon index, because it is a thing that can be
//! worked out again. Clearing it costs one menu that opens the way every menu
//! opened before there was a cache.
//!
//! A line per application: what it is called, what it runs, whether it wants a
//! terminal round it, and the file its picture is in. Tab-separated, like the
//! icon index, and a line that is not four fields is not an application --
//! which is what makes a half-written file a shorter menu rather than a menu
//! that will not draw.

use std::collections::BTreeMap;

use console_core_never::Never;

use crate::entry::Application;

pub struct Kept {
    pub app: Application,
    pub picture: String,
}

fn field(said: &str) -> Result<String, Never> {
    Ok(said.replace(['\t', '\r', '\n'], " "))
}

pub fn written(
    apps: &BTreeMap<String, Application>,
    icon: &BTreeMap<String, String>,
) -> Result<String, Never> {
    let mut said = String::new();

    for app in apps.values() {
        let terminal = match app.terminal {
            true => "terminal",
            false => "",
        };
        let picture = icon.get(&app.name).map(String::as_str).unwrap_or_default();

        let name = field(&app.name)?;

        let command = field(&app.command)?;

        let picture = field(picture)?;

        said.push_str(&format!("{name}\t{command}\t{terminal}\t{picture}\n"));
    }

    Ok(said)
}

pub fn read(said: &str) -> Result<Vec<Kept>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();

            let [name, command, terminal, picture] = fields.as_slice() else { return None };

            match name.is_empty() || command.is_empty() {
                true => return None,
                false => {},
            }

            Some(Kept {
                app: Application {
                    name: name.to_string(),
                    command: command.to_string(),
                    terminal: *terminal == "terminal",
                    icon: String::new(),
                },
                picture: picture.to_string(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn one(name: &str, command: &str, terminal: bool) -> Application {
        Application {
            name: name.to_string(),
            command: command.to_string(),
            terminal,
            icon: "whatever".to_string(),
        }
    }

    fn both() -> (BTreeMap<String, Application>, BTreeMap<String, String>) {
        let apps = BTreeMap::from([
            ("LibreWolf".to_string(), one("LibreWolf", "librewolf", false)),
            ("Top".to_string(), one("Top", "htop", true)),
            ("Plain".to_string(), one("Plain", "plain", false)),
        ]);
        let icon = BTreeMap::from([
            ("LibreWolf".to_string(), "/usr/share/icons/librewolf.svg".to_string()),
            ("Top".to_string(), "/usr/share/icons/htop.png".to_string()),
        ]);
        (apps, icon)
    }

    #[test]
    fn what_was_written_is_what_is_read() {
        let (apps, icon) = both();
        let back = ok(read(&ok(written(&apps, &icon))));
        assert_eq!(back.len(), 3);
        let wolf = back.iter().find(|kept| kept.app.name == "LibreWolf").expect("a row");
        assert_eq!(wolf.app.command, "librewolf");
        assert!(!wolf.app.terminal);
        assert_eq!(wolf.picture, "/usr/share/icons/librewolf.svg");
        let top = back.iter().find(|kept| kept.app.name == "Top").expect("a row");
        assert!(top.app.terminal, "a program that wants a terminal round it");
    }

    #[test]
    fn an_application_with_no_picture_is_still_an_application() {
        let (apps, icon) = both();
        let back = ok(read(&ok(written(&apps, &icon))));
        let plain = back.iter().find(|kept| kept.app.name == "Plain").expect("a row");
        assert_eq!(plain.picture, "");
    }

    #[test]
    fn a_line_that_is_not_an_application_is_not_a_row() {
        assert!(ok(read("")).is_empty());
        assert!(ok(read("LibreWolf")).is_empty(), "no fields");
        assert!(ok(read("LibreWolf\tlibrewolf\t")).is_empty(), "three fields");
        assert!(ok(read("\tlibrewolf\t\t")).is_empty(), "nothing to call it");
        assert!(ok(read("LibreWolf\t\t\t")).is_empty(), "nothing to run");
        assert_eq!(ok(read("A\tb\t\t\nrubbish\nC\td\t\t")).len(), 2, "the good lines stand");
    }

    #[test]
    fn a_name_with_a_tab_in_it_is_still_one_field() {
        let apps =
            BTreeMap::from([("A\tB".to_string(), one("A\tB", "run\tit", false))]);
        let back = ok(read(&ok(written(&apps, &BTreeMap::new()))));
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].app.name, "A B");
        assert_eq!(back[0].app.command, "run it");
    }
}
