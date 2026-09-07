//! The defaults tab: which application opens what.
//!
//! A machine with two browsers on it opens links in whichever one the last
//! thing to touch the setting preferred, and on this machine the way to change
//! that was to know that `xdg-settings` exists. This is that setting, for the
//! handful of kinds of thing anybody on this device actually opens.
//!
//! What can be chosen is read off the machine rather than written down here.
//! Every application says for itself which kinds of file it opens, in the same
//! desktop file the menu draws it from, so a browser installed tomorrow is on
//! this tab tomorrow and nothing here has to hear about it.
//!
//! Reading a desktop file is the whole of the fiddly part and it is here, away
//! from the machine, so it can be tested against the awkward ones: a file with
//! no name, a file that asks not to be shown, a file that is not an application.

use std::collections::BTreeMap;

use console_core_never::Never;
use console_panel::page::{Does, NOW, Row, Showing, YET};

use crate::rows::configuration;

pub struct Kind {
    pub says: &'static str,
    pub mime: &'static str,
    pub and: &'static [&'static str],
}

impl Kind {
    pub fn every(&self) -> Result<impl Iterator<Item = &'static str> + use<>, Never> {
        Ok(std::iter::once(self.mime).chain(self.and.iter().copied()))
    }
}

pub const KINDS: [Kind; 6] = [
    Kind { says: "Links", mime: "x-scheme-handler/https", and: &[] },
    Kind {
        says: "Pictures",
        mime: "image/png",
        and: &["image/jpeg", "image/webp", "image/gif", "image/avif", "image/heif", "image/tiff"],
    },
    Kind {
        says: "Video",
        mime: "video/mp4",
        and: &["video/matroska", "video/webm", "video/quicktime", "video/vnd.avi", "video/ogg"],
    },
    Kind {
        says: "Music",
        mime: "audio/mpeg",
        and: &[
            "audio/flac",
            "audio/ogg",
            "audio/x-opus+ogg",
            "audio/x-vorbis+ogg",
            "audio/x-flac+ogg",
            "audio/mp4",
            "audio/aac",
            "audio/vnd.wave",
        ],
    },
    Kind { says: "Folders", mime: "inode/directory", and: &[] },
    Kind { says: "Text", mime: "text/plain", and: &[] },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opens {
    It,
    Not,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Application {
    pub id: String,
    pub says: String,
    pub opens: Vec<String>,
}

impl Application {
    pub fn opens_a(&self, mime: &str) -> Result<Opens, Never> {
        match self.opens.iter().any(|kind| kind == mime) {
            true => Ok(Opens::It),
            false => Ok(Opens::Not),
        }
    }
}

pub fn application(id: &str, held: &str) -> Result<Option<Application>, Never> {
    let mut fields: BTreeMap<&str, &str> = BTreeMap::new();

    for line in held.lines().map(str::trim) {
        match line.starts_with('[') && !fields.is_empty() {
            true => break,
            false => {},
        }

        match line.split_once('=') {
            Some((name, value)) => {
                fields.entry(name.trim()).or_insert(value.trim());
            }
            None => {},
        }
    }

    match fields.get("Type") == Some(&"Application") {
        true => {},
        false => return Ok(None),
    }

    match fields.get("NoDisplay") == Some(&"true") || fields.get("Hidden") == Some(&"true") {
        true => return Ok(None),
        false => {},
    }

    let Some(says) = fields.get("Name").filter(|name| !name.is_empty()) else {
        return Ok(None);
    };

    Ok(Some(Application {
        id: id.to_string(),
        says: (*says).to_string(),
        opens: fields
            .get("MimeType")
            .unwrap_or(&"")
            .split(';')
            .filter(|kind| !kind.is_empty())
            .map(str::to_string)
            .collect(),
    }))
}

pub fn defaults_rows(
    applications: &[Application],
    now: &dyn Fn(&str) -> Result<String, Never>,
    open: impl Fn(usize) -> Result<Does, Never>,
) -> Result<Vec<Row>, Never> {
    Ok(KINDS
        .iter()
        .enumerate()
        .map(|(at, kind)| {
            let Ok(opening) = now(kind.mime);
            let Ok(said) = in_effect(applications, &opening);
            let Ok(opens) = open(at);
            let Ok(row) = Row::new(kind.says, &said, opens);
            let Ok(opens) = row.opening();

            opens
        })
        .collect())
}

pub fn meanwhile_rows(open: impl Fn(usize) -> Result<Does, Never>) -> Result<Vec<Row>, Never> {
    Ok(KINDS
        .iter()
        .enumerate()
        .map(|(at, kind)| {
            let Ok(opens) = open(at);
            let Ok(row) = Row::new(kind.says, YET, opens);
            let Ok(opens) = row.opening();

            opens
        })
        .collect())
}

fn in_effect(applications: &[Application], id: &str) -> Result<String, Never> {
    Ok(applications
        .iter()
        .find(|application| application.id == id)
        .map(|application| application.says.clone())
        .unwrap_or_default())
}

pub fn choice_rows(
    kind: &'static Kind,
    applications: &[Application],
    now: &dyn Fn(&str) -> Result<String, Never>,
    back: impl Fn(&dyn Showing) + Send + Sync + 'static,
    use_: impl Fn(&Kind, &Application) -> Result<Does, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(configuration) = configuration();
    let Ok(way_back) = Row::back(&configuration, back);
    let Ok(naming) = Row::naming(kind.says, "");
    let mut rows = vec![way_back, naming];
    let Ok(default) = now(kind.mime);
    let mut opening: Vec<&Application> = applications
        .iter()
        .filter(|application| {
            let Ok(opens) = application.opens_a(kind.mime);

            opens == Opens::It
        })
        .collect();
    opening.sort_by_key(|application| application.says.to_lowercase());

    match opening.is_empty() {
        true => {
            let Ok(row) = Row::nothing("Nothing here opens these");

            rows.push(row);

            return Ok(rows);
        }
        false => {},
    }

    for application in opening {
        let Ok(uses) = use_(kind, application);
        let Ok(row) = Row::new(
            &application.says,
            match application.id == default {
                true => NOW,
                false => "",
            },
            uses,
        );

        rows.push(row);
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use console_panel::page::{Heading, InEffect};
    use super::*;

    fn nothing(_: &Kind, _: &Application) -> Result<Does, Never> {
        Does::and_stay(|_| ())
    }

    fn opens(_: usize) -> Result<Does, Never> {
        Does::and_stay(|_| ())
    }

    fn choices(set: &str) -> Vec<Row> {
        let Ok(rows) =
            choice_rows(&KINDS[0], &applications(), &|_| Ok(set.to_string()), |_| (), nothing);

        rows
    }

    fn defaults(now: &dyn Fn(&str) -> Result<String, Never>) -> Vec<Row> {
        let Ok(rows) = defaults_rows(&applications(), now, opens);

        rows
    }

    fn now(row: &Row) -> InEffect {
        let Ok(now) = row.now();

        now
    }

    fn heading(row: &Row) -> Heading {
        let Ok(heading) = row.heading();

        heading
    }

    fn applications() -> Vec<Application> {
        vec![
            Application {
                id: "librewolf.desktop".to_string(),
                says: "LibreWolf".to_string(),
                opens: vec!["x-scheme-handler/https".to_string(), "image/png".to_string()],
            },
            Application {
                id: "chromium.desktop".to_string(),
                says: "Chromium".to_string(),
                opens: vec!["x-scheme-handler/https".to_string()],
            },
        ]
    }

    fn says(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.says.as_str()).collect()
    }

    #[test]
    fn a_desktop_file_gives_its_name_and_what_it_opens() {
        let read = application(
            "librewolf.desktop",
            "[Desktop Entry]\nType=Application\nName=LibreWolf\nMimeType=text/html;image/png;\n",
        )
        .expect("the reading")
        .expect("an application");
        assert_eq!(read.says, "LibreWolf");
        assert_eq!(read.opens_a("image/png"), Ok(Opens::It));
        assert_eq!(read.opens_a("video/mp4"), Ok(Opens::Not));
    }

    #[test]
    fn only_the_first_group_of_a_desktop_file_is_read() {
        let read = application(
            "librewolf.desktop",
            "[Desktop Entry]\nType=Application\nName=LibreWolf\n\
             [Desktop Action new-private-window]\nName=New Private Window\n",
        )
        .expect("the reading")
        .expect("an application");
        assert_eq!(read.says, "LibreWolf");
    }

    #[test]
    fn a_file_that_asks_not_to_be_shown_is_not_offered() {
        let hidden = "[Desktop Entry]\nType=Application\nName=A helper\nNoDisplay=true\n";
        assert_eq!(application("helper.desktop", hidden), Ok(None));
    }

    #[test]
    fn a_file_that_is_not_a_program_is_not_offered() {
        assert_eq!(application("a.desktop", "[Desktop Entry]\nType=Link\nName=A site\n"), Ok(None));
        assert_eq!(application("b.desktop", "[Desktop Entry]\nName=Nameless\n"), Ok(None));
        assert_eq!(
            application("c.desktop", "[Desktop Entry]\nType=Application\nName=\n"),
            Ok(None)
        );
    }

    #[test]
    fn the_program_in_effect_is_the_one_marked() {
        let rows = choices("chromium.desktop");
        let chromium = rows.iter().find(|row| row.says == "Chromium").expect("a row");
        let librewolf = rows.iter().find(|row| row.says == "LibreWolf").expect("a row");
        assert_eq!(now(chromium), InEffect::Yes);
        assert_eq!(now(librewolf), InEffect::No);
    }

    #[test]
    fn every_kind_is_a_row_of_the_tab_in_the_order_it_is_written_down() {
        let rows = defaults(&|_| Ok(String::new()));
        assert_eq!(says(&rows), ["Links", "Pictures", "Video", "Music", "Folders", "Text"]);
    }

    #[test]
    fn every_setting_says_that_it_opens_onto_something() {
        let rows = defaults(&|_| Ok(String::new()));
        assert!(rows.iter().all(|row| row.opens), "a setting that does not say it opens");
    }

    #[test]
    fn a_setting_reads_as_the_name_of_what_it_is_set_to() {
        let rows = defaults(&|_| Ok("librewolf.desktop".to_string()));
        assert_eq!(rows[0].aside, "LibreWolf");
    }

    #[test]
    fn a_setting_pointed_at_a_program_that_is_gone_says_nothing() {
        let rows = defaults(&|_| Ok("dolphin.desktop".to_string()));
        assert_eq!(rows[0].aside, "");
    }

    #[test]
    fn a_list_under_a_setting_is_the_way_back_and_then_what_it_is_about() {
        let rows = choices("");
        let Ok(configuration) = configuration();
        assert!(rows[0].says.ends_with(&configuration), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Links");
        assert_eq!(heading(&rows[1]), Heading::Yes, "the kind is read rather than chosen");
    }

    #[test]
    fn a_kind_nothing_opens_says_so_on_its_own_list() {
        let Ok(rows) =
            choice_rows(&KINDS[3], &applications(), &|_| Ok(String::new()), |_| (), nothing);
        assert_eq!(says(&rows)[1..], ["Music", "Nothing here opens these"]);
    }
}
