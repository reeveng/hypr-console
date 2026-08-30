//! The defaults tab: which program opens what.
//!
//! A machine with two browsers on it opens links in whichever one the last
//! thing to touch the setting preferred, and on this machine the way to change
//! that was to know that `xdg-settings` exists. This is that setting, for the
//! handful of kinds of thing anybody on this device actually opens.
//!
//! What can be chosen is read off the machine rather than written down here.
//! Every program says for itself which kinds of file it opens, in the same
//! desktop file the menu draws it from, so a browser installed tomorrow is on
//! this tab tomorrow and nothing here has to hear about it.
//!
//! Reading a desktop file is the whole of the fiddly part and it is here, away
//! from the machine, so it can be tested against the awkward ones: a file with
//! no name, a file that asks not to be shown, a file that is not a program.

use std::collections::BTreeMap;

use console_panel::page::{Does, NOW, Row, Showing, YET};

use crate::rows::DEFAULTS;

/// A kind of thing that gets opened, and the type it is asked for by.
///
/// The browser is first because it is the one anybody notices. The rest are
/// there because a machine that can change one of these and not the others is
/// a machine that has a setting for the thing somebody complained about.
pub struct Kind {
    pub says: &'static str,
    pub mime: &'static str,
}

/// What this tab offers, in the order it offers them.
pub const KINDS: [Kind; 5] = [
    Kind { says: "Links", mime: "x-scheme-handler/https" },
    Kind { says: "Pictures", mime: "image/png" },
    Kind { says: "Video", mime: "video/mp4" },
    Kind { says: "Music", mime: "audio/mpeg" },
    Kind { says: "Text", mime: "text/plain" },
];

/// A program, as its own desktop file describes it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Program {
    /// The desktop file's own name, which is what xdg is told.
    pub id: String,
    pub says: String,
    pub opens: Vec<String>,
}

impl Program {
    /// Whether it says it opens this kind of thing.
    pub fn opens_a(&self, mime: &str) -> bool {
        self.opens.iter().any(|kind| kind == mime)
    }
}

