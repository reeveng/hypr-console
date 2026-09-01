//! The setup screen, as rows: one thing the desktop does each, and what plays
//! it.
//!
//! Nothing here has a machine. What this device can send and what the table
//! says are both handed in, so the screen somebody sees on a handheld that has
//! never existed can be asked for on a laptop.

use std::collections::BTreeMap;

use console_controller::means::Table;
use console_pad::front::Front;
use console_pad::jobs::Binding;
use console_pad::vocabulary::{button_name, spoken_for};
use console_panel::page::{Does, Row};

/// One tab, because this is one question asked twenty-eight times.
pub const TABS: [&str; 1] = ["Buttons"];

/// What a row says where the device cannot send the button at all.
pub const NOWHERE: &str = "not on this device";

/// What a row says where nothing plays it, on a device that could.
///
/// Not the same as the one above, and the difference is whose doing it is. A
/// button this hardware has not got is a fact about the machine; a job with no
/// button is somebody having given its button to something else, and it is
/// undone by pressing a button on this row.
pub const UNPLAYED: &str = "no button";

/// The row that puts every job back where this desktop has it.
pub const PUT_BACK: &str = "Put every button back";

/// What is said beside it, and what is asked before it is done.
///
/// Short beside the row, because the column it sits in holds button names, and
/// the whole of the warning is the question: this row is the one thing here
/// that undoes work rather than doing any.
pub const PUT_BACK_ASIDE: &str = "undoes every move";
pub const PUT_BACK_SURE: &str = "Put every button back where it started?";
pub const PUT_BACK_YES: &str = "Put them back";

/// What the card says under the question while it is waiting for a press.
///
/// Two things in one line, because there is one line and both matter. A job
/// can be put on a chord, and nothing on the screen would otherwise say so:
/// somebody who does not know to hold a trigger first can only ever bind the
/// button on its own, which is half the table out of reach. And the way out of
/// a card raised by accident is to do nothing, which is worth saying to
/// somebody holding a machine that has just gone inert.
pub const WAITING: &str = "hold L2 or R2 first for a chord, or wait and nothing moves";

/// One way to play a job, and whether this device can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plays {
    pub binding: Binding,
    /// Whether this machine has that button at all.
    pub here: bool,
}

/// One thing the desktop does, and what plays it on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// What the job is called in the file, which is what the table is keyed by.
    pub slug: String,
    /// What it does, in the words the table says it in.
    pub does: String,
    /// What plays it here. More than one only where two buttons genuinely do
    /// one job.
    pub plays: Vec<Plays>,
    /// Whether somebody has moved it off where this desktop puts it.
    pub moved: bool,
}

impl Part {
    /// Whether any button plays this job at all.
    pub fn played(&self) -> bool {
        self.plays.iter().any(|one| one.binding.played())
    }

    /// Whether any of what plays it is a button this device has.
    pub fn here(&self) -> bool {
        self.plays.iter().any(|one| one.here)
    }

    /// What the row says beside the job: what plays it, or which kind of none.
    ///
    /// Only the ones this device has. A job on two buttons where the machine
    /// has one of them is a job you can reach, and naming the other would be
    /// naming a button nobody can press.
    pub fn aside(&self) -> String {
        match (self.played(), self.here()) {
            (false, _) => UNPLAYED.to_string(),
            (_, false) => NOWHERE.to_string(),
            _ => self
                .plays
                .iter()
                .filter(|one| one.here)
                .map(|one| aloud(&one.binding))
                .collect::<Vec<String>>()
                .join(" or "),
        }
    }
}

/// A binding, said out loud: what has to be held, and then the button.
///
/// `l2 + right paddle bottom`. The same words the file is written in, with the
/// dashes taken out: somebody who opens the file after using this screen finds
/// what they read on it.
pub fn aloud(binding: &Binding) -> String {
    let mut words: Vec<String> =
        binding.layer.said().into_iter().map(str::to_string).collect();
    words.push(said(&binding.button));
    words.join(" + ")
}

/// Every job: what this device has not got first, then the rest in the order
/// the table holds them.
pub fn parts(table: &Table, front: &Front) -> Vec<Part> {
    let mut parts: Vec<Part> = table
        .every()
        .map(|(job, bound)| {
            let ours: Vec<Binding> = job
                .bound
                .iter()
                .map(|(layer, button)| Binding::held(*layer, (*button).to_string()))
                .collect();
            Part {
                slug: job.slug.to_string(),
                does: capitalised(job.what.says()),
                plays: bound
                    .iter()
                    .map(|one| Plays { here: has(front, one), binding: one.clone() })
                    .collect(),
                moved: bound != ours.as_slice(),
            }
        })
        .collect();
    // What this device has not got, first. Everywhere else on this desktop a
    // list is in the order somebody wrote it down; here the rows that are the
    // reason the screen exists at all would be at the bottom of it, under four
    // rows about the d-pad. A stable sort, so the rest keep the order the
    // table names them in.
    parts.sort_by_key(Part::here);
    parts
}

