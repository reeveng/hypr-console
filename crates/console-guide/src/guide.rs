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
use console_controller::means::{Job, Press, Table, What, When};
use console_files::doing::{self, Deed};
use console_gamepad::jobs::{ALONE, Binding, Held, Layer, Played};
use console_never::Never;

use crate::binds::binds;

pub const DOABLE: &str = "Anywhere";

pub const MENUS: &str = "Menus";

const HELD: [(Result<Layer, Never>, &str); 3] = [
    (Layer::of(Held::Down, Held::Up), "L2"),
    (Layer::of(Held::Up, Held::Down), "R2"),
    (Layer::of(Held::Down, Held::Down), "L2 + R2"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub button: String,
    pub does: String,
    pub runs: Option<Vec<String>>,
}

impl Line {
    pub fn new(button: &str, does: &str) -> Result<Self, Never> {
        Ok(Line { button: button.to_string(), does: does.to_string(), runs: None })
    }
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

pub fn said(button: &str) -> Result<String, Never> {
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

fn lines(table: &Table, layer: Layer, wanted: impl Fn(&Job) -> bool) -> Result<Vec<Line>, Never> {
    let Ok(every) = table.every();

    Ok(every
        .filter(|(job, _)| wanted(job))
        .filter_map(|(job, bound)| {
            let Ok(line) = line(job, bound, layer);

            line
        })
        .collect())
}

fn line(job: &Job, bound: &[Binding], layer: Layer) -> Result<Option<Line>, Never> {
    let on: Vec<String> = bound
        .iter()
        .filter(|one| {
            let Ok(played) = one.played();

            played == Played::ByAButton && one.layer == layer
        })
        .map(|one| {
            let Ok(said) = said(&one.button);

            said
        })
        .collect();

    let Ok(runs) = runs_for(job.what);
    let Ok(says) = job.what.says();

    Ok(match on.is_empty() {
        true => None,
        false => Some(Line { button: on.join(" / "), does: says.to_string(), runs }),
    })
}

pub fn runs_for(what: What) -> Result<Option<Vec<String>>, Never> {
    let Ok(does) = what.does(Press::Down);

    let Some(doing) = does else { return Ok(None) };

    Ok(match doing {
        Doing::Run(argv) => Some(argv),
        Doing::Frame(_) | Doing::Tell(_) => None,
    })
}

fn what_can_be_done() -> Result<String, Never> {
    let mut said: Vec<&str> = Vec::new();

    for deed in doing::EVERY {
        let says = Deed::says(deed)?;

        said.push(says);
    }

    Ok(said.join(", "))
}

pub fn sections(table: &Table, lua: &str) -> Result<Vec<Section>, Never> {
    let Ok(mut around) = lines(table, ALONE, |job| {
        !matches!(
            job.when,
            When::WithAChooserUp | When::OnTheHomeScreen | When::StandingOnASquare
        )
    });
    let Ok(rest) = written(&[
        ("Volume rocker", "louder, quieter, unmute"),
        ("Touchpad", "move the pointer"),
        ("Tap the touchpad", "click"),
        ("Press the touchpad in", "click and hold to drag"),
        ("The screen", "tap to click, drag to scroll"),
        ("The bar", "tap its icons"),
    ]);

    around.extend(rest);

    let Ok(mut menus) = lines(table, ALONE, |job| job.when == When::WithAChooserUp);
    let Ok(rest) = written(&[
        ("D-pad", "move the highlight"),
        ("Y, in the menu", "put an app on the home screen, or take it off"),
        ("B", "back out"),
        ("X", "show or hide the keyboard"),
        ("Typing", "the top row of a menu that has one"),
        ("D-pad left / right", "move a level"),
        ("Right paddle, top", "close the menu"),
        ("Legion right", "the settings"),
        ("Menu", "this guide"),
        ("Tap a row", "the same as A"),
        ("\u{2039} and \u{203a}", "the tab before or after"),
        ("\u{2212} and +", "move a level with a finger"),
        ("\u{d7}", "close, the same as B"),
        ("Its bar icon", "tap it again to close"),
    ]);

    menus.extend(rest);

    let Ok(mut home) = lines(table, ALONE, |job| {
        matches!(job.when, When::OnTheHomeScreen | When::StandingOnASquare)
    });
    let Ok(rest) = written(&[
        ("D-pad, first press", "show where you are standing"),
        ("D-pad off the side", "the pane before or after"),
        ("Move it", "under Y; the d-pad carries it, A puts it down"),
        ("Take it off", "under Y, on the square it is on"),
        ("An empty square", "the menu, to put one there"),
        ("Tap an app", "the same as A"),
        ("Hold a finger on one", "pick it up"),
        ("Swipe sideways", "the pane before or after"),
        ("Swipe up", "the menu"),
    ]);

    home.extend(rest);

    let Ok(what_can_be_done) = what_can_be_done();
    let Ok(keyboard) = written(&[
        ("X", "put the keyboard away"),
        ("A", "press the key you are on"),
        ("B", "backspace"),
        ("Y", "shift"),
        ("D-pad", "move between keys"),
        ("L1 / R1", "previous / next set of keys"),
        ("Menu", "enter"),
        ("Stick press", "press the key you are on"),
    ]);
    let Ok(files) = written(&[
        ("L1 / R1", "Home, and whatever is plugged in"),
        ("A", "open a folder or a file"),
        ("B", "the folder above"),
        ("Y", &what_can_be_done),
        ("New folder", "under Y, in whichever folder you are in"),
        ("Copy or Move", "pick it up; a row puts it down"),
        ("Delete", "asks first; goes to the wastebasket"),
        ("Row nought", "the folder above, with a finger"),
    ]);
    let Ok(music) = written(&[
        ("A", "play a song, or a folder of them"),
        ("Y", "show it in the files, where it is renamed or thrown away"),
        ("Typing", "a song, whose it is, or anything it says"),
        ("D-pad left / right", "the song before it, the song after it"),
        ("Play them in any order", "on Playing, under what is on"),
        ("Play this one over", "on Playing, under what is on"),
    ]);
    let Ok(browser) = written(&[
        ("Y", "label everything on the page that can be pressed"),
        ("D-pad", "walk between those things, one at a time"),
        ("A", "take the one you are standing on"),
        ("B", "put the labels away, and then go back a page"),
        ("Y again", "the same labels, opening in a new tab"),
        ("Along the bottom", "look for something, the tabs, a new tab, close this one"),
        ("A new tab", "opens on the line to type a question into"),
        ("X", "the keyboard, for the line being typed into"),
    ]);
    let Ok(steam) = written(&[
        ("Legion left", "Steam's own menu, which is Steam's to draw"),
        ("Legion left, held", "back to this desktop"),
        ("Everything else", "the pad, untouched, the way a game expects it"),
    ]);
    let Ok(binds) = binds(lua);
    let shortcuts: Vec<Line> = binds
        .into_iter()
        .map(|bind| Line { button: bind.keys, does: bind.does, runs: Some(bind.runs) })
        .collect();

    let Ok(anywhere) = Section::of(DOABLE, around);
    let mut every = vec![anywhere];

    for (held, title) in HELD {
        let Ok(layer) = held;
        let Ok(under) = lines(table, layer, |_| true);
        let Ok(section) = Section::of(title, under);

        every.push(section);
    }

    for (title, lines) in [
        ("Keyboard", keyboard),
        (MENUS, menus),
        ("Home screen", home),
        ("Files", files),
        ("Music", music),
        ("Browser", browser),
        ("Steam", steam),
        ("Shortcuts", shortcuts),
    ] {
        let Ok(section) = Section::of(title, lines);

        every.push(section);
    }

    Ok(every)
}

fn written(said: &[(&str, &str)]) -> Result<Vec<Line>, Never> {
    Ok(said
        .iter()
        .map(|(button, does)| {
            let Ok(line) = Line::new(button, does);

            line
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_gamepad::jobs::Jobs;

    fn ours() -> Table {
        let Ok(table) = Table::ours();

        table
    }

    fn moved(said: &Jobs) -> Table {
        let Ok(table) = Table::of(said);

        table
    }

    fn sections(table: &Table, lua: &str) -> Vec<Section> {
        let Ok(sections) = super::sections(table, lua);

        sections
    }

    fn section<'a>(every: &'a [Section], title: &str) -> &'a Section {
        every.iter().find(|section| section.title == title).expect("a section")
    }

    fn line<'a>(section: &'a Section, button: &str) -> &'a Line {
        section.lines.iter().find(|line| line.button == button).expect("a line")
    }

    #[test]
    fn what_a_trigger_held_makes_of_a_button_comes_from_the_table() {
        let every = sections(&ours(), "");
        let held = section(&every, "L2");
        assert_eq!(line(held, "D-pad up").does, "louder");
        assert_eq!(line(held, "Right paddle bottom").does, "a screenshot");
    }

    #[test]
    fn a_layer_with_nothing_on_it_has_nothing_under_it() {
        let every = sections(&ours(), "");
        assert!(section(&every, "R2").lines.is_empty());
        assert!(section(&every, "L2 + R2").lines.is_empty());
    }

    #[test]
    fn a_job_somebody_moved_is_named_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let every = sections(&moved(&said), "");
        assert_eq!(line(section(&every, "R2"), "A").does, "a screenshot");
        assert!(
            !section(&every, "L2").lines.iter().any(|line| line.does == "a screenshot"),
            "the screenshot is still where it was"
        );
    }

    #[test]
    fn a_job_with_no_button_is_not_something_to_press() {
        let said = Jobs::read("[jobs]\nmenu = \"\"\n").expect("a table");
        let every = sections(&moved(&said), "");
        assert!(!section(&every, DOABLE).lines.iter().any(|line| line.does == "the menu"));
    }

    #[test]
    fn two_buttons_that_do_one_thing_are_one_line() {
        let every = sections(&ours(), "");
        assert_eq!(line(section(&every, DOABLE), "X / Keyboard").does, "show or hide the keyboard");
    }

    #[test]
    fn what_a_button_runs_comes_from_the_table_that_runs_it() {
        assert_eq!(runs_for(What::PutAway), Ok(Some(vec!["put-away".to_string()])));
        assert_eq!(runs_for(What::GameMode), Ok(Some(vec!["game-mode".to_string()])));
        assert_eq!(runs_for(What::Back), Ok(None));
    }

    #[test]
    fn a_button_is_said_the_way_it_is_spoken() {
        assert_eq!(said("dpad-up"), Ok("D-pad up".to_string()));
        assert_eq!(said("right-paddle-bottom"), Ok("Right paddle bottom".to_string()));
        assert_eq!(said("l1"), Ok("L1".to_string()));
        assert_eq!(said("legion-right"), Ok("Legion right".to_string()));
    }

    #[test]
    fn the_guide_holds_together_with_nothing_read_off_the_machine() {
        let sections = sections(&ours(), "");

        assert_eq!(sections[0].title, DOABLE);
        assert!(!sections[0].lines.is_empty(), "the parts nothing has to be read for");
        assert!(sections.last().expect("a section").lines.is_empty(), "no keyboard, no binds");
    }

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

    #[test]
    fn the_guide_names_every_deed_the_files_offer() {
        let sections = sections(&ours(), "");
        let files = section(&sections, "Files");
        let said = &line(files, "Y").does;
        for deed in doing::EVERY {
            let Ok(says) = Deed::says(deed);

            assert!(said.contains(says), "the guide does not name {says}");
        }
    }

    #[test]
    fn every_section_is_named() {
        for section in sections(&ours(), "") {
            assert!(!section.title.is_empty());
        }
    }
}
