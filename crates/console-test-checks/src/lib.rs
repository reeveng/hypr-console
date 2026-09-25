//! The checks: one per feature, in the order the desktop grew them.
//!
//! A check is one thing, and one feature. It says what someone did and what
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

pub mod architecture;
pub mod bluetooth;
pub mod brightness;
pub mod carry;
pub mod picker;
pub mod close;
pub mod control_center;
pub mod dpad;
pub mod download;
pub mod files;
pub mod game_mode;
pub mod guide;
pub mod handoff;
pub mod home;
pub mod icons;
pub mod input;
pub mod login_pattern;
pub mod keyboard;
pub mod language;
pub mod launcher;
pub mod music;
pub mod notifications;
pub mod panel;
pub mod pointer;
pub mod resource_usage;
pub mod resume;
pub mod screenshot;
pub mod services;
pub mod typing;
pub mod updating;
pub mod volume;
pub mod wallpaper;
pub mod workspaces;

use std::fmt;
use std::path::PathBuf;

use console_core_never::Never;
use console_test_stages::Error;
use console_test_stages::checking::{Check, Named, Why};

#[derive(Debug)]
pub enum Unchecked {
    Stage(Error),
    Machine(std::io::Error),
    Read(PathBuf, std::io::Error),
    Temporary(console_core_temporary_directories::Unmade),
    Unparsed(toml::de::Error),
    NoGround(PathBuf),
    ShowingInstead(String),
    NotInTheTable(String, Vec<String>),
    StillRepainting,
    NoPlate(String),
    NoColor(String),
    NoRuntime,
    NoSuchCheck,
    SomeonesMachine,
    Concurrently(console_concurrency::Error),
}

impl fmt::Display for Unchecked {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unchecked::Stage(fault) => write!(to, "{fault}"),
            Unchecked::Machine(fault) => write!(to, "{fault}"),
            Unchecked::Read(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unchecked::Temporary(fault) => write!(to, "{fault}"),
            Unchecked::Concurrently(fault) => write!(to, "{fault}"),
            Unchecked::Unparsed(fault) => write!(to, "{fault}"),
            Unchecked::NoGround(at) => {
                write!(to, "{} sets no ground color", at.display())
            }
            Unchecked::ShowingInstead(showing) => {
                write!(to, "the wallpaper daemon is showing {showing}")
            }
            Unchecked::NotInTheTable(path, names) => write!(
                to,
                "the wallpaper is {path}, which theme/sky.toml does not name. It names {}",
                names.join(", ")
            ),
            Unchecked::StillRepainting => write!(
                to,
                "the home screen went on repainting with the pointer standing still"
            ),
            Unchecked::NoPlate(spent) => {
                write!(to, "the palette spends no {spent} for a square to be read by")
            }
            Unchecked::NoColor(name) => {
                write!(to, "the palette this machine spends has no {name}")
            }
            Unchecked::NoRuntime => write!(
                to,
                "XDG_RUNTIME_DIR: nothing says where this session keeps its marks"
            ),
            Unchecked::NoSuchCheck => write!(to, "no checks by that name"),
            Unchecked::SomeonesMachine => write!(
                to,
                "that is someone's machine. Add --dry-run to see what would happen, \
                 or --yes to do it."
            ),
        }
    }
}

impl std::error::Error for Unchecked {}

impl From<Error> for Unchecked {
    fn from(fault: Error) -> Self {
        Unchecked::Stage(fault)
    }
}

impl From<Unchecked> for Why {
    fn from(fault: Unchecked) -> Self {
        Why::Failed(fault.to_string())
    }
}

pub const CHECKS: [&Check; 65] = [
    &workspaces::RIGHT,
    &workspaces::LEFT,
    &workspaces::TAPPED,
    &workspaces::ANOTHER,
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
    &panel::PUT_AWAY_AT_ONCE,
    &game_mode::GAME_MODE,
    &files::DRAWS,
    &services::STEADY,
    &notifications::DRAWS,
    &notifications::CARD,
    &notifications::TOUCHED,
    &download::DRAWS,
    &keyboard::EVERY_TIME,
    &keyboard::IN_A_PAGE,
    &home::WHOSE_BUTTONS,
    &home::PRESSABLE,
    &music::LIBRARY,
    &music::QUIET,
    &music::AGAIN,
    &music::AWAKE,
    &home::ARRANGING,
    &icons::ICONS,
    &home::POINTED,
    &input::OWNED,
    &handoff::AGAIN,
    &handoff::HOLDS_NOTHING,
    &handoff::LEFT_NOTHING,
    &files::UNZIPS,
    &resume::REFUSED,
    &resume::AGAIN,
    &resume::NOT_TWICE,
    &language::HOUR,
    &language::FIRST_ROW,
    &typing::HANDED,
    &typing::A_KEY,
    &typing::LAST_PRESS,
    &bluetooth::LOOKS,
    &updating::FILLS,
    &resource_usage::KEPT,
    &resource_usage::ON_ITS_OWN,
    &login_pattern::STILL,
    &login_pattern::BY_HAND,
    &architecture::MAPPED,
    &control_center::PULLED,
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
    fn nothing_carried_is_a_check_this_tree_has_stopped_having() {
        let Ok(carried) = console_test_stages::baseline::lengths();
        let Ok(names) = carried.names();
        let every: Vec<&str> = CHECKS.iter().map(|check| check.name).collect();
        let gone: Vec<&String> =
            names.iter().filter(|name| !every.contains(&name.as_str())).collect();

        assert!(gone.is_empty(), "{gone:?} is carried as a length and is not a check any more");
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
