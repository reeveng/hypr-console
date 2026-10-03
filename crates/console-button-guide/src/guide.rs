//! Every part of the guide, in the order it is learned.
//!
//! One list, read twice: printed as headings in a terminal, and drawn as tabs
//! on the device. A section that exists in one and not the other is how a guide
//! starts lying. On the device the tabs are the Buttons panel's, after its own
//! two: what a button does is a row there that can be changed, so the sections
//! that say it are the terminal's alone, and [`reference`] is what both read --
//! what a menu, the Home Screen or the keyboard does with a button, which no
//! one moves.
//!
//! What a button does is read off the one table that decides it, grouped by
//! what is held with it: a section for a press on its own, and one for each
//! set of things held. Nothing here is written by hand about a button, which is
//! the whole point -- a job someone has moved is a job this guide names on the
//! button they moved it to, and a chord no one has put anything on is a heading
//! that never appears.
//!
//! The keyboard is a section of the same table rather than a reading of
//! someone else's file. It used to be `binds`, which found `hl.bind` lines in
//! `hyprland.lua` and guessed at what each one meant from the dispatcher it
//! called -- a parser for a language this desktop does not own, kept honest by
//! nothing. Those binds are rows in the table now, so the section is built the
//! way every other section is, and the parser is gone.
//!
//! Every section is here whichever hand is on the machine, and which one is
//! open first is the whole of what the input decides -- [`opens_on`], read
//! against the last press. Filtering was the other way to do it and is wrong:
//! someone at a keyboard asking what the pad does is the most common reason to
//! open this at all, and a guide that had hidden the answer would be a guide
//! that knew it and would not say.
//!
//! The hand is half the question and it used to be the whole of it. A guide
//! raised over a menu opened on Anywhere, where A is a click and R1 is a
//! workspace, and both of those are false of the screen it was raised over:
//! the true answers were one tab along, under Menus, and a person who has to
//! find the right tab before the first line is true has been handed a manual.
//! So what is in front is asked as well, and it is asked of the compositor,
//! the way the daemon asks it -- a [`Mode`], which is the same word for what
//! is on the screen that every press is already decided against. The hand
//! answers only where the front has nothing to say, which is the desktop
//! itself.


use std::collections::BTreeSet;

use console_input_controller::actions::{Task, Table, Context};
use console_input_controller::mode::Mode;
use console_files::doing::{self, FileAction};
use console_input_bindings::bound::{Binding, Input, Played};
use console_core_never::Never;

pub const DOABLE: &str = "General";

pub const MENUS: &str = "Menus";

pub const KEYBOARD: &str = "On-Screen Keyboard";

pub const HOME_SCREEN: &str = "Home Screen";

pub const TYPED: &str = "Keyboard shortcuts";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub button: String,
    pub does: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    pub lines: Vec<Line>,
}

impl Section {
    fn of(title: &str, lines: Vec<Line>) -> Result<Self, Never> {
        Ok(Section { title: title.to_string(), lines })
    }
}

pub fn button_label(button: &str) -> Result<String, Never> {
    let said = match button.strip_prefix("dpad-") {
        Some(way) => format!("d-pad {way}"),
        None => button.replace('-', " "),
    };
    let mut letters = said.chars();

    Ok(match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    })
}

fn lines(
    table: &Table,
    on: Input,
    held: &[String],
    wanted: impl Fn(&Task) -> bool,
) -> Result<Vec<Line>, Never> {
    let Ok(every) = table.every();

    Ok(every
        .filter(|(job, _)| wanted(job))
        .filter_map(|(job, bound)| {
            let Ok(line) = line(job, bound, on, held);

            line
        })
        .collect())
}

fn line(job: &Task, bound: &[Binding], on: Input, held: &[String]) -> Result<Option<Line>, Never> {
    let pressed: Vec<String> = bound
        .iter()
        .filter(|one| {
            let Ok(played) = one.played();

            played == Played::ByAPress && one.on == on && one.held == held
        })
        .map(|one| {
            let Ok(said) = button_label(&one.pressed);

            said
        })
        .collect();

    let Ok(says) = job.action.says();

    Ok(match pressed.is_empty() {
        true => None,
        false => Some(Line { button: pressed.join(" / "), does: says.to_string() }),
    })
}

