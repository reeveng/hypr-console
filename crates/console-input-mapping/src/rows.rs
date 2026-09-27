//! The setup screen, as rows: one thing the desktop does each, and what plays
//! it.
//!
//! Nothing here has a machine. What this device can send and what the table
//! says are both handed in, so the screen someone sees on a handheld that has
//! never existed can be asked for on a laptop.
//!
//! One page per input, and the same rows on each. A job is a job whichever
//! hand is reaching for it, so the page does not change what it lists when you
//! walk from the controller to the keyboard -- only what it says beside each
//! row. That is why `parts` takes an input rather than returning everything at
//! once: a row showing a button and a chord and a key all at the same time is a
//! row no one can read on a screen this size, and a person changing what a key
//! does is not thinking about the pad while they do it.

use std::collections::BTreeMap;

use console_input_controller::actions::{Action, ButtonPress, Table, ours};
use console_input_controller::effect::Effect;
use console_input_gamepad::devices::Has;
use console_input_gamepad::front::Front;
use console_input_gamepad::vocabulary::{self, Names, button_name, spoken_for};
use console_input_bindings::bound::{Binding, Input, Played};
use console_core_never::Never;
use console_panel::page::{Aside, Handler, Row};

pub const NOWHERE: &str = "Not available";

pub const UNPLAYED_ON_THE_PAD: &str = "None";

pub const UNPLAYED_ON_A_KEYBOARD: &str = "None";

pub const PUT_BACK: &str = "Reset All Buttons";

pub const PUT_BACK_ASIDE: &str = "Restores the defaults";
pub const PUT_BACK_SURE: &str = "Reset all buttons to their defaults?";
pub const PUT_BACK_YES: &str = "Reset";

pub const ADD_ON_THE_PAD: &str = "Add Button";

pub const ADD_ON_A_KEYBOARD: &str = "Add Shortcut";

pub const REMOVE: &str = "Remove";

pub const WAITING_ON_THE_PAD: &str = "Hold a button to combine, or wait to cancel";

pub const WAITING_ON_A_KEYBOARD: &str = "Hold Super, Ctrl, Shift or Alt, or wait to cancel";

pub fn tabs() -> Result<Vec<&'static str>, Never> {
    let mut tabs = Vec::new();

    for input in console_input_bindings::bound::EVERY {
        let Ok(says) = input.says();

        tabs.push(says);
    }

    Ok(tabs)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plays {
    pub binding: Binding,
    pub here: Has,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub slug: String,
    pub action: Action,
    pub does: String,
    pub on: Input,
    pub plays: Vec<Plays>,
    pub moved: bool,
}

impl Part {
    pub fn played(&self) -> Result<Played, Never> {
        let by_a_press = self.plays.iter().any(|one| {
            let Ok(played) = one.binding.played();

            played == Played::ByAPress
        });

        Ok(match by_a_press {
            true => Played::ByAPress,
            false => Played::ByNothing,
        })
    }

    pub fn here(&self) -> Result<Has, Never> {
        Ok(match self.plays.iter().any(|one| one.here == Has::Yes) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    pub fn unplayed(&self) -> Result<&'static str, Never> {
        Ok(match self.on {
            Input::Pad => UNPLAYED_ON_THE_PAD,
            Input::Keyboard => UNPLAYED_ON_A_KEYBOARD,
        })
    }

    pub fn waiting(&self) -> Result<&'static str, Never> {
        Ok(match self.on {
            Input::Pad => WAITING_ON_THE_PAD,
            Input::Keyboard => WAITING_ON_A_KEYBOARD,
        })
    }

    pub fn aside(&self) -> Result<String, Never> {
        let Ok(played) = self.played();
        let Ok(here) = self.here();
        let Ok(unplayed) = self.unplayed();

        Ok(match (played, here) {
            (Played::ByNothing, _) => unplayed.to_string(),
            (_, Has::No) => NOWHERE.to_string(),
            _ => self
                .plays
                .iter()
                .filter(|one| one.here == Has::Yes)
                .map(|one| {
                    let Ok(said) = aloud(&one.binding);

                    said
                })
                .collect::<Vec<String>>()
                .join(" or "),
        })
    }
}

