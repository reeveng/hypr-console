//! A Legion Go you can press, on a machine that is not one.
//!
//! One fault for the whole crate, because one press walks all of it. A word in
//! a script names a button, a button is looked up in the vocabulary, a profile
//! says where it goes, and a device made through uinput is what it arrives on
//! -- so a press that fails fails at one of those four, and `GamepadError` is
//! which. What each of them used to say is the `Display` arm, so the emulator
//! prints the sentence it always printed and a caller that wants to tell a
//! word no one knows from a device this machine would not make now can.

use std::fmt;


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

#[derive(Debug)]
pub enum GamepadError {
    Capture(serde_json::Error),
    NoAxis(String, u16),
    Device(console_input_event_devices::Unmade),
    ListNodes(std::io::Error),
    NotAKeyName(String),
    NoSuchKey(String),
    NoSuchButton(String),
    NoMouseButton(String),
    NoPadButton(String),
    NotOneCode(profile::Kind, String),
    NoSuchProfile(String, Vec<String>),
    NoStick(String),
    NoTrigger(String),
    Read(std::path::PathBuf, std::io::Error),
    #[cfg(feature = "read")]
    Parse(std::path::PathBuf, serde_yaml_ng::Error),
    NotANumber(String),
    NotFound(&'static str),
    NoSuchStep(String),
    AtLine(u32, Box<GamepadError>),
}

impl fmt::Display for GamepadError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GamepadError::Capture(fault) => write!(to, "the capture does not parse: {fault}"),
            GamepadError::NoAxis(role, code) => write!(to, "{role} has no axis {code}"),
            GamepadError::Device(fault) => write!(to, "{fault}"),
            GamepadError::ListNodes(fault) => write!(to, "the device's nodes would not be listed: {fault}"),
            GamepadError::NotAKeyName(name) => write!(to, "not a key name: {name:?}"),
            GamepadError::NoSuchKey(name) => write!(to, "no such key: {name:?}"),
            GamepadError::NoSuchButton(spoken) => {
                let mut every: Vec<&str> =
                    vocabulary::BUTTONS.iter().map(|(said, _)| *said).collect();

                every.sort_unstable();

                write!(to, "no button called {spoken:?}; try one of {}", every.join(", "))
            }
            GamepadError::NoMouseButton(name) => write!(to, "no mouse button called {name:?}"),
            GamepadError::NoPadButton(name) => write!(to, "no pad button called {name:?}"),
            GamepadError::NotOneCode(kind, name) => {
                write!(to, "{kind:?} does not arrive as one code: {name:?}")
            }
            GamepadError::NoSuchProfile(name, every) => {
                write!(to, "no profile called {name:?}; there is {}", every.join(", "))
            }
            GamepadError::NoStick(which) => write!(to, "no stick called {which:?}"),
            GamepadError::NoTrigger(which) => write!(to, "no trigger called {which:?}"),
            GamepadError::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            #[cfg(feature = "read")]
            GamepadError::Parse(at, fault) => {
                write!(to, "{} does not parse: {fault}", at.display())
            }
            GamepadError::NotANumber(word) => write!(to, "{word:?} is not a number"),
            GamepadError::NotFound(what) => write!(to, "no {what}"),
            GamepadError::NoSuchStep(other) => write!(to, "no such thing as {other:?}"),
            GamepadError::AtLine(number, fault) => write!(to, "line {number}: {fault}"),
        }
    }
}

impl std::error::Error for GamepadError {}