fn chords(table: &Table, on: Input) -> Result<Vec<Vec<String>>, Never> {
    let Ok(every) = table.every();
    let mut held: BTreeSet<Vec<String>> = BTreeSet::new();

    for (_, bound) in every {
        'over_binds: for one in bound.iter().filter(|one| one.on == on) {
            let Ok(played) = one.played();

            match played {
                Played::ByNothing => continue 'over_binds,
                Played::ByAPress => {},
            }

            match one.held.is_empty() {
                true => continue 'over_binds,
                false => {
                    let _ = held.insert(one.held.clone());
                },
            }
        }
    }

    let mut found: Vec<Vec<String>> = held.into_iter().collect();

    found.sort_by_key(|held| (held.len(), held.join(" + ")));

    Ok(found)
}

fn chord_title(held: &[String]) -> Result<String, Never> {
    let mut words: Vec<String> = Vec::new();

    for word in held {
        let Ok(said) = button_label(word);

        words.push(said);
    }

    Ok(words.join(" + "))
}

fn keyboard_lines(table: &Table) -> Result<Vec<Line>, Never> {
    let Ok(chords) = chords(table, Input::Keyboard);
    let mut every: Vec<Line> = Vec::new();

    for held in std::iter::once(Vec::new()).chain(chords) {
        let Ok(mut lines) = lines(table, Input::Keyboard, &held, |_| true);

        match held.is_empty() {
            true => {},
            false => {
                let Ok(said) = chord_title(&held);

                for line in &mut lines {
                    line.button = format!("{said} + {}", line.button);
                }
            }
        }

        every.extend(lines);
    }

    Ok(every)
}

fn what_can_be_done() -> Result<String, Never> {
    let mut said: Vec<&str> = Vec::new();

    for deed in doing::EVERY {
        let says = FileAction::says(deed)?;

        said.push(says);
    }

    Ok(said.join(", "))
}

pub fn opens_on(on: Input, front: Mode) -> Result<&'static str, Never> {
    let Ok(in_front) = in_front(front);

    Ok(match (in_front, on) {
        (Some(title), _) => title,
        (None, Input::Pad) => DOABLE,
        (None, Input::Keyboard) => TYPED,
    })
}

pub fn in_front(front: Mode) -> Result<Option<&'static str>, Never> {
    Ok(match front {
        Mode::Tabs | Mode::App => Some(MENUS),
        Mode::Keyboard => Some(KEYBOARD),
        Mode::HomeScreen | Mode::Standing => Some(HOME_SCREEN),
        Mode::Desktop | Mode::Prompt => None,
    })
}

fn on_the_pad(
    table: &Table,
    wanted: impl Fn(&Task) -> bool,
    rest: &[(&str, &str)],
) -> Result<Vec<Line>, Never> {
    let Ok(mut bound) = lines(table, Input::Pad, &[], wanted);
    let Ok(rest) = lines_from(rest);

    bound.extend(rest);

    Ok(bound)
}

fn around(table: &Table) -> Result<Vec<Line>, Never> {
    on_the_pad(
        table,
        |job| {
            !matches!(
                job.context,
                Context::WithAPanelOrApp | Context::OnTheHomeScreen | Context::StandingOnASquare
            )
        },
        &[
            ("Volume rocker", "volume up, volume down, mute"),
            ("Touchpad", "move the pointer"),
            ("Tap the touchpad", "click"),
            ("Press the touchpad in", "hold to drag"),
            ("The screen", "tap to select, swipe to scroll"),
            ("The bar", "tap its icons"),
        ],
    )
}

