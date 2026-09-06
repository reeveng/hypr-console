//! Everything this desktop has grown, in the order it grew.
//!
//! A check is one thing, and one feature. It says what somebody did and what
//! should have happened, and it is edited in place when the feature changes
//! rather than joined by a second one saying something different. Running them
//! in order walks the whole desktop, oldest first, and says which of it still
//! works.
//!
//! Large features are split, because "the d-pad works" is not a thing that
//! fails: left works or right works, and a check that presses both and asserts
//! once tells you neither which failed nor that only one did.
//!
//! Where a check runs decides what it can see, so it says what it needs by which
//! stage it is written for. A stage nothing is written for skips it and says so
//! rather than passing quietly.

pub mod brightness;
pub mod carry;
pub mod chooser;
pub mod close;
pub mod dpad;
pub mod download;
pub mod files;
pub mod game_mode;
pub mod guide;
pub mod held;
pub mod home;
pub mod icons;
pub mod input;
pub mod keyboard;
pub mod launcher;
pub mod music;
pub mod notices;
pub mod panel;
pub mod pointer;
pub mod screenshot;
pub mod services;
pub mod volume;
pub mod wallpaper;
pub mod workspaces;

use console_never::Never;
use console_test_stages::checking::{Check, Named};

pub const CHECKS: [&Check; 40] = [
    &workspaces::RIGHT,
    &workspaces::LEFT,
    &carry::CARRY,
    &carry::HALF,
    &close::CLOSE,
    &launcher::MENU,
    &guide::GUIDE,
    &screenshot::SHOT,
    &screenshot::ALONE,
    &panel::PANEL,
    &brightness::BRIGHTER,
    &brightness::DIMMER,
    &volume::LOUDER,
    &volume::QUIETER,
    &dpad::DPAD,
    &keyboard::KEYBOARD,
    &pointer::SCROLL,
    &pointer::TOUCHPAD,
    &services::SERVICES,
    &wallpaper::WALLPAPER,
    &panel::DRAWS,
    &keyboard::DRAWS,
    &panel::WITH_THE_KEYBOARD,
    &game_mode::GAME_MODE,
    &files::DRAWS,
    &services::STEADY,
    &notices::DRAWS,
    &download::DRAWS,
    &keyboard::EVERY_TIME,
    &keyboard::IN_A_PAGE,
    &home::WHOSE_BUTTONS,
    &home::PRESSABLE,
    &music::LIBRARY,
    &home::ARRANGING,
    &icons::ICONS,
    &home::POINTED,
    &input::OWNED,
    &held::AGAIN,
    &held::HOLDS_NOTHING,
    &files::UNZIPS,
];

pub fn chosen(words: &[String]) -> Result<Vec<&'static Check>, Never> {
    Ok(match words.is_empty() {
        true => CHECKS.to_vec(),
        false => CHECKS
            .into_iter()
            .filter(|check| check.named_by(words) == Ok(Named::Yes))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_checks_are_in_the_order_they_grew() {
        let names: Vec<&str> = CHECKS.iter().map(|check| check.name).collect();
        let mut ordered = names.clone();
        ordered.sort_unstable();
        assert_eq!(names, ordered);
    }

    #[test]
    fn no_two_checks_have_the_same_name() {
        let mut seen = BTreeSet::new();
        let twice: Vec<&str> =
            CHECKS.iter().map(|check| check.name).filter(|name| !seen.insert(*name)).collect();

        assert!(twice.is_empty(), "two checks are called {twice:?}");
    }

    #[test]
    fn every_check_is_written_for_somewhere() {
        for check in CHECKS {
            assert!(
                !check.bodies.is_empty(),
                "{} is written for nowhere",
                check.name
            );
        }
    }

    #[test]
    fn every_check_says_what_it_is_and_when_it_arrived() {
        for check in CHECKS {
            assert!(
                check.about.ends_with('.'),
                "{}'s line is not a sentence",
                check.name
            );
            let Ok(number) = check.number();

            assert_eq!(
                number.len(),
                3,
                "{} does not open with when",
                check.name
            );
            assert!(
                !check.feature.is_empty(),
                "{} is part of nothing",
                check.name
            );
            assert_eq!(
                check.since.len(),
                10,
                "{} did not arrive on a date",
                check.name
            );
        }
    }

    #[test]
    fn a_word_chooses_the_checks_about_it_and_nothing_chooses_all_of_them() {
        let Ok(every) = chosen(&[]);
        let Ok(about) = chosen(&["brightness".to_string()]);
        let Ok(none) = chosen(&["nothing-by-that-name".to_string()]);
        let brightness: Vec<&str> = about.iter().map(|check| check.name).collect();

        assert_eq!(every.len(), CHECKS.len());
        assert_eq!(brightness, ["090-brighter", "091-dimmer"]);
        assert!(none.is_empty());
    }
}