pub fn aloud(binding: &Binding) -> Result<String, Never> {
    let mut words: Vec<String> = Vec::new();

    for word in &binding.held {
        let Ok(said) = button_label(word);

        words.push(said);
    }

    let Ok(pressed) = button_label(&binding.pressed);

    words.push(pressed);

    Ok(words.join(" + "))
}

pub fn parts(table: &Table, front: &Front, on: Input) -> Result<Vec<Part>, Never> {
    let Ok(every) = table.every();

    let mut parts: Vec<Part> = every
        .map(|(job, bound)| {
            let Ok(defaults) = ours(job);
            let mine: Vec<&Binding> = bound.iter().filter(|one| one.on == on).collect();
            let Ok(says) = job.action.says();
            let Ok(does) = capitalised(says);
            let moved = mine.iter().map(|one| (*one).clone()).collect::<Vec<Binding>>()
                != defaults
                    .iter()
                    .filter(|one| one.on == on)
                    .cloned()
                    .collect::<Vec<Binding>>();

            Part {
                slug: job.slug.to_string(),
                action: job.action,
                does,
                on,
                plays: mine
                    .into_iter()
                    .map(|one| {
                        let Ok(here) = has(front, one);

                        Plays { here, binding: one.clone() }
                    })
                    .collect(),
                moved,
            }
        })
        .collect();

    parts.sort_by_key(|part| {
        let Ok(played) = part.played();
        let Ok(here) = part.here();

        match (played, here) {
            (Played::ByAPress, Has::Yes) => 0,
            (Played::ByAPress, Has::No) => 1,
            (Played::ByNothing, _) => 2,
        }
    });

    Ok(parts)
}

fn has(front: &Front, binding: &Binding) -> Result<Has, Never> {
    match binding.on {
        Input::Keyboard => return Ok(Has::Yes),
        Input::Pad => {},
    }

    let mut wanted: Vec<&str> = binding.held.iter().map(String::as_str).collect();

    wanted.push(&binding.pressed);

    for button in wanted {
        let Ok(trigger) = vocabulary::is_trigger(button);

        match trigger {
            Names::ATrigger => continue,
            Names::AButton => {},
        }

        let sends = match button_name(button) {
            Ok(named) => front.can_send(named)?,
            Err(_unnamed) => Has::No,
        };

        match sends {
            Has::No => return Ok(Has::No),
            Has::Yes => {},
        }
    }

    Ok(Has::Yes)
}

pub fn every(table: &Table) -> Result<BTreeMap<String, Vec<Binding>>, Never> {
    let Ok(every) = table.every();

    Ok(every.map(|(job, bound)| (job.slug.to_string(), bound.to_vec())).collect())
}

pub fn button_label(button: &str) -> Result<String, Never> {
    let Ok(spoken) = spoken_for(button);

    match spoken.contains(|letter: char| letter.is_uppercase()) {
        true => split(spoken),
        false => Ok(spoken.replace('-', " ")),
    }
}

fn split(named: &str) -> Result<String, Never> {
    let mut said = String::new();

    for letter in named.chars() {
        let seam = (letter.is_uppercase() || letter.is_ascii_digit()) && !said.is_empty();

        match seam {
            true => said.push(' '),
            false => {}
        }

        said.extend(letter.to_lowercase());
    }

    Ok(said)
}

pub fn rows(parts: &[Part], made: impl Fn(&Part) -> Row, putting_back: Handler) -> Result<Vec<Row>, Never> {
    let mut rows: Vec<Row> = Vec::new();

    match parts.iter().any(|part| part.moved) {
        true => {
            let Ok(back) = Row::new(PUT_BACK, Aside(PUT_BACK_ASIDE), putting_back);

            rows.push(back);
        }
        false => {}
    }

    rows.extend(parts.iter().map(made));

    Ok(rows)
}

