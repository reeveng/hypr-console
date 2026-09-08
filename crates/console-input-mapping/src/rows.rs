//! The setup screen, as rows: one thing the desktop does each, and what plays
//! it.
//!
//! Nothing here has a machine. What this device can send and what the table
//! says are both handed in, so the screen somebody sees on a handheld that has
//! never existed can be asked for on a laptop.
//!
//! One page per input, and the same rows on each. A job is a job whichever
//! hand is reaching for it, so the page does not change what it lists when you
//! walk from the controller to the keyboard -- only what it says beside each
//! row. That is why `parts` takes an input rather than returning everything at
//! once: a row showing a button and a chord and a key all at the same time is a
//! row nobody can read on a screen this size, and a person changing what a key
//! does is not thinking about the pad while they do it.

use std::collections::BTreeMap;

use console_input_controller::means::{Table, ours};
use console_input_gamepad::devices::Has;
use console_input_gamepad::front::Front;
use console_input_gamepad::vocabulary::{self, Names, button_name, spoken_for};
use console_input_bindings::bound::{Binding, Input, Played};
use console_core_never::Never;
use console_panel::page::{Does, Row};

pub const NOWHERE: &str = "not on this device";

pub const UNPLAYED_ON_THE_PAD: &str = "no button";

pub const UNPLAYED_ON_A_KEYBOARD: &str = "no key";

pub const PUT_BACK: &str = "Put every button back";

pub const PUT_BACK_ASIDE: &str = "undoes every move";
pub const PUT_BACK_SURE: &str = "Put every button back where it started?";
pub const PUT_BACK_YES: &str = "Put them back";

pub const WAITING_ON_THE_PAD: &str =
    "hold anything you want held first, or wait and nothing moves";

pub const WAITING_ON_A_KEYBOARD: &str =
    "hold Super, Ctrl, Shift or Alt first, or wait and nothing moves";

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
        let Ok(said) = said(word);

        words.push(said);
    }

    let Ok(pressed) = said(&binding.pressed);

    words.push(pressed);

    Ok(words.join(" + "))
}

