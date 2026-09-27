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

const NO_PICTURE: &str = "";


pub struct CachedApplication {
    pub application: Application,
    pub picture: String,
}

fn field(said: &str) -> Result<String, Never> {
    Ok(said.replace(['\t', '\r', '\n'], " "))
}

pub fn serialize(
    applications: &BTreeMap<String, Application>,
    icon: &BTreeMap<String, String>,
) -> Result<String, Never> {
    let mut said = String::new();

    for application in applications.values() {
        let terminal = match application.terminal {
            true => "terminal",
            false => "",
        };
        let picture = match icon.get(&application.name) {
            Some(picture) => picture.as_str(),
            None => NO_PICTURE,
        };

        let name = field(&application.name)?;

        let command = field(&application.command)?;

        let picture = field(picture)?;

        said.push_str(&format!("{name}\t{command}\t{terminal}\t{picture}\n"));
    }

    Ok(said)
}

pub fn read(said: &str) -> Result<Vec<CachedApplication>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();

            let (name, command, terminal, picture) = match fields.as_slice() {
                [name, command, terminal, picture] => (name, command, terminal, picture),
                _not_four_fields => return None,
            };

            match name.is_empty() || command.is_empty() {
                true => return None,
                false => {},
            }

            Some(CachedApplication {
                application: Application {
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
    use std::error::Error;

    fn app(named: Named<'_>) -> Result<Application, Never> {
        Ok(Application {
            name: named.name.to_string(),
            command: named.command.to_string(),
            terminal: false,
            icon: "whatever".to_string(),
        })
    }

    struct Named<'a> {
        name: &'a str,
        command: &'a str,
    }

    type Both = (BTreeMap<String, Application>, BTreeMap<String, String>);

    fn both() -> Result<Both, Never> {
        let Ok(wolf) = app(Named { name: "LibreWolf", command: "librewolf" });
        let Ok(top) = app(Named { name: "Top", command: "htop" });
        let Ok(plain) = app(Named { name: "Plain", command: "plain" });
        let top = Application { terminal: true, ..top };
        let applications = BTreeMap::from([
            ("LibreWolf".to_string(), wolf),
            ("Top".to_string(), top),
            ("Plain".to_string(), plain),
        ]);
        let icon = BTreeMap::from([
            ("LibreWolf".to_string(), "/usr/share/icons/librewolf.svg".to_string()),
            ("Top".to_string(), "/usr/share/icons/htop.png".to_string()),
        ]);

        Ok((applications, icon))
    }

    #[test]
    fn what_was_written_is_what_is_read() -> Result<(), Box<dyn Error>> {
        let Ok((applications, icon)) = both();
        let Ok(said) = serialize(&applications, &icon);
        let Ok(back) = read(&said);

        assert_eq!(back.len(), 3);

        let wolf = back.iter().find(|kept| kept.application.name == "LibreWolf").ok_or("a row")?;
        assert_eq!(wolf.application.command, "librewolf");
        assert!(!wolf.application.terminal);
        assert_eq!(wolf.picture, "/usr/share/icons/librewolf.svg");

        let top = back.iter().find(|kept| kept.application.name == "Top").ok_or("a row")?;
        assert!(top.application.terminal, "a program that wants a terminal round it");

        Ok(())
    }

    #[test]
    fn an_application_with_no_picture_is_still_an_application() -> Result<(), Box<dyn Error>> {
        let Ok((applications, icon)) = both();
        let Ok(said) = serialize(&applications, &icon);
        let Ok(back) = read(&said);
        let plain = back.iter().find(|kept| kept.application.name == "Plain").ok_or("a row")?;
        assert_eq!(plain.picture, "");

        Ok(())
    }

    #[test]
    fn a_line_that_is_not_an_application_is_not_a_row() {
        for (said, why) in [
            ("", "nothing"),
            ("LibreWolf", "no fields"),
            ("LibreWolf\tlibrewolf\t", "three fields"),
            ("\tlibrewolf\t\t", "nothing to call it"),
            ("LibreWolf\t\t\t", "nothing to run"),
        ] {
            assert_eq!(read(said).map(|rows| rows.len()), Ok(0), "{why}");
        }

        assert_eq!(read("A\tb\t\t\nrubbish\nC\td\t\t").map(|rows| rows.len()), Ok(2), "the good lines stand");
    }

    #[test]
    fn a_name_with_a_tab_in_it_is_still_one_field() {
        let Ok(tabbed) = app(Named { name: "A\tB", command: "run\tit" });
        let applications = BTreeMap::from([("A\tB".to_string(), tabbed)]);
        let Ok(said) = serialize(&applications, &BTreeMap::new());
        let Ok(back) = read(&said);

        assert_eq!(back.len(), 1);
        assert_eq!(back.first().map(|kept| kept.application.name.as_str()), Some("A B"));
        assert_eq!(back.first().map(|kept| kept.application.command.as_str()), Some("run it"));
    }
}
