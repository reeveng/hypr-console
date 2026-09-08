//! Every part of the guide, in the order it is learned.
//!
//! One list, read twice: printed as headings in a terminal, and drawn as tabs
//! on the device. A section that exists in one and not the other is how a guide
//! starts lying.
//!
//! What a button does is read off the one table that decides it, grouped by
//! what is held with it: a section for a press on its own, and one for each
//! set of things held. Nothing here is written by hand about a button, which is
//! the whole point -- a job somebody has moved is a job this guide names on the
//! button they moved it to, and a chord nobody has put anything on is a heading
//! that never appears.
//!
//! The keyboard is a section of the same table rather than a reading of
//! somebody else's file. It used to be `binds`, which found `hl.bind` lines in
//! `hyprland.lua` and guessed at what each one meant from the dispatcher it
//! called -- a parser for a language this desktop does not own, kept honest by
//! nothing. Those binds are rows in the table now, so the section is built the
//! way every other section is, and the parser is gone.
//!
//! Every section is here whichever hand is on the machine, and which one is
//! open first is the whole of what the input decides -- [`opens_on`], read
//! against the last press. Filtering was the other way to do it and is wrong:
//! somebody at a keyboard asking what the pad does is the commonest reason to
//! open this at all, and a guide that had hidden the answer would be a guide
//! that knew it and would not say.

use console_input_controller::doing::Doing;
use console_input_controller::means::{Job, Press, Table, What, When};
use console_files::doing::{self, Deed};
use console_input_bindings::bound::{Binding, Input, Played};
use console_core_never::Never;

pub const DOABLE: &str = "Anywhere";

pub const MENUS: &str = "Menus";

pub const TYPED: &str = "Keyboard shortcuts";

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

