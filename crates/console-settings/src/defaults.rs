//! The defaults tab: which application opens what.
//!
//! A machine with two browsers on it opens links in whichever one the last
//! thing to touch the setting preferred, and on this machine the way to change
//! that was to know that `xdg-settings` exists. This is that setting, for the
//! handful of kinds of thing anyone on this device actually opens.
//!
//! What can be chosen is read off the machine rather than written down here.
//! Every application says for itself which kinds of file it opens, in the same
//! desktop file the menu draws it from, so a browser installed tomorrow is on
//! this tab tomorrow and nothing here has to hear about it.
//!
//! Reading a desktop file is the whole of the fiddly part and it is here, away
//! from the machine, so it can be tested against the awkward ones: a file with
//! no name, a file that asks not to be shown, a file that is not an application.

use console_applications::entry::{DesktopEntry, Worth};
use console_core_never::Never;
use console_panel::page::{Aside, Handler, NOW, Row, Showing, YET};

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
        and: &[
            "video/matroska",
            "video/x-matroska",
            "video/webm",
            "video/quicktime",
            "video/vnd.avi",
            "video/ogg",
        ],
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopFilePath<'a>(pub &'a str);

pub fn application(id: &str, held: DesktopFilePath<'_>) -> Result<Option<Application>, Never> {
    let entry = DesktopEntry::read(held.0)?;

    let worth = entry.worth()?;

    match worth {
        Worth::Skipping => return Ok(None),
        Worth::Drawing => {},
    }

    let named = entry.says()?;

    let says = match named {
        Some(says) => says,
        None => return Ok(None),
    };

    let opens = entry.opens()?;

    Ok(Some(Application { id: id.to_string(), says: says.to_string(), opens }))
}

pub fn defaults_rows(
    applications: &[Application],
    now: &dyn Fn(&str) -> Result<String, Never>,
    open: impl Fn(u32) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
    Ok(KINDS
        .iter()
        .enumerate()
        .map(|(at, kind)| {
            let Ok(opening) = now(kind.mime);
            let Ok(said) = in_effect(applications, &opening);
            let Ok(at) = console_core_number_conversion::fitted(at);
            let Ok(opens) = open(at);
            let Ok(row) = Row::new(kind.says, Aside(&said), opens);
            let Ok(opens) = row.opening();

            opens
        })
        .collect())
}

pub fn meanwhile_rows(open: impl Fn(u32) -> Result<Handler, Never>) -> Result<Vec<Row>, Never> {
    Ok(KINDS
        .iter()
        .enumerate()
        .map(|(at, kind)| {
            let Ok(at) = console_core_number_conversion::fitted(at);
            let Ok(opens) = open(at);
            let Ok(row) = Row::new(kind.says, Aside(YET), opens);
            let Ok(opens) = row.opening();

            opens
        })
        .collect())
}

fn in_effect(applications: &[Application], id: &str) -> Result<String, Never> {
    let found =
        applications.iter().find(|application| application.id == id).map(|one| one.says.clone());

    Ok(match found {
        Some(says) => says,
        None => String::new(),
    })
}