fn menus(table: &Table) -> Result<Vec<Line>, Never> {
    on_the_pad(
        table,
        |job| job.context == Context::WithAPanelOrApp,
        &[
            ("D-pad", "move the selection"),
            ("Y, in the menu", "add to or remove from the Home Screen"),
            ("B", "go back"),
            ("X", "show or hide the keyboard"),
            ("Typing", "search, where a menu has it"),
            ("D-pad left / right", "change the value"),
            ("Right paddle, top", "close the menu"),
            ("Legion right", "open Settings"),
            ("Menu", "show the button guide"),
            ("Tap a row", "select"),
            ("\u{2039} and \u{203a}", "previous or next tab"),
            ("\u{2212} and +", "change the value"),
            ("\u{d7}", "close"),
            ("Its bar icon", "tap again to close"),
        ],
    )
}

fn home(table: &Table) -> Result<Vec<Line>, Never> {
    on_the_pad(
        table,
        |job| matches!(job.context, Context::OnTheHomeScreen | Context::StandingOnASquare),
        &[
            ("D-pad, first press", "show the selection"),
            ("D-pad off the side", "previous or next page"),
            ("Move", "under Y; move with the D-pad, A to drop"),
            ("Remove from Home Screen", "under Y, on the app"),
            ("An empty spot", "open the menu to add an app"),
            ("Tap an app", "open it"),
            ("Touch and hold an app", "move it"),
            ("Swipe sideways", "previous or next page"),
            ("Swipe up", "open the menu"),
        ],
    )
}

fn files() -> Result<Vec<Line>, Never> {
    let Ok(what_can_be_done) = what_can_be_done();

    lines_from(&[
        ("L1 / R1", "Home or an external drive"),
        ("A", "open"),
        ("B", "enclosing folder"),
        ("Y", &what_can_be_done),
        ("New Folder", "under Y, in this folder"),
        ("Copy or Move", "choose it, then paste"),
        ("Delete", "asks first; moves to the Trash"),
        ("Top row", "tap to go to the enclosing folder"),
    ])
}

pub fn sections(table: &Table) -> Result<Vec<Section>, Never> {
    let Ok(around) = around(table);
    let Ok(anywhere) = Section::of(DOABLE, around);
    let mut every = vec![anywhere];

    let Ok(chords) = chords(table, Input::Pad);

    for held in chords {
        let Ok(under) = lines(table, Input::Pad, &held, |_| true);
        let Ok(title) = chord_title(&held);
        let Ok(section) = Section::of(&title, under);

        every.push(section);
    }

    let Ok(reference) = reference(table);
    let Ok(shortcuts) = keyboard_lines(table);
    let Ok(typed) = Section::of(TYPED, shortcuts);

    every.extend(reference);
    every.push(typed);

    Ok(every)
}

pub fn reference(table: &Table) -> Result<Vec<Section>, Never> {
    let Ok(menus) = menus(table);
    let Ok(home) = home(table);
    let Ok(keyboard) = lines_from(&[
        ("X", "hide the keyboard"),
        ("A", "press the selected key"),
        ("B", "backspace"),
        ("Y", "shift"),
        ("D-pad", "move between keys"),
        ("L1 / R1", "previous / next keyboard page"),
        ("Menu", "enter"),
        ("Stick press", "press the selected key"),
    ]);
    let Ok(files) = files();
    let Ok(music) = lines_from(&[
        ("A", "play a song or folder"),
        ("Y", "show in Files to rename or delete"),
        ("Typing", "search songs, artists or albums"),
        ("D-pad left / right", "previous or next song"),
        ("Shuffle", "under the song playing"),
        ("Repeat One", "under the song playing"),
    ]);
    let Ok(browser) = lines_from(&[
        ("Y", "show link hints"),
        ("D-pad", "move between links"),
        ("A", "open the selected link"),
        ("B", "hide link hints, then go back"),
        ("Y again", "link hints that open in a new tab"),
        ("Along the bottom", "search, tabs, new tab, close tab"),
        ("A new tab", "opens with the address bar selected"),
        ("X", "show the keyboard"),
    ]);
    let Ok(steam) = lines_from(&[
        ("Legion left", "open the Steam menu"),
        ("Legion left, held", "return to the desktop"),
        ("Everything else", "passed straight to the game"),
    ]);

    let mut every = Vec::new();

    for (title, lines) in [
        (KEYBOARD, keyboard),
        (MENUS, menus),
        (HOME_SCREEN, home),
        ("Files", files),
        ("Music", music),
        ("Browser", browser),
        ("Steam", steam),
    ] {
        let Ok(section) = Section::of(title, lines);

        every.push(section);
    }

    Ok(every)
}