pub fn row(part: &Part) -> Result<Row, Never> {
    let Ok(aside) = part.aside();
    let Ok(runs) = runs(part.action);

    match runs {
        Some(arguments) => Row::new(&part.does, Aside(&aside), Handler::Run(arguments)),
        None => Row::text(&part.does, Aside(&aside)),
    }
}

pub fn runs(action: Action) -> Result<Option<Vec<String>>, Never> {
    let tapped = match action == Action::PutAway {
        true => Action::CloseWindow,
        false => action,
    };
    let Ok(does) = tapped.does(ButtonPress::Down);

    let doing = match does {
        Some(doing) => doing,
        None => return Ok(None),
    };

    Ok(match doing {
        Effect::Run(arguments) => Some(arguments),
        Effect::Frame(_) | Effect::Tell(_) | Effect::Using(_) | Effect::Reconnected(_) => None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Add,
    Remove(Binding),
}

pub fn choices(part: &Part) -> Result<Vec<(String, Choice)>, Never> {
    let adding = match part.on {
        Input::Pad => ADD_ON_THE_PAD,
        Input::Keyboard => ADD_ON_A_KEYBOARD,
    };
    let mut choices = vec![(adding.to_string(), Choice::Add)];

    for one in &part.plays {
        let Ok(played) = one.binding.played();

        match played {
            Played::ByAPress => {
                let Ok(said) = aloud(&one.binding);

                choices.push((format!("{REMOVE} {said}"), Choice::Remove(one.binding.clone())));
            }
            Played::ByNothing => {},
        }
    }

    Ok(choices)
}

pub fn question(part: &Part) -> Result<String, Never> {
    let does = &part.does;

    Ok(match part.on {
        Input::Pad => format!("Press a button for \u{201c}{does}\u{201d}"),
        Input::Keyboard => format!("Press keys for \u{201c}{does}\u{201d}"),
    })
}

pub fn lowered(does: &str) -> Result<String, Never> {
    let mut letters = does.chars();

    Ok(match letters.next() {
        Some(first) => first.to_lowercase().collect::<String>() + letters.as_str(),
        None => does.to_string(),
    })
}

fn capitalised(does: &str) -> Result<String, Never> {
    let mut letters = does.chars();

    Ok(match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    })
}

#[cfg(test)]
mod tests {
    use console_core_external_programs::Program;

    use super::*;
    use std::collections::BTreeSet;

    use console_input_bindings::moved::Tasks;
    use console_input_gamepad::vocabulary::capability_of;

    type Failure = Box<dyn std::error::Error>;

    fn ordinary() -> Result<Front, Never> {
        let has: BTreeSet<String> = [
            "South", "East", "North", "West", "Start", "Select", "LeftBumper", "RightBumper",
            "DPadUp", "DPadDown", "DPadLeft", "DPadRight",
        ]
        .into_iter()
        .map(|button| {
            let Ok(capability) = capability_of(button);

            capability
        })
        .collect();

        Ok(Front { capabilities: Some(has), touchscreen: Some(false) })
    }

    fn moved(said: &str) -> Result<Table, Failure> {
        let tasks = Tasks::read(said)?;
        let Ok(table) = Table::of(&tasks);

        Ok(table)
    }

    fn on_the_pad(table: &Table, front: &Front) -> Result<Vec<Part>, Never> {
        super::parts(table, front, Input::Pad)
    }

    fn on_a_keyboard(table: &Table, front: &Front) -> Result<Vec<Part>, Never> {
        super::parts(table, front, Input::Keyboard)
    }

    fn laid_out(parts: &[Part]) -> Result<Vec<Row>, Never> {
        let Ok(name) = Program::True.name();
        let Ok(runs) = Handler::run(&[name]);

        super::rows(
            parts,
            |part| {
                let Ok(row) = super::row(part);

                row
            },
            runs,
        )
    }

    fn find_part<'a>(parts: &'a [Part], slug: &str) -> Result<&'a Part, Failure> {
        let found = parts.iter().find(|part| part.slug == slug).ok_or(format!("no job named {slug}"))?;

        Ok(found)
    }

    #[test]
    fn a_job_says_what_it_does_and_what_plays_it() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(parts) = on_the_pad(&table, &Front::default());
        let menu = find_part(&parts, "menu")?;

        assert_eq!(menu.does, "Open the menu");
        assert_eq!(menu.aside(), Ok("left paddle top".to_string()));
        assert!(!menu.moved);

        Ok(())
    }

    #[test]
    fn a_job_on_a_chord_says_what_is_held_with_it() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(parts) = on_the_pad(&table, &Front::default());
        let screenshot = find_part(&parts, "screenshot")?;

        assert_eq!(screenshot.aside(), Ok("l2 + right paddle bottom".to_string()));

        Ok(())
    }

    #[test]
    fn the_same_job_says_a_different_thing_on_the_keyboards_page() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let front = Front::default();
        let Ok(pad) = on_the_pad(&table, &front);
        let Ok(keyboard) = on_a_keyboard(&table, &front);
        let pad_settings = find_part(&pad, "settings")?;
        let keyboard_settings = find_part(&keyboard, "settings")?;

        assert_eq!(pad_settings.aside(), Ok("legion right".to_string()));
        assert_eq!(keyboard_settings.aside(), Ok("super + i".to_string()));

        Ok(())
    }

    #[test]
    fn a_job_with_nothing_on_this_input_says_which_input_it_has_nothing_on() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let front = Front::default();
        let Ok(pad) = on_the_pad(&table, &front);
        let Ok(keyboard) = on_a_keyboard(&table, &front);
        let pad_terminal = find_part(&pad, "terminal")?;
        let keyboard_put_away = find_part(&keyboard, "put-away")?;

        assert_eq!(pad_terminal.aside(), Ok(UNPLAYED_ON_THE_PAD.to_string()));
        assert_eq!(keyboard_put_away.aside(), Ok(UNPLAYED_ON_A_KEYBOARD.to_string()));

        Ok(())
    }

    #[test]
    fn every_job_is_on_every_page_so_that_any_of_them_can_be_given_a_place() {
        let Ok(table) = Table::ours();
        let front = Front::default();
        let Ok(pad) = on_the_pad(&table, &front);
        let Ok(keyboard) = on_a_keyboard(&table, &front);

        assert_eq!(pad.len(), keyboard.len());
    }

    #[test]
    fn a_job_bound_to_a_button_this_device_has_not_got_says_so() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let menu = find_part(&parts, "menu")?;

        assert_eq!(menu.here(), Ok(Has::No));
        assert_eq!(menu.aside(), Ok(NOWHERE.to_string()));

        Ok(())
    }

    #[test]
    fn a_chord_over_a_button_this_device_has_not_got_is_not_here_either() -> Result<(), Failure> {
        let table = moved("[jobs]\nmenu = \"left-paddle-top + a\"\n")?;
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let menu = find_part(&parts, "menu")?;

        assert_eq!(menu.here(), Ok(Has::No));

        Ok(())
    }

    #[test]
    fn a_key_is_here_whatever_the_front_of_the_machine_has_not_got() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(front) = ordinary();
        let Ok(typed) = on_a_keyboard(&table, &front);
        let settings = find_part(&typed, "settings")?;

        assert_eq!(settings.here(), Ok(Has::Yes));

        Ok(())
    }

    #[test]
    fn a_job_moved_onto_a_button_this_device_has_is_no_longer_missing() -> Result<(), Failure> {
        let table = moved("[jobs]\nmenu = \"r2 + a\"\n")?;
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let menu = find_part(&parts, "menu")?;

        assert_eq!(menu.here(), Ok(Has::Yes));
        assert!(menu.moved);
        assert_eq!(menu.aside(), Ok("r2 + a".to_string()));

        Ok(())
    }

    #[test]
    fn a_job_on_two_buttons_names_the_one_this_device_has() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let keyboard = find_part(&parts, "keyboard")?;

        assert_eq!(keyboard.plays.len(), 2);
        assert_eq!(keyboard.here(), Ok(Has::Yes));
        assert_eq!(keyboard.aside(), Ok("x".to_string()));

        Ok(())
    }

    #[test]
    fn what_plays_something_comes_first_and_a_job_with_nothing_on_it_last() {
        let Ok(table) = Table::ours();
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let order: Vec<(Played, Has)> = parts
            .iter()
            .map(|part| {
                let Ok(played) = part.played();
                let Ok(here) = part.here();

                (played, here)
            })
            .collect();

        assert_eq!(order.first(), Some(&(Played::ByAPress, Has::Yes)), "{order:?}");
        assert!(
            order.iter().skip_while(|one| one.0 != Played::ByNothing).all(|one| *one != (Played::ByAPress, Has::No)),
            "{order:?}"
        );
        assert_eq!(order.last().map(|one| one.0), Some(Played::ByNothing));
    }

    #[test]
    fn a_page_opens_on_what_is_bound_rather_than_on_a_screen_of_nothing() {
        let Ok(table) = Table::ours();
        let front = Front::default();
        let Ok(pad) = on_the_pad(&table, &front);
        let Ok(keyboard) = on_a_keyboard(&table, &front);

        for on in [pad, keyboard] {
            assert_eq!(on.first().map(Part::played), Some(Ok(Played::ByAPress)));
        }
    }

    #[test]
    fn a_row_does_what_it_names_and_the_paddle_closes_the_window_behind_the_panel() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(parts) = on_the_pad(&table, &Front::default());
        let Ok(files) = super::runs(Action::Files);
        let Ok(put_away) = super::runs(Action::PutAway);
        let Ok(close) = super::runs(Action::CloseWindow);
        let files_part = find_part(&parts, "files")?;
        let Ok(files_row) = super::row(files_part);
        let back_part = find_part(&parts, "back")?;
        let Ok(back_row) = super::row(back_part);

        assert_eq!(files, Some(vec!["/usr/local/bin/files".to_string()]));
        assert_eq!(put_away, close);
        assert!(put_away.is_some());
        assert!(files_row.does.is_some());
        assert!(back_row.does.is_none(), "going back is only a press");

        Ok(())
    }

    #[test]
    fn y_offers_to_add_and_to_remove_each_place_a_job_is_played() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(parts) = on_the_pad(&table, &Front::default());
        let keyboard_part = find_part(&parts, "keyboard")?;
        let Ok(keyboard) = choices(keyboard_part);
        let terminal_part = find_part(&parts, "terminal")?;
        let Ok(terminal) = choices(terminal_part);
        let said: Vec<&str> = keyboard.iter().map(|(said, _)| said.as_str()).collect();

        assert_eq!(said, [ADD_ON_THE_PAD, "Remove x", "Remove keyboard"]);
        assert_eq!(terminal, [(ADD_ON_THE_PAD.to_string(), Choice::Add)], "nothing to remove");

        let Ok(typed_parts) = on_a_keyboard(&table, &Front::default());
        let typed_terminal = find_part(&typed_parts, "terminal")?;
        let Ok(typed) = choices(typed_terminal);

        assert_eq!(typed.first().map(|(said, _)| said.as_str()), Some(ADD_ON_A_KEYBOARD));

        Ok(())
    }

    #[test]
    fn a_device_that_said_nothing_leaves_every_job_where_it_is() {
        let Ok(table) = Table::ours();
        let Ok(parts) = on_the_pad(&table, &Front::default());

        assert!(parts.iter().all(|part| part.here() == Ok(Has::Yes) || part.played() == Ok(Played::ByNothing)));
    }

    #[test]
    fn a_button_is_said_the_way_it_would_be_spoken() {
        assert_eq!(button_label("left-paddle-top"), Ok("left paddle top".to_string()));
        assert_eq!(button_label("LeftPaddle1"), Ok("left paddle top".to_string()));
        assert_eq!(button_label("QuickAccess"), Ok("legion right".to_string()));
        assert_eq!(button_label("RightPaddle3"), Ok("right paddle 3".to_string()));
        assert_eq!(button_label("LeftPaddle9"), Ok("left paddle 9".to_string()));
        assert_eq!(button_label("l1"), Ok("l1".to_string()));
    }

    #[test]
    fn the_card_asks_for_one_job_by_what_it_does_and_says_which_hand() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let front = Front::default();
        let Ok(pad) = on_the_pad(&table, &front);
        let Ok(keyboard) = on_a_keyboard(&table, &front);
        let pad_menu = find_part(&pad, "menu")?;
        let keyboard_menu = find_part(&keyboard, "menu")?;

        assert_eq!(
            question(pad_menu),
            Ok("Press a button for \u{201c}Open the menu\u{201d}".to_string())
        );
        assert_eq!(
            question(keyboard_menu),
            Ok("Press keys for \u{201c}Open the menu\u{201d}".to_string())
        );

        Ok(())
    }

    #[test]
    fn the_card_says_what_can_be_held() {
        assert!(WAITING_ON_A_KEYBOARD.contains("Super") && WAITING_ON_A_KEYBOARD.contains("Ctrl"));
        assert!(WAITING_ON_THE_PAD.contains("Hold"));
    }

    #[test]
    fn putting_it_all_back_is_the_first_row_once_something_has_moved() -> Result<(), Failure> {
        let Ok(table) = Table::ours();
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let Ok(plain) = laid_out(&parts);

        assert_ne!(plain.first().map(|row| row.says.clone()), Some(PUT_BACK.to_string()));

        let table = moved("[jobs]\nmenu = \"a\"\n")?;
        let Ok(parts) = on_the_pad(&table, &front);
        let Ok(after) = laid_out(&parts);

        assert_eq!(after.len(), plain.len().saturating_add(1));
        assert_eq!(after.first().map(|row| row.says.clone()), Some(PUT_BACK.to_string()));

        Ok(())
    }

    #[test]
    fn a_job_left_with_no_button_says_that_rather_than_that_the_device_lacks_one() -> Result<(), Failure> {
        let table = moved("[jobs]\nmenu = \"\"\n")?;
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let menu = find_part(&parts, "menu")?;

        assert_eq!(menu.played(), Ok(Played::ByNothing));
        assert!(menu.moved);
        assert_eq!(menu.aside(), Ok(UNPLAYED_ON_THE_PAD.to_string()));

        Ok(())
    }

    #[test]
    fn a_job_left_with_no_button_is_something_to_put_back() -> Result<(), Failure> {
        let table = moved("[jobs]\nmenu = \"\"\n")?;
        let Ok(front) = ordinary();
        let Ok(parts) = on_the_pad(&table, &front);
        let Ok(rows) = laid_out(&parts);

        assert_eq!(rows.first().map(|row| row.says.clone()), Some(PUT_BACK.to_string()));

        Ok(())
    }

    #[test]
    fn a_move_is_worked_out_against_every_job_on_every_input() {
        let Ok(table) = Table::ours();
        let Ok(every) = every(&table);
        let Ok(on) = Binding::pad("left-paddle-top");
        let Ok(key) = Binding::chord(Input::Keyboard, &["super"], "i");

        assert!(every.get("menu").is_some_and(|bound| bound.contains(&on)));
        assert!(
            every.get("settings").is_some_and(|bound| bound.contains(&key)),
            "a key taken by a move has to be findable to be taken"
        );
    }

    #[test]
    fn the_word_for_going_ahead_is_short_and_stands_for_nothing() {
        let words: Vec<&str> = PUT_BACK_YES.split_whitespace().collect();

        assert!(words.len() <= 2, "{PUT_BACK_YES:?} is a sentence where a verb was wanted");

        for standing in ["it", "them", "this", "that", "these", "those"] {
            assert!(
                !words.iter().any(|word| word.to_lowercase() == standing),
                "{PUT_BACK_YES:?} says {standing:?} where the question already said the thing"
            );
        }
    }
}
