//! Every part of the guide, in the order it is learned.
//!
//! One list, read twice: printed as headings in a terminal, and drawn as tabs
//! on the device. A section that exists in one and not the other is how a guide
//! starts lying.
//!
//! What a button does is read off the one table that decides it, grouped by
//! what is held with the button: a section for a press on its own, and one for
//! each trigger held. Nothing here is written by hand about a button, which is
//! the whole point -- a job somebody has moved is a job this guide names on the
//! button they moved it to, and a layer nobody has put anything on is a heading
//! that never appears.

use console_controller::doing::Doing;
use console_controller::means::{Job, Table, What, When};
use console_files::doing::{self, Deed};
use console_pad::jobs::{ALONE, Binding, Layer};

use crate::binds::binds;

/// The section whose rows are things to do rather than things to know.
///
/// One word a tab, where there is one. These are read along a strip on a
/// handheld, at a glance, by somebody who stopped reading because they could
/// not remember a button: a title that has to be read twice has already cost
/// more than it saves.
pub const DOABLE: &str = "Anywhere";

/// The section about what is in front of you when a chooser is up.
pub const MENUS: &str = "Menus";

/// The layers, and what each of them is headed.
///
/// The layer with nothing held is not in here: it is the whole of the rest of
/// the guide. These three are the second thing a button does, and nothing is
/// on R2 or on both triggers as this desktop ships -- so those two headings do
/// not appear at all until somebody puts something there, which is a section
/// read out of the table doing exactly what it should.
const HELD: [(Layer, &str); 3] = [
    (Layer::of(true, false), "L2"),
    (Layer::of(false, true), "R2"),
    (Layer::of(true, true), "L2 + R2"),
];

/// One line of the guide: a button, and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub button: String,
    pub does: String,
    /// What the desktop runs when it is pressed, where anything is.
    pub runs: Option<Vec<String>>,
}

impl Line {
    pub fn new(button: &str, does: &str) -> Self {
        Line { button: button.to_string(), does: does.to_string(), runs: None }
    }
}

/// One heading and its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub lines: Vec<Line>,
}

impl Section {
    fn of(title: &str, lines: Vec<Line>) -> Self {
        Section { title: title.to_string(), lines }
    }
}

