//! Every part of the guide, in the order it is learned.
//!
//! One list, read twice: printed as headings in a terminal, and drawn as tabs
//! on the device. A section that exists in one and not the other is how a guide
//! starts lying.

use evdev::KeyCode;
use console_controller::buttons::{BUTTONS, KEYS};
use console_files::doing::{self, Deed};
use console_pad::profile::Profile;

use crate::binds::binds;

/// The section whose rows are things to do rather than things to know.
///
/// One word a tab, where there is one. These are read along a strip on a
/// handheld, at a glance, by somebody who stopped reading because they could
/// not remember a button: a title that has to be read twice has already cost
/// more than it saves.
pub const DOABLE: &str = "Anywhere";

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

/// Each mapping in the profile, named "Button - what it does", and what the
/// desktop does about it.
///
/// The daemon's own table is the answer to the second half, so a button that is
/// given something new to do becomes something new to press here without
/// anybody remembering to say so twice.
pub fn mapped(profile: &Profile) -> Vec<Line> {
    profile
        .mappings
        .iter()
        .map(|mapping| {
            let (button, does) = mapping.label.split_once(" - ").unwrap_or((&mapping.label, ""));
            let does = match does.trim().is_empty() {
                true => "nothing yet",
                false => does.trim(),
            };
            // The last target is the one the desktop sees: a mapping that
            // sends two things sends the pad's own button last.
            let runs = mapping.targets.last().and_then(|target| target.code()).and_then(runs_for);
            Line { button: button.trim().to_string(), does: does.to_string(), runs }
        })
        .collect()
}

/// What the desktop runs when one key or button arrives.
pub fn runs_for(code: KeyCode) -> Option<Vec<String>> {
    KEYS.iter()
        .chain(BUTTONS.iter())
        .find(|(key, _)| *key == code)
        .map(|(_, argv)| argv.iter().map(|word| (*word).to_string()).collect())
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
pub fn sections(profile: Option<&Profile>, lua: &str) -> Vec<Section> {
    let mut around = profile.map(mapped).unwrap_or_default();
    around.extend([
        Line::new("Volume rocker", "louder, quieter, unmute"),
        Line::new("Touchpad", "move the pointer"),
        Line::new("Tap the touchpad", "click"),
        Line::new("Press the touchpad in", "click and hold to drag"),
        Line::new("The screen", "tap to click, drag to scroll"),
        Line::new("The bar", "tap its icons"),
    ]);
    vec![
        Section::of(DOABLE, around),
        Section::of(
            "L2",
            vec![
                Line::new("L1 / R1", "carry the window with you"),
                Line::new("D-pad left", "screen dimmer"),
                Line::new("D-pad right", "screen brighter"),
            ],
        ),
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
        Section::of(
            "Menus",
            vec![
                Line::new("D-pad", "move the highlight"),
                Line::new("A", "choose it"),
                Line::new("B", "back out"),
                Line::new("X", "show or hide the keyboard"),
                Line::new("Y", "what else can be done with a row"),
                Line::new("Typing", "the top row of a menu that has one"),
                Line::new("L1 / R1", "the tab left or right"),
                Line::new("D-pad left / right", "move a level"),
                Line::new("Right paddle, top", "close the menu"),
                Line::new("Legion right", "the settings"),
                Line::new("Menu", "this guide"),
                Line::new("Tap a row", "the same as A"),
                Line::new("\u{2039} and \u{203a}", "the tab before or after"),
                Line::new("\u{2212} and +", "move a level with a finger"),
                Line::new("\u{d7}", "close, the same as B"),
                Line::new("Its bar icon", "tap it again to close"),
            ],
        ),
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const PROFILE: &str = "
name: Desktop
target_devices:
  - keyboard
  - mouse
  - xbox-elite
mapping:
  - name: Right paddle, top - close this window
    source_event:
      gamepad:
        button: RightPaddle1
    target_events:
      - keyboard: KeyF15
  - name: Legion left - Game Mode
    source_event:
      gamepad:
        button: LeftPaddle2
    target_events:
      - gamepad:
          button: Guide
  - name: View
    source_event:
      gamepad:
        button: Select
    target_events: []
";

    fn profile() -> Profile {
        Profile::read(Path::new("desktop.yaml"), PROFILE).expect("a profile")
    }

    #[test]
    fn a_mapping_is_read_as_the_button_and_what_it_does() {
        let lines = mapped(&profile());
        assert_eq!(lines[0].button, "Right paddle, top");
        assert_eq!(lines[0].does, "close this window");
    }

    /// The daemon's own table is the answer, so a button given something new to
    /// do becomes something new to press here without anybody saying so twice.
    #[test]
    fn what_a_button_runs_comes_from_the_daemon_that_runs_it() {
        let lines = mapped(&profile());
        assert_eq!(lines[0].runs, Some(vec!["put-away".to_string()]));
        assert_eq!(lines[1].runs, Some(vec!["game-mode".to_string()]));
    }

    /// A button in the profile that says nothing about itself is still a button
    /// somebody will press.
    #[test]
    fn a_mapping_with_nothing_after_the_dash_says_so() {
        let lines = mapped(&profile());
        assert_eq!(lines[2].button, "View");
        assert_eq!(lines[2].does, "nothing yet");
        assert_eq!(lines[2].runs, None);
    }

    #[test]
    fn the_guide_holds_together_without_a_profile_to_read() {
        let sections = sections(None, "");
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
        let typed = &sections(None, lua).last().expect("a section").lines.clone();
        assert_eq!(typed[0].runs, Some(vec!["/usr/local/bin/launcher".to_string()]));
        assert!(typed.iter().all(|line| line.runs.is_some()), "a key nobody can press and nothing can ask for");
    }

    /// A guide is read by somebody who does not know the answer, and a button
    /// answered twice in one breath is worse than a button not answered at
    /// all: they have to work out which of the two they are looking at.
    #[test]
    fn nothing_is_answered_twice_in_one_section() {
        for section in sections(Some(&profile()), "") {
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
        let sections = sections(None, "");
        let files = sections.iter().find(|section| section.title == "Files").expect("the files");
        let said = &files.lines.iter().find(|line| line.button == "Y").expect("what Y does").does;
        for deed in doing::EVERY {
            assert!(said.contains(Deed::says(deed)), "the guide does not name {}", Deed::says(deed));
        }
    }

    #[test]
    fn every_section_is_named() {
        for section in sections(Some(&profile()), "") {
            assert!(!section.title.is_empty());
        }
    }
}