/// Whether this machine has the button a binding names.
///
/// A machine that could not be asked is not a machine with no buttons: every
/// job is left where it is rather than the whole front of the device being
/// reported missing.
fn has(front: &Front, binding: &Binding) -> bool {
    match button_name(&binding.button) {
        Ok(named) => front.can_send(named),
        // A job with no button at all, and a button this repository has no
        // word for -- which is a button nothing could have been bound to.
        Err(_) => false,
    }
}

/// Every job and what plays it now, keyed the way the file is.
///
/// What a move is worked out against, because a button can be taken from a job
/// nobody has ever moved and so has no row in anybody's file to find it by.
pub fn every(parts: &[Part]) -> BTreeMap<String, Vec<Binding>> {
    parts
        .iter()
        .map(|part| {
            (part.slug.clone(), part.plays.iter().map(|one| one.binding.clone()).collect())
        })
        .collect()
}

/// A button, said the way somebody holding the thing would say it.
///
/// `left-paddle-top` is how this repository writes it and not how anybody
/// speaks. A button it has no word for at all -- and every device this has
/// never run on is full of them -- is split where the capitals are, which
/// turns `RightPaddle3` into something readable without a table of names for
/// hardware nobody here has seen.
pub fn said(button: &str) -> String {
    let spoken = spoken_for(button);
    match spoken.contains(|letter: char| letter.is_uppercase()) {
        true => split(spoken),
        false => spoken.replace('-', " "),
    }
}

fn split(named: &str) -> String {
    let mut said = String::new();
    for letter in named.chars() {
        if (letter.is_uppercase() || letter.is_ascii_digit()) && !said.is_empty() {
            said.push(' ');
        }
        said.extend(letter.to_lowercase());
    }
    said
}

/// The whole page: the way to undo the lot, and then every job.
///
/// Putting it all back is the first row rather than the last. It is not a way
/// out -- this page goes nowhere deeper, and the way out is the panel's own --
/// it is the way back from a machine somebody has made unusable, which is a
/// thing this screen can do in two presses: give the menu's paddle to something
/// else and the menu has no button. Under twenty-eight rows that is at the
/// bottom of a list you walk with the button you have just lost.
///
/// It says beside itself what it will undo, and it asks before it does it,
/// because it is now the row the highlight opens on.
///
/// Only once something has moved. A row offering to undo nothing is a row that
/// is in the way twenty-seven times out of twenty-eight, and on a machine
/// nobody has touched it would never have anything to do.
pub fn rows(parts: &[Part], moving: impl Fn(&Part) -> Does, putting_back: Does) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    if parts.iter().any(|part| part.moved) {
        rows.push(Row::new(PUT_BACK, PUT_BACK_ASIDE, putting_back));
    }
    rows.extend(parts.iter().map(|part| Row::new(&part.does, &part.aside(), moving(part))));
    rows
}

/// What the card asks, for one job.
pub fn question(part: &Part) -> String {
    format!("Press the button for {}", lowered(&part.does))
}

/// The same phrase inside a sentence rather than at the head of a row.
pub fn lowered(does: &str) -> String {
    let mut letters = does.chars();
    match letters.next() {
        Some(first) => first.to_lowercase().collect::<String>() + letters.as_str(),
        None => does.to_string(),
    }
}

