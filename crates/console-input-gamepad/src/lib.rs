//! A Legion Go you can press, on a machine that is not one.
//!
//! One fault for the whole crate, because one press walks all of it. A word in
//! a script names a button, a button is looked up in the vocabulary, a profile
//! says where it goes, and a device made through uinput is what it arrives on
//! -- so a press that fails fails at one of those four, and `Unpressed` is
//! which. What each of them used to say is the `Display` arm, so the emulator
//! prints the sentence it always printed and a caller that wants to tell a
//! word nobody knows from a device this machine would not make now can.

use std::fmt;

use console_core_words::Words;

pub mod allowing;
pub mod capture;
pub mod devices;
pub mod finding;
pub mod front;
pub mod go;

pub mod profile;
pub mod router;
pub mod routing;
pub mod script;
pub mod targets;
pub mod uinput;
pub mod vocabulary;

pub mod world;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Making {
    #[words(step = "no way in to /dev/uinput")]
    Opening,
    #[words(step = "a physical location")]
    Phys,
    #[words(step = "the keys")]
    Keys,
    #[words(step = "the relative axes")]
    RelativeAxes,
    #[words(step = "the misc codes")]
    Misc,
    #[words(step = "the properties")]
    Properties,
    #[words(step = "an axis")]
    Axis,
    #[words(step = "the device would not build")]
    Building,
    #[words(step = "the device's nodes would not be listed")]
    Listing,
    #[words(step = "the device's node would not be read")]
    Reading,
}

#[derive(Debug)]
pub enum Unpressed {
    Uncaptured(serde_json::Error),
    NoAxis(String, u16),
    PhysHasANul,
    Unmade(Making, std::io::Error),
    NotAKeyName(String),
    NoSuchKey(String),
    NoSuchButton(String),
    NoMouseButton(String),
    NoPadButton(String),
    NotOneCode(profile::Kind, String),
    NoSuchProfile(String, Vec<String>),
    NoStick(String),
    NoTrigger(String),
    Unreadable(std::path::PathBuf, std::io::Error),
    #[cfg(feature = "read")]
    Unparsed(std::path::PathBuf, serde_yaml_ng::Error),
    NotANumber(String),
    NothingSaid(&'static str),
    NoSuchStep(String),
    AtLine(usize, Box<Unpressed>),
}

impl fmt::Display for Unpressed {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unpressed::Uncaptured(fault) => write!(to, "the capture does not parse: {fault}"),
            Unpressed::NoAxis(role, code) => write!(to, "{role} has no axis {code}"),
            Unpressed::PhysHasANul => write!(to, "a phys with a nul in it"),
            Unpressed::Unmade(making, fault) => {
                let Ok(step) = making.step();

                write!(to, "{step}: {fault}")
            }
            Unpressed::NotAKeyName(name) => write!(to, "not a key name: {name:?}"),
            Unpressed::NoSuchKey(name) => write!(to, "no such key: {name:?}"),
            Unpressed::NoSuchButton(spoken) => {
                let mut every: Vec<&str> =
                    vocabulary::BUTTONS.iter().map(|(said, _)| *said).collect();

                every.sort_unstable();

                write!(to, "no button called {spoken:?}; try one of {}", every.join(", "))
            }
            Unpressed::NoMouseButton(name) => write!(to, "no mouse button called {name:?}"),
            Unpressed::NoPadButton(name) => write!(to, "no pad button called {name:?}"),
            Unpressed::NotOneCode(kind, name) => {
                write!(to, "{kind:?} does not arrive as one code: {name:?}")
            }
            Unpressed::NoSuchProfile(name, every) => {
                write!(to, "no profile called {name:?}; there is {}", every.join(", "))
            }
            Unpressed::NoStick(which) => write!(to, "no stick called {which:?}"),
            Unpressed::NoTrigger(which) => write!(to, "no trigger called {which:?}"),
            Unpressed::Unreadable(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            #[cfg(feature = "read")]
            Unpressed::Unparsed(at, fault) => {
                write!(to, "{} does not parse: {fault}", at.display())
            }
            Unpressed::NotANumber(word) => write!(to, "{word:?} is not a number"),
            Unpressed::NothingSaid(what) => write!(to, "no {what}"),
            Unpressed::NoSuchStep(other) => write!(to, "no such thing as {other:?}"),
            Unpressed::AtLine(number, fault) => write!(to, "line {number}: {fault}"),
        }
    }
}

impl std::error::Error for Unpressed {}