fn lines(
    table: &Table,
    on: Input,
    held: &[String],
    wanted: impl Fn(&Job) -> bool,
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

fn line(job: &Job, bound: &[Binding], on: Input, held: &[String]) -> Result<Option<Line>, Never> {
    let pressed: Vec<String> = bound
        .iter()
        .filter(|one| {
            let Ok(played) = one.played();

            played == Played::ByAPress && one.on == on && one.held == held
        })
        .map(|one| {
            let Ok(said) = said(&one.pressed);

            said
        })
        .collect();

    let Ok(runs) = runs_for(job.what);
    let Ok(says) = job.what.says();

    Ok(match pressed.is_empty() {
        true => None,
        false => Some(Line { button: pressed.join(" / "), does: says.to_string(), runs }),
    })
}

fn chords(table: &Table, on: Input) -> Result<Vec<Vec<String>>, Never> {
    let Ok(every) = table.every();
    let mut found: Vec<Vec<String>> = Vec::new();

    for (_, bound) in every {
        for one in bound.iter().filter(|one| one.on == on) {
            let Ok(played) = one.played();

            match played {
                Played::ByNothing => continue,
                Played::ByAPress => {},
            }

            match one.held.is_empty() || found.contains(&one.held) {
                true => continue,
                false => found.push(one.held.clone()),
            }
        }
    }

    found.sort_by_key(|held| (held.len(), held.join(" + ")));

    Ok(found)
}

fn titled(held: &[String]) -> Result<String, Never> {
    let mut words: Vec<String> = Vec::new();

    for word in held {
        let Ok(said) = said(word);

        words.push(said);
    }

    Ok(words.join(" + "))
}

fn typed(table: &Table) -> Result<Vec<Line>, Never> {
    let Ok(chords) = chords(table, Input::Keyboard);
    let mut every: Vec<Line> = Vec::new();

    for held in std::iter::once(Vec::new()).chain(chords) {
        let Ok(mut lines) = lines(table, Input::Keyboard, &held, |_| true);

        match held.is_empty() {
            true => {},
            false => {
                let Ok(said) = titled(&held);

                for line in &mut lines {
                    line.button = format!("{said} + {}", line.button);
                }
            }
        }

        every.extend(lines);
    }

    Ok(every)
}

pub fn runs_for(what: What) -> Result<Option<Vec<String>>, Never> {
    let Ok(does) = what.does(Press::Down);

    let doing = match does {
        Some(doing) => doing,
        None => return Ok(None),
    };

    Ok(match doing {
        Doing::Run(argv) => Some(argv),
        Doing::Frame(_) | Doing::Tell(_) | Doing::Using(_) => None,
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

pub fn opens_on(on: Input) -> Result<&'static str, Never> {
    Ok(match on {
        Input::Pad => DOABLE,
        Input::Keyboard => TYPED,
    })
}

pub fn sections(table: &Table) -> Result<Vec<Section>, Never> {
    let Ok(mut around) = lines(table, Input::Pad, &[], |job| {
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

    let Ok(mut menus) = lines(table, Input::Pad, &[], |job| job.when == When::WithAChooserUp);
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

    let Ok(mut home) = lines(table, Input::Pad, &[], |job| {
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
    let Ok(shortcuts) = typed(table);

    let Ok(anywhere) = Section::of(DOABLE, around);
    let mut every = vec![anywhere];

    let Ok(chords) = chords(table, Input::Pad);

    for held in chords {
        let Ok(under) = lines(table, Input::Pad, &held, |_| true);
        let Ok(title) = titled(&held);
        let Ok(section) = Section::of(&title, under);

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
        (TYPED, shortcuts),
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
    use console_input_bindings::moved::Jobs;

    fn ours() -> Table {
        let Ok(table) = Table::ours();

        table
    }

    fn moved(said: &Jobs) -> Table {
        let Ok(table) = Table::of(said);

        table
    }

    fn sections(table: &Table) -> Vec<Section> {
        let Ok(sections) = super::sections(table);

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
        let every = sections(&ours());
        let held = section(&every, "L2");
        assert_eq!(line(held, "D-pad up").does, "louder");
        assert_eq!(line(held, "Right paddle bottom").does, "a screenshot");
    }

    #[test]
    fn the_guide_opens_on_the_hand_that_was_last_used_and_hides_neither() {
        let every = sections(&ours());

        assert_eq!(opens_on(Input::Pad), Ok(DOABLE));
        assert_eq!(opens_on(Input::Keyboard), Ok(TYPED));

        for title in [DOABLE, TYPED] {
            assert!(
                every.iter().any(|section| section.title == title && !section.lines.is_empty()),
                "{title} is where the guide can open, so it has to be a page with something on it"
            );
        }
    }

    #[test]
    fn a_chord_nobody_has_put_anything_on_is_not_a_heading() {
        let every = sections(&ours());

        assert!(!every.iter().any(|section| section.title == "R2"));
        assert!(!every.iter().any(|section| section.title == "L2 + R2"));
    }

    #[test]
    fn a_job_somebody_moved_is_named_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let every = sections(&moved(&said));

        assert_eq!(line(section(&every, "R2"), "A").does, "a screenshot");
        assert!(
            !section(&every, "L2").lines.iter().any(|line| line.does == "a screenshot"),
            "the screenshot is still where it was"
        );
    }

    #[test]
    fn a_chord_of_two_buttons_is_a_heading_of_its_own() {
        let said = Jobs::read("[jobs]\nmenu = \"left-paddle-bottom + right-paddle-top\"\n")
            .expect("a table");
        let every = sections(&moved(&said));

        assert_eq!(line(section(&every, "Left paddle bottom"), "Right paddle top").does, "the menu");
    }

    #[test]
    fn a_job_with_no_button_is_not_something_to_press() {
        let said = Jobs::read("[jobs]\nmenu = \"\"\n").expect("a table");
        let every = sections(&moved(&said));
        assert!(!section(&every, DOABLE).lines.iter().any(|line| line.does == "the menu"));
    }

    #[test]
    fn two_buttons_that_do_one_thing_are_one_line() {
        let every = sections(&ours());
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
        let sections = sections(&ours());

        assert_eq!(sections.first().map(|section| section.title.as_str()), Some(DOABLE));
        assert!(
            sections.first().is_some_and(|section| !section.lines.is_empty()),
            "the parts nothing has to be read for"
        );
    }

    #[test]
    fn what_a_keyboard_reaches_is_a_section_read_off_the_same_table() {
        let every = sections(&ours());
        let typed = section(&every, TYPED);

        assert_eq!(line(typed, "Super + I").does, "the settings");
        assert_eq!(line(typed, "Super + Shift + F").does, "fill the screen with this window");
        assert_eq!(line(typed, "Print").does, "a screenshot");
        assert!(
            typed.lines.iter().all(|line| line.runs.is_some()),
            "a key nobody can press and nothing can ask for"
        );
    }

    #[test]
    fn nothing_is_answered_twice_in_one_section() {
        for section in sections(&ours()) {
            let mut said: Vec<&str> = section.lines.iter().map(|line| line.button.as_str()).collect();
            said.sort_unstable();
            let mut once = said.clone();
            once.dedup();
            assert_eq!(said, once, "{}: a button is answered twice", section.title);
        }
    }

    #[test]
    fn the_guide_names_every_deed_the_files_offer() {
        let sections = sections(&ours());
        let files = section(&sections, "Files");
        let said = &line(files, "Y").does;
        for deed in doing::EVERY {
            let Ok(says) = Deed::says(deed);

            assert!(said.contains(says), "the guide does not name {says}");
        }
    }

    #[test]
    fn every_section_is_named() {
        for section in sections(&ours()) {
            assert!(!section.title.is_empty());
        }
    }
}