/// And the other way, which is how a job reaches the head of a row.
fn capitalised(does: &str) -> String {
    let mut letters = does.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use console_pad::jobs::Jobs;
    use console_pad::vocabulary::capability_of;

    /// A device with no paddles and no Legion buttons, which is most of them.
    fn ordinary() -> Front {
        let has: BTreeSet<String> = [
            "South", "East", "North", "West", "Start", "Select", "LeftBumper", "RightBumper",
            "DPadUp", "DPadDown", "DPadLeft", "DPadRight",
        ]
        .into_iter()
        .map(capability_of)
        .collect();
        Front { capabilities: Some(has), touchscreen: Some(false) }
    }

    fn moved(said: &str) -> Table {
        Table::of(&Jobs::read(said).expect("a table"))
    }

    fn named<'a>(parts: &'a [Part], slug: &str) -> &'a Part {
        parts.iter().find(|part| part.slug == slug).expect("a job")
    }

    #[test]
    fn a_job_says_what_it_does_and_what_plays_it() {
        let parts = parts(&Table::ours(), &Front::default());
        let menu = named(&parts, "menu");
        assert_eq!(menu.does, "The menu");
        assert_eq!(menu.aside(), "left paddle top");
        assert!(!menu.moved);
    }

    /// A chord is said the way the file writes it, dashes and all taken out.
    #[test]
    fn a_job_on_a_chord_says_what_is_held_with_it() {
        let parts = parts(&Table::ours(), &Front::default());
        assert_eq!(named(&parts, "screenshot").aside(), "l2 + right paddle bottom");
    }

    #[test]
    fn a_job_bound_to_a_button_this_device_has_not_got_says_so() {
        let parts = parts(&Table::ours(), &ordinary());
        let menu = named(&parts, "menu");
        assert!(!menu.here());
        assert_eq!(menu.aside(), NOWHERE);
    }

    /// The one that says the whole feature works: a job moved onto a button
    /// this device does have stops being out of reach, and says where it went.
    #[test]
    fn a_job_moved_onto_a_button_this_device_has_is_no_longer_missing() {
        let parts = parts(&moved("[jobs]\nmenu = \"r2 + a\"\n"), &ordinary());
        let menu = named(&parts, "menu");
        assert!(menu.here() && menu.moved);
        assert_eq!(menu.aside(), "r2 + a");
    }

    /// Two buttons doing one job, on a device that has one of them: what the
    /// row names is the one that can be pressed.
    #[test]
    fn a_job_on_two_buttons_names_the_one_this_device_has() {
        let parts = parts(&Table::ours(), &ordinary());
        let keyboard = named(&parts, "keyboard");
        assert_eq!(keyboard.plays.len(), 2);
        assert!(keyboard.here());
        assert_eq!(keyboard.aside(), "x");
    }

    /// The rows that are the reason the screen exists come first. Left in the
    /// order the table names them, the paddles this device has not got would
    /// be under everything else.
    #[test]
    fn what_this_device_has_not_got_is_at_the_top() {
        let parts = parts(&Table::ours(), &ordinary());
        assert!(!parts[0].here());
        assert!(parts.last().expect("a job").here());
    }

    /// A machine that could not be asked is not a machine with no buttons.
    #[test]
    fn a_device_that_said_nothing_leaves_every_job_where_it_is() {
        let parts = parts(&Table::ours(), &Front::default());
        assert!(parts.iter().all(|part| part.here()));
    }

    /// The way somebody would say it, including for hardware nothing here has
    /// a word for.
    #[test]
    fn a_button_is_said_the_way_it_would_be_spoken() {
        assert_eq!(said("left-paddle-top"), "left paddle top");
        assert_eq!(said("LeftPaddle1"), "left paddle top");
        assert_eq!(said("QuickAccess"), "legion right");
        assert_eq!(said("RightPaddle3"), "right paddle 3");
        assert_eq!(said("LeftPaddle9"), "left paddle 9");
        // And the shoulders, which are not two words however they are read.
        assert_eq!(said("l1"), "l1");
    }

    #[test]
    fn the_card_asks_for_one_job_by_what_it_does() {
        let parts = parts(&Table::ours(), &Front::default());
        assert_eq!(question(named(&parts, "menu")), "Press the button for the menu");
    }

    /// The card says the chord is there, because nothing else on the screen
    /// does and a trigger is not something anybody holds by accident.
    #[test]
    fn the_card_says_a_chord_can_be_held() {
        assert!(WAITING.contains("L2") && WAITING.contains("R2"));
    }

    /// The way to undo everything is only there when there is something to
    /// undo, and when it is there it is the first row: the machine somebody
    /// wants it on is the one where the button they would walk the list with
    /// is the button they have just given away.
    #[test]
    fn putting_it_all_back_is_the_first_row_once_something_has_moved() {
        let putting_back = || Does::run(&["true"]);
        let plain = rows(&parts(&Table::ours(), &ordinary()), |_| Does::run(&["true"]), putting_back());
        assert_ne!(plain[0].says, PUT_BACK);

        let after = rows(
            &parts(&moved("[jobs]\nmenu = \"a\"\n"), &ordinary()),
            |_| Does::run(&["true"]),
            putting_back(),
        );
        assert_eq!(after.len(), plain.len() + 1);
        assert_eq!(after[0].says, PUT_BACK);
        assert_eq!(after[0].aside, PUT_BACK_ASIDE);
    }

    /// A job somebody took the button off says so, and says it differently
    /// from a job on hardware that never had one.
    #[test]
    fn a_job_left_with_no_button_says_that_rather_than_that_the_device_lacks_one() {
        let parts = parts(&moved("[jobs]\nmenu = \"\"\n"), &ordinary());
        let menu = named(&parts, "menu");
        assert!(!menu.played() && !menu.here() && menu.moved);
        assert_eq!(menu.aside(), UNPLAYED);
        // And it is at the top, with the rest of what cannot be pressed.
        assert!(!parts[0].here());
    }

    /// Which is a row the reset can be reached from, so the reset is offered.
    #[test]
    fn a_job_left_with_no_button_is_something_to_put_back() {
        let rows = rows(
            &parts(&moved("[jobs]\nmenu = \"\"\n"), &ordinary()),
            |_| Does::run(&["true"]),
            Does::run(&["true"]),
        );
        assert_eq!(rows[0].says, PUT_BACK);
    }

    /// What a move is worked out against is every job on the screen, and not
    /// only the ones somebody has already moved.
    #[test]
    fn a_move_is_worked_out_against_every_job_there_is() {
        let parts = parts(&Table::ours(), &Front::default());
        let every = every(&parts);
        assert_eq!(every.len(), parts.len());
        assert_eq!(every.get("menu").expect("the menu")[0], Binding::on("left-paddle-top"));
    }
}