pub fn parts(table: &Table, front: &Front, on: Input) -> Result<Vec<Part>, Never> {
    let Ok(every) = table.every();

    let mut parts: Vec<Part> = every
        .map(|(job, bound)| {
            let Ok(defaults) = ours(job);
            let mine: Vec<&Binding> = bound.iter().filter(|one| one.on == on).collect();
            let theirs: Vec<&Binding> = defaults.iter().filter(|one| one.on == on).collect();
            let Ok(says) = job.what.says();
            let Ok(does) = capitalised(says);
            let moved = mine.iter().map(|one| (*one).clone()).collect::<Vec<Binding>>()
                != theirs.iter().map(|one| (*one).clone()).collect::<Vec<Binding>>();

            Part {
                slug: job.slug.to_string(),
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
            (Played::ByAPress, Has::No) => 0,
            (Played::ByNothing, _) => 1,
            (Played::ByAPress, Has::Yes) => 2,
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

pub fn said(button: &str) -> Result<String, Never> {
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

pub fn rows(parts: &[Part], moving: impl Fn(&Part) -> Does, putting_back: Does) -> Result<Vec<Row>, Never> {
    let mut rows: Vec<Row> = Vec::new();

    match parts.iter().any(|part| part.moved) {
        true => {
            let Ok(back) = Row::new(PUT_BACK, PUT_BACK_ASIDE, putting_back);

            rows.push(back);
        }
        false => {}
    }

    rows.extend(parts.iter().map(|part| {
        let Ok(aside) = part.aside();
        let Ok(row) = Row::new(&part.does, &aside, moving(part));

        row
    }));
    Ok(rows)
}

pub fn question(part: &Part) -> Result<String, Never> {
    let Ok(does) = lowered(&part.does);

    Ok(match part.on {
        Input::Pad => format!("Press the button for {does}"),
        Input::Keyboard => format!("Press the keys for {does}"),
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

    use console_input_bindings::moved::Jobs;
    use console_input_gamepad::vocabulary::capability_of;

    fn ordinary() -> Front {
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

        Front { capabilities: Some(has), touchscreen: Some(false) }
    }

    fn runs() -> Does {
        let Ok(name) = Program::True.name();
        let Ok(does) = Does::run(&[name]);

        does
    }

    fn ours() -> Table {
        let Ok(table) = Table::ours();

        table
    }

    fn moved(said: &str) -> Table {
        let Ok(table) = Table::of(&Jobs::read(said).expect("a table"));

        table
    }

    fn aside(part: &Part) -> String {
        let Ok(said) = part.aside();

        said
    }

    fn here(part: &Part) -> Has {
        let Ok(has) = part.here();

        has
    }

    fn played(part: &Part) -> Played {
        let Ok(played) = part.played();

        played
    }

    fn parts(table: &Table, front: &Front) -> Vec<Part> {
        let Ok(parts) = super::parts(table, front, Input::Pad);

        parts
    }

    fn typed(table: &Table, front: &Front) -> Vec<Part> {
        let Ok(parts) = super::parts(table, front, Input::Keyboard);

        parts
    }

    fn said(button: &str) -> String {
        let Ok(said) = super::said(button);

        said
    }

    fn question(part: &Part) -> String {
        let Ok(asked) = super::question(part);

        asked
    }

    fn rows(parts: &[Part], moving: impl Fn(&Part) -> Does, putting_back: Does) -> Vec<Row> {
        let Ok(rows) = super::rows(parts, moving, putting_back);

        rows
    }

    fn every(table: &Table) -> BTreeMap<String, Vec<Binding>> {
        let Ok(every) = super::every(table);

        every
    }

    fn named<'a>(parts: &'a [Part], slug: &str) -> &'a Part {
        parts.iter().find(|part| part.slug == slug).expect("a job")
    }

    #[test]
    fn a_job_says_what_it_does_and_what_plays_it() {
        let parts = parts(&ours(), &Front::default());
        let menu = named(&parts, "menu");

        assert_eq!(menu.does, "The menu");
        assert_eq!(aside(menu), "left paddle top");
        assert!(!menu.moved);
    }

    #[test]
    fn a_job_on_a_chord_says_what_is_held_with_it() {
        let parts = parts(&ours(), &Front::default());

        assert_eq!(aside(named(&parts, "screenshot")), "l2 + right paddle bottom");
    }

    #[test]
    fn the_same_job_says_a_different_thing_on_the_keyboards_page() {
        let front = Front::default();

        assert_eq!(aside(named(&parts(&ours(), &front), "settings")), "legion right");
        assert_eq!(aside(named(&typed(&ours(), &front), "settings")), "super + i");
    }

    #[test]
    fn a_job_with_nothing_on_this_input_says_which_input_it_has_nothing_on() {
        let front = Front::default();

        assert_eq!(aside(named(&parts(&ours(), &front), "terminal")), UNPLAYED_ON_THE_PAD);
        assert_eq!(aside(named(&typed(&ours(), &front), "put-away")), UNPLAYED_ON_A_KEYBOARD);
    }

    #[test]
    fn every_job_is_on_every_page_so_that_any_of_them_can_be_given_a_place() {
        let front = Front::default();

        assert_eq!(parts(&ours(), &front).len(), typed(&ours(), &front).len());
    }

    #[test]
    fn a_job_bound_to_a_button_this_device_has_not_got_says_so() {
        let parts = parts(&ours(), &ordinary());
        let menu = named(&parts, "menu");

        assert_eq!(here(menu), Has::No);
        assert_eq!(aside(menu), NOWHERE);
    }

    #[test]
    fn a_chord_over_a_button_this_device_has_not_got_is_not_here_either() {
        let parts = parts(&moved("[jobs]\nmenu = \"left-paddle-top + a\"\n"), &ordinary());

        assert_eq!(here(named(&parts, "menu")), Has::No);
    }

    #[test]
    fn a_key_is_here_whatever_the_front_of_the_machine_has_not_got() {
        let typed = typed(&ours(), &ordinary());

        assert_eq!(here(named(&typed, "settings")), Has::Yes);
    }

    #[test]
    fn a_job_moved_onto_a_button_this_device_has_is_no_longer_missing() {
        let parts = parts(&moved("[jobs]\nmenu = \"r2 + a\"\n"), &ordinary());
        let menu = named(&parts, "menu");

        assert_eq!(here(menu), Has::Yes);
        assert!(menu.moved);
        assert_eq!(aside(menu), "r2 + a");
    }

    #[test]
    fn a_job_on_two_buttons_names_the_one_this_device_has() {
        let parts = parts(&ours(), &ordinary());
        let keyboard = named(&parts, "keyboard");

        assert_eq!(keyboard.plays.len(), 2);
        assert_eq!(here(keyboard), Has::Yes);
        assert_eq!(aside(keyboard), "x");
    }

    #[test]
    fn what_this_device_has_not_got_is_at_the_top() {
        let parts = parts(&ours(), &ordinary());

        assert_eq!(parts.first().map(here), Some(Has::No));
        assert_eq!(parts.last().map(here), Some(Has::Yes));
    }

    #[test]
    fn a_device_that_said_nothing_leaves_every_job_where_it_is() {
        let parts = parts(&ours(), &Front::default());

        assert!(parts.iter().all(|part| here(part) == Has::Yes || played(part) == Played::ByNothing));
    }

    #[test]
    fn a_button_is_said_the_way_it_would_be_spoken() {
        assert_eq!(said("left-paddle-top"), "left paddle top");
        assert_eq!(said("LeftPaddle1"), "left paddle top");
        assert_eq!(said("QuickAccess"), "legion right");
        assert_eq!(said("RightPaddle3"), "right paddle 3");
        assert_eq!(said("LeftPaddle9"), "left paddle 9");
        assert_eq!(said("l1"), "l1");
    }

    #[test]
    fn the_card_asks_for_one_job_by_what_it_does_and_says_which_hand() {
        let front = Front::default();

        assert_eq!(
            question(named(&parts(&ours(), &front), "menu")),
            "Press the button for the menu"
        );
        assert_eq!(
            question(named(&typed(&ours(), &front), "menu")),
            "Press the keys for the menu"
        );
    }

    #[test]
    fn the_card_says_what_can_be_held() {
        assert!(WAITING_ON_A_KEYBOARD.contains("Super") && WAITING_ON_A_KEYBOARD.contains("Ctrl"));
        assert!(WAITING_ON_THE_PAD.contains("held"));
    }

    #[test]
    fn putting_it_all_back_is_the_first_row_once_something_has_moved() {
        let plain = rows(&parts(&ours(), &ordinary()), |_| runs(), runs());

        assert_ne!(plain.first().map(|row| row.says.clone()), Some(PUT_BACK.to_string()));

        let after = rows(&parts(&moved("[jobs]\nmenu = \"a\"\n"), &ordinary()), |_| runs(), runs());

        assert_eq!(after.len(), plain.len().saturating_add(1));
        assert_eq!(after.first().map(|row| row.says.clone()), Some(PUT_BACK.to_string()));
    }

    #[test]
    fn a_job_left_with_no_button_says_that_rather_than_that_the_device_lacks_one() {
        let parts = parts(&moved("[jobs]\nmenu = \"\"\n"), &ordinary());
        let menu = named(&parts, "menu");

        assert_eq!(played(menu), Played::ByNothing);
        assert!(menu.moved);
        assert_eq!(aside(menu), UNPLAYED_ON_THE_PAD);
    }

    #[test]
    fn a_job_left_with_no_button_is_something_to_put_back() {
        let rows = rows(&parts(&moved("[jobs]\nmenu = \"\"\n"), &ordinary()), |_| runs(), runs());

        assert_eq!(rows.first().map(|row| row.says.clone()), Some(PUT_BACK.to_string()));
    }

    #[test]
    fn a_move_is_worked_out_against_every_job_on_every_input() {
        let table = ours();
        let every = every(&table);
        let Ok(on) = Binding::pad("left-paddle-top");
        let Ok(key) = Binding::holding(Input::Keyboard, &["super"], "i");

        assert!(every.get("menu").is_some_and(|bound| bound.contains(&on)));
        assert!(
            every.get("settings").is_some_and(|bound| bound.contains(&key)),
            "a key taken by a move has to be findable to be taken"
        );
    }
}
