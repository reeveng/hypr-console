//! The setup screen, as rows: one thing the desktop does each, and what plays
//! it.
//!
//! Nothing here has a machine. What this device can send and what the table
//! says are both handed in, so the screen somebody sees on a handheld that has
//! never existed can be asked for on a laptop.

use std::collections::BTreeMap;

use console_controller::means::Table;
use console_gamepad::devices::Has;
use console_gamepad::front::Front;
use console_gamepad::jobs::{Binding, Played};
use console_gamepad::vocabulary::{button_name, spoken_for};
use console_never::Never;
use console_panel::page::{Does, Row};

pub const TABS: [&str; 1] = ["Buttons"];

pub const NOWHERE: &str = "not on this device";

pub const UNPLAYED: &str = "no button";

pub const PUT_BACK: &str = "Put every button back";

pub const PUT_BACK_ASIDE: &str = "undoes every move";
pub const PUT_BACK_SURE: &str = "Put every button back where it started?";
pub const PUT_BACK_YES: &str = "Put them back";

pub const WAITING: &str = "hold L2 or R2 first for a chord, or wait and nothing moves";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plays {
    pub binding: Binding,
    pub here: Has,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub slug: String,
    pub does: String,
    pub plays: Vec<Plays>,
    pub moved: bool,
}

impl Part {
    pub fn played(&self) -> Result<Played, Never> {
        let by_a_button = self.plays.iter().any(|one| {
            let Ok(played) = one.binding.played();

            played == Played::ByAButton
        });

        Ok(match by_a_button {
            true => Played::ByAButton,
            false => Played::ByNothing,
        })
    }

    pub fn here(&self) -> Result<Has, Never> {
        Ok(match self.plays.iter().any(|one| one.here == Has::Yes) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    pub fn aside(&self) -> Result<String, Never> {
        let Ok(played) = self.played();
        let Ok(here) = self.here();

        Ok(match (played, here) {
            (Played::ByNothing, _) => UNPLAYED.to_string(),
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
    let Ok(layer) = binding.layer.said();
    let mut words: Vec<String> = layer.into_iter().map(str::to_string).collect();
    let Ok(button) = said(&binding.button);

    words.push(button);
    Ok(words.join(" + "))
}

pub fn parts(table: &Table, front: &Front) -> Result<Vec<Part>, Never> {
    let Ok(every) = table.every();

    let mut parts: Vec<Part> = every
        .map(|(job, bound)| {
            let ours: Vec<Binding> = job
                .bound
                .iter()
                .map(|(layer, button)| {
                    let Ok(held) = Binding::held(*layer, (*button).to_string());

                    held
                })
                .collect();
            let Ok(says) = job.what.says();
            let Ok(does) = capitalised(says);

            Part {
                slug: job.slug.to_string(),
                does,
                plays: bound
                    .iter()
                    .map(|one| {
                        let Ok(here) = has(front, one);

                        Plays { here, binding: one.clone() }
                    })
                    .collect(),
                moved: bound != ours.as_slice(),
            }
        })
        .collect();
    parts.sort_by_key(|part| {
        let Ok(here) = part.here();

        match here {
            Has::No => 0,
            Has::Yes => 1,
        }
    });
    Ok(parts)
}

fn has(front: &Front, binding: &Binding) -> Result<Has, Never> {
    match button_name(&binding.button) {
        Ok(named) => front.can_send(named),
        Err(_) => Ok(Has::No),
    }
}

pub fn every(parts: &[Part]) -> Result<BTreeMap<String, Vec<Binding>>, Never> {
    Ok(parts
        .iter()
        .map(|part| {
            (part.slug.clone(), part.plays.iter().map(|one| one.binding.clone()).collect())
        })
        .collect())
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

    Ok(format!("Press the button for {does}"))
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
    use console_external_programs::Program;

    use super::*;
    use std::collections::BTreeSet;

    use console_gamepad::jobs::Jobs;
    use console_gamepad::vocabulary::capability_of;

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
        let Ok(parts) = super::parts(table, front);

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

    fn every(parts: &[Part]) -> BTreeMap<String, Vec<Binding>> {
        let Ok(every) = super::every(parts);

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
    fn a_job_bound_to_a_button_this_device_has_not_got_says_so() {
        let parts = parts(&ours(), &ordinary());
        let menu = named(&parts, "menu");
        assert_eq!(here(menu), Has::No);
        assert_eq!(aside(menu), NOWHERE);
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
        assert_eq!(here(&parts[0]), Has::No);
        assert_eq!(here(parts.last().expect("a job")), Has::Yes);
    }

    #[test]
    fn a_device_that_said_nothing_leaves_every_job_where_it_is() {
        let parts = parts(&ours(), &Front::default());
        assert!(parts.iter().all(|part| here(part) == Has::Yes));
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
    fn the_card_asks_for_one_job_by_what_it_does() {
        let parts = parts(&ours(), &Front::default());
        assert_eq!(question(named(&parts, "menu")), "Press the button for the menu");
    }

    #[test]
    fn the_card_says_a_chord_can_be_held() {
        assert!(WAITING.contains("L2") && WAITING.contains("R2"));
    }

    #[test]
    fn putting_it_all_back_is_the_first_row_once_something_has_moved() {
        let plain = rows(&parts(&ours(), &ordinary()), |_| runs(), runs());
        assert_ne!(plain[0].says, PUT_BACK);

        let after = rows(&parts(&moved("[jobs]\nmenu = \"a\"\n"), &ordinary()), |_| runs(), runs());
        assert_eq!(after.len(), plain.len() + 1);
        assert_eq!(after[0].says, PUT_BACK);
        assert_eq!(after[0].aside, PUT_BACK_ASIDE);
    }

    #[test]
    fn a_job_left_with_no_button_says_that_rather_than_that_the_device_lacks_one() {
        let parts = parts(&moved("[jobs]\nmenu = \"\"\n"), &ordinary());
        let menu = named(&parts, "menu");
        assert_eq!(played(menu), Played::ByNothing);
        assert_eq!(here(menu), Has::No);
        assert!(menu.moved);
        assert_eq!(aside(menu), UNPLAYED);
        assert_eq!(here(&parts[0]), Has::No);
    }

    #[test]
    fn a_job_left_with_no_button_is_something_to_put_back() {
        let rows = rows(&parts(&moved("[jobs]\nmenu = \"\"\n"), &ordinary()), |_| runs(), runs());
        assert_eq!(rows[0].says, PUT_BACK);
    }

    #[test]
    fn a_move_is_worked_out_against_every_job_there_is() {
        let parts = parts(&ours(), &Front::default());
        let every = every(&parts);
        assert_eq!(every.len(), parts.len());
        let Ok(on) = Binding::on("left-paddle-top");

        assert_eq!(every.get("menu").expect("the menu")[0], on);
    }
}