/// One desktop file, as far as this cares about it.
///
/// Only the first group is read. A desktop file may hold several, and the ones
/// after the first are the actions on its right-click menu: a browser declares
/// "new private window" that way, and reading them would offer the browser
/// twice under two names.
///
/// Nothing that asks not to be shown is offered, and nothing that is not a
/// program: a desktop file can describe a directory or a link, and neither of
/// those opens anything.
pub fn program(id: &str, held: &str) -> Option<Program> {
    let mut fields: BTreeMap<&str, &str> = BTreeMap::new();
    for line in held.lines().map(str::trim) {
        if line.starts_with('[') && !fields.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once('=') {
            fields.entry(name.trim()).or_insert(value.trim());
        }
    }

    if fields.get("Type") != Some(&"Application") {
        return None;
    }
    if fields.get("NoDisplay") == Some(&"true") || fields.get("Hidden") == Some(&"true") {
        return None;
    }
    let says = fields.get("Name").filter(|name| !name.is_empty())?;
    Some(Program {
        id: id.to_string(),
        says: (*says).to_string(),
        opens: fields
            .get("MimeType")
            .unwrap_or(&"")
            .split(';')
            .filter(|kind| !kind.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

/// What the tab holds: one row for each kind, saying what opens it now and
/// opening onto what else could.
///
/// Every kind and every program at once was the whole answer to a question
/// nobody had asked yet. Five headings with every browser, viewer and player on
/// the machine under them is a tab that has to be read down to find the one
/// line anybody came for, and the line about music sat between somebody and the
/// line about links. A row that says "Links   LibreWolf" is the setting and its
/// reading in one, and what else there is to choose is a press away rather than
/// on the screen.
///
/// A kind nothing on the machine opens is still listed. Leaving it out would
/// make the tab a different shape on every machine, and somebody looking for
/// the setting they were told about would find nothing and not know whether
/// they were in the wrong place or had nothing to choose from.
pub fn defaults_rows(
    programs: &[Program],
    now: &dyn Fn(&str) -> String,
    open: impl Fn(usize) -> Does,
) -> Vec<Row> {
    KINDS
        .iter()
        .enumerate()
        .map(|(at, kind)| Row::new(kind.says, &in_effect(programs, &now(kind.mime)), open(at)).opening())
        .collect()
}

/// The tab before the machine has been asked what opens what.
///
/// The settings are the same settings whatever the answer turns out to be, and
/// all the answer decides is the name beside each one. Reading it is every
/// desktop file on the machine and a question to xdg for each kind, which is
/// long enough to watch, so the rows go up at once wearing the mark that says a
/// reading is coming and the names land in a list already on the screen.
pub fn meanwhile_rows(open: impl Fn(usize) -> Does) -> Vec<Row> {
    KINDS
        .iter()
        .enumerate()
        .map(|(at, kind)| Row::new(kind.says, YET, open(at)).opening())
        .collect()
}

/// The name of the program a kind is set to, as far as this machine knows it.
///
/// Nothing where what is set is a program that is not installed. The identifier
/// is what xdg keeps, and saying `librewolf.desktop` on the row would be the
/// panel showing its working rather than an answer.
fn in_effect(programs: &[Program], id: &str) -> String {
    programs
        .iter()
        .find(|program| program.id == id)
        .map(|program| program.says.clone())
        .unwrap_or_default()
}

/// What could open one kind: the way back, the kind it is about, and every
/// program that says it opens that sort of thing, with the one in effect
/// marked.
pub fn choice_rows(
    kind: &'static Kind,
    programs: &[Program],
    now: &dyn Fn(&str) -> String,
    back: impl Fn(&dyn Showing) + Send + Sync + 'static,
    use_: impl Fn(&Kind, &Program) -> Does,
) -> Vec<Row> {
    let mut rows = vec![Row::back(DEFAULTS, back), Row::naming(kind.says, "")];
    let default = now(kind.mime);
    let mut opening: Vec<&Program> =
        programs.iter().filter(|program| program.opens_a(kind.mime)).collect();
    opening.sort_by_key(|program| program.says.to_lowercase());

    if opening.is_empty() {
        rows.push(Row::said("Nothing here opens these", ""));
        return rows;
    }
    for program in opening {
        rows.push(Row::new(
            &program.says,
            match program.id == default {
                true => NOW,
                false => "",
            },
            use_(kind, program),
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing(_: &Kind, _: &Program) -> Does {
        Does::and_stay(|_| ())
    }

    fn opens(_: usize) -> Does {
        Does::and_stay(|_| ())
    }

    /// The list under Links, with one program set as the one in effect.
    fn choices(set: &str) -> Vec<Row> {
        choice_rows(&KINDS[0], &programs(), &|_| set.to_string(), |_| (), nothing)
    }

    fn programs() -> Vec<Program> {
        vec![
            Program {
                id: "librewolf.desktop".to_string(),
                says: "LibreWolf".to_string(),
                opens: vec!["x-scheme-handler/https".to_string(), "image/png".to_string()],
            },
            Program {
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
        let read = program(
            "librewolf.desktop",
            "[Desktop Entry]\nType=Application\nName=LibreWolf\nMimeType=text/html;image/png;\n",
        )
        .expect("a program");
        assert_eq!(read.says, "LibreWolf");
        assert!(read.opens_a("image/png"));
        assert!(!read.opens_a("video/mp4"));
    }

    /// The groups after the first are a program's right-click actions, and
    /// reading them offered every browser twice under two names.
    #[test]
    fn only_the_first_group_of_a_desktop_file_is_read() {
        let read = program(
            "librewolf.desktop",
            "[Desktop Entry]\nType=Application\nName=LibreWolf\n\
             [Desktop Action new-private-window]\nName=New Private Window\n",
        )
        .expect("a program");
        assert_eq!(read.says, "LibreWolf");
    }

    #[test]
    fn a_file_that_asks_not_to_be_shown_is_not_offered() {
        let hidden = "[Desktop Entry]\nType=Application\nName=A helper\nNoDisplay=true\n";
        assert_eq!(program("helper.desktop", hidden), None);
    }

    /// A desktop file can describe a directory or a link, and neither of those
    /// opens anything.
    #[test]
    fn a_file_that_is_not_a_program_is_not_offered() {
        assert_eq!(program("a.desktop", "[Desktop Entry]\nType=Link\nName=A site\n"), None);
        assert_eq!(program("b.desktop", "[Desktop Entry]\nName=Nameless\n"), None);
        assert_eq!(program("c.desktop", "[Desktop Entry]\nType=Application\nName=\n"), None);
    }

    #[test]
    fn the_program_in_effect_is_the_one_marked() {
        let rows = choices("chromium.desktop");
        let chromium = rows.iter().find(|row| row.says == "Chromium").expect("a row");
        let librewolf = rows.iter().find(|row| row.says == "LibreWolf").expect("a row");
        assert!(chromium.now());
        assert!(!librewolf.now());
    }

    /// A tab that is a different shape on every machine is a tab nobody can be
    /// told how to use.
    #[test]
    fn every_kind_is_a_row_of_the_tab_in_the_order_it_is_written_down() {
        let rows = defaults_rows(&programs(), &|_| String::new(), opens);
        assert_eq!(says(&rows), ["Links", "Pictures", "Video", "Music", "Text"]);
    }

    /// Otherwise there is nothing to say a row is a way in but pressing it.
    #[test]
    fn every_setting_says_that_it_opens_onto_something() {
        let rows = defaults_rows(&programs(), &|_| String::new(), opens);
        assert!(rows.iter().all(|row| row.opens), "a setting that does not say it opens");
    }

    #[test]
    fn a_setting_reads_as_the_name_of_what_it_is_set_to() {
        let rows = defaults_rows(&programs(), &|_| "librewolf.desktop".to_string(), opens);
        assert_eq!(rows[0].aside, "LibreWolf");
    }

    /// The identifier is what xdg keeps and `librewolf.desktop` on the row
    /// would be the panel showing its working. A kind set to something that is
    /// not installed says nothing at all.
    #[test]
    fn a_setting_pointed_at_a_program_that_is_gone_says_nothing() {
        let rows = defaults_rows(&programs(), &|_| "dolphin.desktop".to_string(), opens);
        assert_eq!(rows[0].aside, "");
    }

    #[test]
    fn a_list_under_a_setting_is_the_way_back_and_then_what_it_is_about() {
        let rows = choices("");
        assert!(rows[0].says.ends_with(DEFAULTS), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Links");
        assert!(rows[1].heading(), "the kind is read rather than chosen");
    }

    #[test]
    fn a_kind_nothing_opens_says_so_on_its_own_list() {
        let rows = choice_rows(&KINDS[3], &programs(), &|_| String::new(), |_| (), nothing);
        assert_eq!(says(&rows)[1..], ["Music", "Nothing here opens these"]);
    }
}