/// A button, said the way this guide says it.
///
/// The words on the machine with the dashes taken out and the first letter
/// raised. The d-pad keeps its dash, because that is how it is written
/// everywhere else here and how anybody says it out loud.
pub fn said(button: &str) -> String {
    let said = match button.strip_prefix("dpad-") {
        Some(way) => format!("d-pad {way}"),
        None => button.replace('-', " "),
    };
    let mut letters = said.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

/// The jobs on one layer, as lines, in the order the table holds them.
///
/// A job with nothing on this layer is not a line here, which is what makes a
/// heading with no rows under it a heading nothing prints.
fn lines(table: &Table, layer: Layer, wanted: impl Fn(&Job) -> bool) -> Vec<Line> {
    table
        .every()
        .filter(|(job, _)| wanted(job))
        .filter_map(|(job, bound)| line(job, bound, layer))
        .collect()
}

/// One job, where anything on this layer plays it.
///
/// Two buttons doing one job is one line naming both, which is how the guide
/// has always said the shoulders and how somebody reads it: the button with a
/// keyboard drawn on it and X are not two things to learn.
fn line(job: &Job, bound: &[Binding], layer: Layer) -> Option<Line> {
    let on: Vec<String> = bound
        .iter()
        .filter(|one| one.played() && one.layer == layer)
        .map(|one| said(&one.button))
        .collect();
    match on.is_empty() {
        // A job with no button at all is not a line. This is a guide to what
        // pressing something comes to, and somebody who has taken the button
        // off a job is told about it on the screen where they took it off.
        true => None,
        false => Some(Line {
            button: on.join(" / "),
            does: job.what.says().to_string(),
            runs: runs_for(job.what),
        }),
    }
}

/// What the desktop runs when a job is asked for, where it runs anything.
///
/// Read off the one table that says what a button is for, so the guide cannot
/// promise something the desktop does not do -- and, since that table now
/// carries the jobs this desktop does not itself carry out, cannot quietly
/// omit one either. X was exactly that omission: the guide could only report
/// what the profile said, and the profile said `North`.
///
/// Asked on the way down, because a row that does what it describes is doing
/// the press and not the release. A job that sends a key rather than starting
/// something is nothing a row can do for you: pressing Enter at a guide is not
/// choosing the row the guide is describing.
pub fn runs_for(what: What) -> Option<Vec<String>> {
    match what.does(true)? {
        Doing::Run(argv) => Some(argv),
        Doing::Frame(_) => None,
    }
}

/// What Y offers on a thing in the files, read off the list the panel offers.
///
/// The list is the whole answer, and shorter than a sentence about it. Written
/// once: a deed the files learn is a deed named here, without anybody
/// remembering that the guide exists.
fn what_can_be_done() -> String {
    let said: Vec<&str> = doing::EVERY.iter().map(|deed| Deed::says(*deed)).collect();
    said.join(", ")
}

/// The whole guide.
pub fn sections(table: &Table, lua: &str) -> Vec<Section> {
    // What a press comes to on its own, and then the things on this device
    // that are not buttons at all and so are in no table.
    let mut around = lines(table, ALONE, |job| job.when != When::WithAChooserUp);
    around.extend([
        Line::new("Volume rocker", "louder, quieter, unmute"),
        Line::new("Touchpad", "move the pointer"),
        Line::new("Tap the touchpad", "click"),
        Line::new("Press the touchpad in", "click and hold to drag"),
        Line::new("The screen", "tap to click, drag to scroll"),
        Line::new("The bar", "tap its icons"),
    ]);
    // What a chooser makes of the same buttons, and then the ways of driving
    // one that are the panel's own doing rather than any button's.
    let mut menus = lines(table, ALONE, |job| job.when == When::WithAChooserUp);
    menus.extend([
        Line::new("D-pad", "move the highlight"),
        Line::new("B", "back out"),
        Line::new("X", "show or hide the keyboard"),
        Line::new("Typing", "the top row of a menu that has one"),
        Line::new("D-pad left / right", "move a level"),
        Line::new("Right paddle, top", "close the menu"),
        Line::new("Legion right", "the settings"),
        Line::new("Menu", "this guide"),
        Line::new("Tap a row", "the same as A"),
        Line::new("\u{2039} and \u{203a}", "the tab before or after"),
        Line::new("\u{2212} and +", "move a level with a finger"),
        Line::new("\u{d7}", "close, the same as B"),
        Line::new("Its bar icon", "tap it again to close"),
    ]);

    let mut every = vec![Section::of(DOABLE, around)];
    every.extend(
        HELD.iter().map(|(layer, title)| Section::of(title, lines(table, *layer, |_| true))),
    );
    every.extend([
        Section::of(
            "Keyboard",
            vec![
                Line::new("X", "put the keyboard away"),
                Line::new("A", "press the key you are on"),
                Line::new("B", "backspace"),
                Line::new("Y", "shift"),
                Line::new("D-pad", "move between keys"),
                Line::new("L1 / R1", "previous / next set of keys"),
                Line::new("Menu", "enter"),
                Line::new("Stick press", "press the key you are on"),
            ],
        ),
        Section::of(MENUS, menus),
        Section::of(
            "Files",
            vec![
                Line::new("L1 / R1", "Home, and whatever is plugged in"),
                Line::new("A", "open a folder or a file"),
                Line::new("B", "the folder above"),
                Line::new("Y", &what_can_be_done()),
                Line::new("New folder", "under Y, in whichever folder you are in"),
                Line::new("Copy or Move", "pick it up; a row puts it down"),
                Line::new("Delete", "asks first; goes to the wastebasket"),
                Line::new("Row nought", "the folder above, with a finger"),
            ],
        ),
        Section::of(
            "Music",
            vec![
                Line::new("A", "play a song, or a folder of them"),
                Line::new("Y", "show it in the files, where it is renamed or thrown away"),
                Line::new("Typing", "a song, whose it is, or anything it says"),
                Line::new("D-pad left / right", "the song before it, the song after it"),
                Line::new("Play them in any order", "on Playing, under what is on"),
                Line::new("Play this one over", "on Playing, under what is on"),
            ],
        ),
        // What the buttons come to inside a page, which is the one place on
        // this device where they are not the table's doing. The add-on this
        // desktop packs for the browser is what makes them mean this, and no
        // table mentions any of it, so these rows are hand-written and are a
        // promise somebody has to keep by hand. docs/browser.md is the rest.
        Section::of(
            "Browser",
            vec![
                Line::new("Y", "label everything on the page that can be pressed"),
                Line::new("D-pad", "walk between those things, one at a time"),
                Line::new("A", "take the one you are standing on"),
                Line::new("B", "put the labels away, and then go back a page"),
                Line::new("Y again", "the same labels, opening in a new tab"),
                Line::new("Along the bottom", "look for something, the tabs, a new tab, close this one"),
                Line::new("A new tab", "opens on the line to type a question into"),
                Line::new("X", "the keyboard, for the line being typed into"),
            ],
        ),
        // What the front of the machine means once Steam has the screen, which
        // is almost nothing: it is Steam's there, down to the button that left
        // for it. The one thing this desktop keeps is the hold, and a hold is
        // not something anybody finds by pressing.
        Section::of(
            "Steam",
            vec![
                Line::new("Legion left", "Steam's own menu, which is Steam's to draw"),
                Line::new("Legion left, held", "back to this desktop"),
                Line::new("Everything else", "the pad, untouched, the way a game expects it"),
            ],
        ),
        Section::of(
            "Shortcuts",
            binds(lua)
                .into_iter()
                .map(|bind| Line { button: bind.keys, does: bind.does, runs: Some(bind.runs) })
                .collect(),
        ),
    ]);
    every
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_pad::jobs::Jobs;

    fn ours() -> Table {
        Table::ours()
    }

    fn section<'a>(every: &'a [Section], title: &str) -> &'a Section {
        every.iter().find(|section| section.title == title).expect("a section")
    }

    fn line<'a>(section: &'a Section, button: &str) -> &'a Line {
        section.lines.iter().find(|line| line.button == button).expect("a line")
    }

    /// The second thing a button does is read out of the table rather than
    /// written down here, which is what makes it true.
    #[test]
    fn what_a_trigger_held_makes_of_a_button_comes_from_the_table() {
        let every = sections(&ours(), "");
        let held = section(&every, "L2");
        assert_eq!(line(held, "D-pad up").does, "louder");
        assert_eq!(line(held, "Right paddle bottom").does, "a screenshot");
    }

    /// A layer nobody has put anything on is a heading nothing prints. R2 is
    /// the whole of this desktop's second trigger as it ships: empty.
    #[test]
    fn a_layer_with_nothing_on_it_has_nothing_under_it() {
        let every = sections(&ours(), "");
        assert!(section(&every, "R2").lines.is_empty());
        assert!(section(&every, "L2 + R2").lines.is_empty());
    }

    /// The one thing the whole rework is for: a job somebody moved is named on
    /// the button they moved it to, in the section for the layer they put it
    /// on, and nowhere else.
    #[test]
    fn a_job_somebody_moved_is_named_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let every = sections(&Table::of(&said), "");
        assert_eq!(line(section(&every, "R2"), "A").does, "a screenshot");
        assert!(
            !section(&every, "L2").lines.iter().any(|line| line.does == "a screenshot"),
            "the screenshot is still where it was"
        );
    }

    /// A job left with no button at all is not a line. This is a guide to what
    /// pressing something comes to.
    #[test]
    fn a_job_with_no_button_is_not_something_to_press() {
        let said = Jobs::read("[jobs]\nmenu = \"\"\n").expect("a table");
        let every = sections(&Table::of(&said), "");
        assert!(!section(&every, DOABLE).lines.iter().any(|line| line.does == "the menu"));
    }

    /// Two buttons doing one job is one line naming both.
    #[test]
    fn two_buttons_that_do_one_thing_are_one_line() {
        let every = sections(&ours(), "");
        assert_eq!(line(section(&every, DOABLE), "X / Keyboard").does, "show or hide the keyboard");
    }

    /// The table is the answer to what a row does, so a button given something
    /// new to do becomes something new to press here without anybody saying so
    /// twice.
    #[test]
    fn what_a_button_runs_comes_from_the_table_that_runs_it() {
        assert_eq!(runs_for(What::PutAway), Some(vec!["put-away".to_string()]));
        assert_eq!(runs_for(What::GameMode), Some(vec!["game-mode".to_string()]));
        // A key is not something a row can do for you.
        assert_eq!(runs_for(What::Back), None);
    }

    /// A button said the way somebody would say it out loud.
    #[test]
    fn a_button_is_said_the_way_it_is_spoken() {
        assert_eq!(said("dpad-up"), "D-pad up");
        assert_eq!(said("right-paddle-bottom"), "Right paddle bottom");
        assert_eq!(said("l1"), "L1");
        assert_eq!(said("legion-right"), "Legion right");
    }

    #[test]
    fn the_guide_holds_together_with_nothing_read_off_the_machine() {
        let sections = sections(&ours(), "");
        assert_eq!(sections[0].title, DOABLE);
        assert!(!sections[0].lines.is_empty(), "the parts nothing has to be read for");
        assert!(sections.last().expect("a section").lines.is_empty(), "no keyboard, no binds");
    }

    /// A guide is read on a device with no keyboard plugged into it, so the
    /// keys it names are keys nobody can press. Every one of them is doable
    /// from the guide instead.
    #[test]
    fn every_typed_bind_carries_a_way_of_asking_for_it() {
        let lua = "
hl.bind(mod .. \"R\", hl.dsp.exec_cmd(\"/usr/local/bin/launcher\"))
hl.bind(mod .. \"W\", hl.dsp.window.close())
";
        let typed = &sections(&ours(), lua).last().expect("a section").lines.clone();
        assert_eq!(typed[0].runs, Some(vec!["/usr/local/bin/launcher".to_string()]));
        assert!(typed.iter().all(|line| line.runs.is_some()), "a key nobody can press and nothing can ask for");
    }

    /// A guide is read by somebody who does not know the answer, and a button
    /// answered twice in one breath is worse than a button not answered at
    /// all: they have to work out which of the two they are looking at.
    #[test]
    fn nothing_is_answered_twice_in_one_section() {
        for section in sections(&ours(), "") {
            let mut said: Vec<&str> = section.lines.iter().map(|line| line.button.as_str()).collect();
            said.sort_unstable();
            let mut once = said.clone();
            once.dedup();
            assert_eq!(said, once, "{}: a button is answered twice", section.title);
        }
    }

    /// The files name what can be done with a thing, and the guide says it in
    /// one line. Read off the same list the panel offers, so a deed the files
    /// learn is a deed the guide names.
    #[test]
    fn the_guide_names_every_deed_the_files_offer() {
        let sections = sections(&ours(), "");
        let files = section(&sections, "Files");
        let said = &line(files, "Y").does;
        for deed in doing::EVERY {
            assert!(said.contains(Deed::says(deed)), "the guide does not name {}", Deed::says(deed));
        }
    }

    #[test]
    fn every_section_is_named() {
        for section in sections(&ours(), "") {
            assert!(!section.title.is_empty());
        }
    }
}