fn lines_from(said: &[(&str, &str)]) -> Result<Vec<Line>, Never> {
    Ok(said
        .iter()
        .map(|(button, does)| Line {
            button: button.to_string(),
            does: does.to_string(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;
    use console_input_bindings::moved::Tasks;

    fn our_sections() -> Result<Vec<Section>, Never> {
        let Ok(table) = Table::ours();

        super::sections(&table)
    }

    fn sections_read(said: &str) -> Result<Vec<Section>, Box<dyn Error>> {
        let tasks = Tasks::read(said)?;
        let Ok(table) = Table::of(&tasks);
        let Ok(sections) = super::sections(&table);

        Ok(sections)
    }

    fn section<'a>(every: &'a [Section], title: &str) -> Result<Option<&'a Section>, Never> {
        Ok(every.iter().find(|section| section.title == title))
    }

    fn does<'a>(every: &'a [Section], (title, button): (&str, &str)) -> Result<Option<&'a str>, Never> {
        let Ok(section) = section(every, title);

        Ok(section
            .and_then(|section| section.lines.iter().find(|line| line.button == button))
            .map(|line| line.does.as_str()))
    }

    #[test]
    fn what_a_trigger_held_makes_of_a_button_comes_from_the_table() {
        let Ok(every) = our_sections();

        assert_eq!(does(&every, ("L2", "D-pad up")), Ok(Some("volume up")));
        assert_eq!(does(&every, ("L2", "Right paddle bottom")), Ok(Some("take a screenshot")));
    }

    #[test]
    fn the_guide_opens_on_the_hand_that_was_last_used_and_hides_neither() {
        let Ok(every) = our_sections();

        assert_eq!(opens_on(Input::Pad, Mode::Desktop), Ok(DOABLE));
        assert_eq!(opens_on(Input::Keyboard, Mode::Desktop), Ok(TYPED));

        for title in [DOABLE, TYPED] {
            assert!(
                every.iter().any(|section| section.title == title && !section.lines.is_empty()),
                "{title} is where the guide can open, so it has to be a page with something on it"
            );
        }
    }

    #[test]
    fn what_is_in_front_answers_before_the_hand_does() {
        for on in [Input::Pad, Input::Keyboard] {
            assert_eq!(
                opens_on(on, Mode::Tabs),
                Ok(MENUS),
                "a guide raised over a menu opened on a tab where A is a click"
            );
            assert_eq!(opens_on(on, Mode::Keyboard), Ok(KEYBOARD));
            assert_eq!(opens_on(on, Mode::HomeScreen), Ok(HOME_SCREEN));
            assert_eq!(opens_on(on, Mode::Standing), Ok(HOME_SCREEN));
        }
    }

    #[test]
    fn every_tab_the_front_can_open_on_is_a_page_with_something_on_it() {
        let Ok(every) = our_sections();

        for title in [MENUS, KEYBOARD, HOME_SCREEN] {
            assert!(
                every.iter().any(|section| section.title == title && !section.lines.is_empty()),
                "{title} is where the guide can open, and an empty page is not drawn at all"
            );
        }
    }

    #[test]
    fn a_chord_no_one_has_put_anything_on_is_not_a_heading() {
        let Ok(every) = our_sections();

        assert!(!every.iter().any(|section| section.title == "R2"));
    }

    #[test]
    fn l2_with_r2_is_where_a_window_is_carried() {
        let Ok(every) = our_sections();

        assert_eq!(does(&every, ("L2 + R2", "R1")), Ok(Some("move the window to the next desktop")));
        assert_eq!(does(&every, ("L2", "R1")), Ok(Some("next desktop")));
    }

    #[test]
    fn a_job_someone_moved_is_named_where_they_moved_it() -> Result<(), Box<dyn Error>> {
        let every = sections_read("[jobs]\nscreenshot = \"r2 + a\"\n")?;
        let Ok(held) = section(&every, "L2");
        let held = held.ok_or("a section for L2")?;

        assert_eq!(does(&every, ("R2", "A")), Ok(Some("take a screenshot")));
        assert!(
            !held.lines.iter().any(|line| line.does == "take a screenshot"),
            "the screenshot is still where it was"
        );

        Ok(())
    }

    #[test]
    fn a_chord_of_two_buttons_is_a_heading_of_its_own() -> Result<(), Box<dyn Error>> {
        let every = sections_read("[jobs]\nmenu = \"left-paddle-bottom + right-paddle-top\"\n")?;

        assert_eq!(does(&every, ("Left paddle bottom", "Right paddle top")), Ok(Some("open the menu")));

        Ok(())
    }

    #[test]
    fn a_job_with_no_button_is_not_something_to_press() -> Result<(), Box<dyn Error>> {
        let every = sections_read("[jobs]\nmenu = \"\"\n")?;
        let Ok(doable) = section(&every, DOABLE);
        let doable = doable.ok_or("a section of what can be done")?;

        assert!(!doable.lines.iter().any(|line| line.does == "open the menu"));

        Ok(())
    }

    #[test]
    fn two_buttons_that_do_one_thing_are_one_line() {
        let Ok(every) = our_sections();

        assert_eq!(does(&every, (DOABLE, "X / Keyboard")), Ok(Some("show or hide the keyboard")));
    }

    #[test]
    fn a_button_is_said_the_way_it_is_spoken() {
        assert_eq!(button_label("dpad-up"), Ok(String::from("D-pad up")));
        assert_eq!(button_label("right-paddle-bottom"), Ok(String::from("Right paddle bottom")));
        assert_eq!(button_label("l1"), Ok(String::from("L1")));
        assert_eq!(button_label("legion-right"), Ok(String::from("Legion right")));
    }

    #[test]
    fn the_guide_holds_together_with_nothing_read_off_the_machine() {
        let Ok(sections) = our_sections();

        assert_eq!(sections.first().map(|section| section.title.as_str()), Some(DOABLE));
        assert!(
            sections.first().is_some_and(|section| !section.lines.is_empty()),
            "the parts nothing has to be read for"
        );
    }

    #[test]
    fn what_a_keyboard_reaches_is_a_section_read_off_the_same_table() {
        let Ok(every) = our_sections();

        assert_eq!(does(&every, (TYPED, "Super + I")), Ok(Some("open Settings")));
        assert_eq!(does(&every, (TYPED, "Super + Shift + F")), Ok(Some("full screen on or off")));
        assert_eq!(does(&every, (TYPED, "Print")), Ok(Some("take a screenshot")));
    }

    #[test]
    fn nothing_is_answered_twice_in_one_section() {
        let Ok(every) = our_sections();

        for section in every {
            let mut said: Vec<&str> = section.lines.iter().map(|line| line.button.as_str()).collect();
            said.sort_unstable();
            let mut once = said.clone();
            once.dedup();
            assert_eq!(said, once, "{}: a button is answered twice", section.title);
        }
    }

    #[test]
    fn the_guide_names_every_deed_the_files_offer() -> Result<(), &'static str> {
        let Ok(every) = our_sections();
        let Ok(said) = does(&every, ("Files", "Y"));
        let said = said.ok_or("the files say what Y does")?;

        for deed in doing::EVERY {
            let Ok(says) = FileAction::says(deed);

            assert!(said.contains(says), "the guide does not name {says}");
        }

        Ok(())
    }

    #[test]
    fn every_section_is_named() {
        let Ok(every) = our_sections();

        for section in every {
            assert!(!section.title.is_empty());
        }
    }
}