pub fn choice_rows(
    kind: &'static Kind,
    applications: &[Application],
    now: &dyn Fn(&str) -> Result<String, Never>,
    back: impl Fn(&dyn Showing) + Send + Sync + 'static,
    use_: impl Fn(&Kind, &Application) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(configuration) = configuration();
    let Ok(way_back) = Row::back(&configuration, back);
    let Ok(naming) = Row::naming(kind.says, Aside(""));
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
            let Ok(row) = Row::placeholder("Nothing here opens these");

            rows.push(row);

            return Ok(rows);
        }
        false => {},
    }

    for application in opening {
        let Ok(uses) = use_(kind, application);
        let Ok(row) = Row::new(
            &application.says,
            Aside(match application.id == default {
                true => NOW,
                false => "",
            }),
            uses,
        );

        rows.push(row);
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use console_panel::page::{Heading, Active};
    use super::*;

    const A_ROW: &str = "a row the list was to have";

    fn nothing(_: &Kind, _: &Application) -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn opens(_: u32) -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn applications() -> Result<Vec<Application>, Never> {
        Ok(vec![
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
        ])
    }

    fn choices(kind: &'static Kind, set: &str) -> Result<Vec<Row>, Never> {
        let Ok(applications) = applications();

        choice_rows(kind, &applications, &|_| Ok(set.to_string()), |_| (), nothing)
    }

    const LINKS_FIRST: &str = "the kinds start with links";

    fn links(set: &str) -> Result<Option<Vec<Row>>, Never> {
        Ok(KINDS.first().map(|links| {
            let Ok(rows) = choices(links, set);

            rows
        }))
    }

    fn defaults(now: &dyn Fn(&str) -> Result<String, Never>) -> Result<Vec<Row>, Never> {
        let Ok(applications) = applications();

        defaults_rows(&applications, now, opens)
    }

    fn says(rows: &[Row]) -> Result<Vec<&str>, Never> {
        Ok(rows.iter().map(|row| row.says.as_str()).collect())
    }

    #[test]
    fn a_desktop_file_gives_its_name_and_what_it_opens() -> Result<(), &'static str> {
        let Ok(read) = application(
            "librewolf.desktop",
            DesktopFilePath(
                "[Desktop Entry]\nType=Application\nName=LibreWolf\n\
                 MimeType=text/html;image/png;\n",
            ),
        );
        let read = read.ok_or("an application")?;

        assert_eq!(read.says, "LibreWolf");
        assert_eq!(read.opens_a("image/png"), Ok(Opens::It));
        assert_eq!(read.opens_a("video/mp4"), Ok(Opens::Not));

        Ok(())
    }

    #[test]
    fn only_the_first_group_of_a_desktop_file_is_read() -> Result<(), &'static str> {
        let Ok(read) = application(
            "librewolf.desktop",
            DesktopFilePath("[Desktop Entry]\nType=Application\nName=LibreWolf\n\
             [Desktop Effect new-private-window]\nName=New Private Window\n"),
        );
        let read = read.ok_or("an application")?;

        assert_eq!(read.says, "LibreWolf");

        Ok(())
    }

    #[test]
    fn a_file_that_asks_not_to_be_shown_is_not_offered() {
        let hidden = "[Desktop Entry]\nType=Application\nName=A helper\nNoDisplay=true\n";

        assert_eq!(application("helper.desktop", DesktopFilePath(hidden)), Ok(None));
    }

    #[test]
    fn a_file_that_is_not_a_program_is_not_offered() {
        assert_eq!(
            application("a.desktop", DesktopFilePath("[Desktop Entry]\nType=Link\nName=A site\n")),
            Ok(None)
        );
        assert_eq!(application("b.desktop", DesktopFilePath("[Desktop Entry]\nName=Untitled\n")), Ok(None));
        assert_eq!(
            application("c.desktop", DesktopFilePath("[Desktop Entry]\nType=Application\nName=\n")),
            Ok(None)
        );
    }

    #[test]
    fn the_program_in_effect_is_the_one_marked() -> Result<(), &'static str> {
        let Ok(rows) = links("chromium.desktop");
        let rows = rows.ok_or(LINKS_FIRST)?;
        let chromium = rows.iter().find(|row| row.says == "Chromium").ok_or(A_ROW)?;
        let librewolf = rows.iter().find(|row| row.says == "LibreWolf").ok_or(A_ROW)?;

        assert_eq!(chromium.now(), Ok(Active::Yes));
        assert_eq!(librewolf.now(), Ok(Active::No));

        Ok(())
    }

    #[test]
    fn every_kind_is_a_row_of_the_tab_in_the_order_it_is_written_down() {
        let Ok(rows) = defaults(&|_| Ok(String::new()));

        assert_eq!(says(&rows), Ok(vec!["Links", "Pictures", "Video", "Music", "Folders", "Text"]));
    }

    #[test]
    fn every_setting_says_that_it_opens_onto_something() {
        let Ok(rows) = defaults(&|_| Ok(String::new()));

        assert!(rows.iter().all(|row| row.opens), "a setting that does not say it opens");
    }

    #[test]
    fn a_setting_reads_as_the_name_of_what_it_is_set_to() -> Result<(), &'static str> {
        let Ok(rows) = defaults(&|_| Ok("librewolf.desktop".to_string()));
        let first = rows.first().ok_or(A_ROW)?;

        assert_eq!(first.aside, "LibreWolf");

        Ok(())
    }

    #[test]
    fn a_setting_pointed_at_a_program_that_is_gone_says_nothing() -> Result<(), &'static str> {
        let Ok(rows) = defaults(&|_| Ok("dolphin.desktop".to_string()));
        let first = rows.first().ok_or(A_ROW)?;

        assert_eq!(first.aside, "");

        Ok(())
    }

    #[test]
    fn a_list_under_a_setting_is_the_way_back_and_then_what_it_is_about() -> Result<(), &'static str> {
        let Ok(rows) = links("");
        let rows = rows.ok_or(LINKS_FIRST)?;
        let Ok(configuration) = configuration();
        let back = rows.first().ok_or(A_ROW)?;
        let named = rows.get(1).ok_or(A_ROW)?;

        assert!(back.says.ends_with(&configuration), "{:?} is not the way back", back.says);
        assert_eq!(named.says, "Links");
        assert_eq!(named.heading(), Ok(Heading::Yes), "the kind is read rather than chosen");

        Ok(())
    }

    #[test]
    fn a_kind_nothing_opens_says_so_on_its_own_list() -> Result<(), &'static str> {
        let music = KINDS.get(3).ok_or("the kinds hold music fourth")?;
        let Ok(rows) = choices(music, "");
        let Ok(said) = says(&rows);

        assert_eq!(said.get(1..), Some(["Music", "Nothing here opens these"].as_slice()));

        Ok(())
    }
}
