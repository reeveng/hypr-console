//! What the short-road radio was doing before the machine left for Steam.
//!
//! `/etc/bluetooth/main.conf` says the radio stays off until someone asks,
//! because a radio no one is listening to is a radio spending a battery, and
//! that once it has been asked for it stays on. Game Mode is the hole in that
//! sentence. Steam powers the adapter down a second after it comes up, and
//! coming back put nothing back: a keyboard someone was typing on, gone, with
//! nothing on the screen saying who took it. The journal says it plainly --
//! `btd_adv_monitor_power_down` one second after `steam`, and nothing after it
//! until someone reached for the tab.
//!
//! So what it was is written down on the way out and put back on the way in.
//! One way round only: a radio the person turned off before leaving is one
//! they turned off, and Steam's own power-down is not an answer worth keeping.
//! A radio someone turned on in Game Mode is theirs as well, and comes back
//! on.
//!
//! Under the runtime directory rather than the state directory, and that is
//! the whole of what keeps the policy. What is written there is gone at the
//! next boot, so a machine shut down from Game Mode comes up with the radio
//! off, the way a machine no one has asked yet is supposed to.
//!
//! The word `Powered` is bluez's and is spelled in two other crates -- the
//! settings tab, which offers the row that turns it on, and the bar, which
//! draws whether anything is connected. Each reads it for its own question and
//! this one is the third; the fourth is when it moves rather than when it is
//! copied again.

use std::fmt;
use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum Radio {
    #[words(word = "on")]
    On,
    #[words(word = "off")]
    Off,
}

pub const EVERY: [Radio; 2] = [Radio::On, Radio::Off];

pub const NAMED: &str = "radio";

const POWERED: &str = "Powered: yes";

impl Radio {
    pub fn of_word(word: &str) -> Result<Option<Radio>, Never> {
        Ok(EVERY.into_iter().find(|radio| {
            let Ok(said) = radio.word();

            said == word.trim()
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unkept {
    Unasked(String),
    Read(String),
    Unwritten(String),
}

impl fmt::Display for Unkept {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unkept::Unasked(why) => write!(to, "bluez would not say whether the radio is on: {why}"),
            Unkept::Read(why) => write!(to, "what the radio was doing: {why}"),
            Unkept::Unwritten(why) => write!(to, "writing down what the radio is doing: {why}"),
        }
    }
}

pub fn shown(said: &str) -> Result<Radio, Never> {
    Ok(match said.lines().any(|line| line.trim() == POWERED) {
        true => Radio::On,
        false => Radio::Off,
    })
}

pub fn putting_back(before: Option<Radio>, now: Radio) -> Result<Option<Vec<String>>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    Ok(match (before, now) {
        (Some(Radio::On), Radio::Off) => {
            Some(vec![bluetoothctl.to_string(), "power".to_string(), "on".to_string()])
        }
        (Some(Radio::On), Radio::On) | (Some(Radio::Off), _) | (None, _) => None,
    })
}

pub fn beside(runtime: &Path) -> Result<PathBuf, Never> {
    Ok(runtime.join(NAMED))
}

pub fn asked() -> Result<Radio, Unkept> {
    let Ok(mut bluetoothctl) = Program::Bluetoothctl.command();

    let said = match bluetoothctl.arg("show").output() {
        Ok(said) => said,
        Err(fault) => return Err(Unkept::Unasked(fault.to_string())),
    };

    let Ok(shown) = shown(&String::from_utf8_lossy(&said.stdout));

    Ok(shown)
}

pub fn remember(runtime: &Path, radio: Radio) -> Result<(), Unkept> {
    let Ok(at) = beside(runtime);
    let Ok(word) = radio.word();

    match std::fs::create_dir_all(runtime) {
        Ok(()) => {},
        Err(fault) => return Err(Unkept::Unwritten(fault.to_string())),
    }

    match console_core_atomic_writes::whole(&at, word.as_bytes()) {
        Ok(()) => Ok(()),
        Err(fault) => Err(Unkept::Unwritten(fault.to_string())),
    }
}

pub fn remembered(runtime: &Path) -> Result<Option<Radio>, Unkept> {
    let Ok(at) = beside(runtime);
    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        console_core_atomic_writes::Stored::Absent => Ok(None),
        console_core_atomic_writes::Stored::Failed(why) => Err(Unkept::Read(why)),
        console_core_atomic_writes::Stored::Text(said) => {
            let Ok(radio) = Radio::of_word(&said);

            Ok(radio)
        }
    }
}

pub fn forget(runtime: &Path) -> Result<(), Unkept> {
    let Ok(at) = beside(runtime);

    match std::fs::remove_file(&at) {
        Ok(()) => Ok(()),
        Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
            true => Ok(()),
            false => Err(Unkept::Read(fault.to_string())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: &str = "Controller A8:3B:76:91:B7:E2 (public)\n\tPowered: yes\n\tDiscoverable: no\n";

    const OFF: &str = "Controller A8:3B:76:91:B7:E2 (public)\n\tPowered: no\n";

    #[test]
    fn what_bluez_says_about_the_radio_is_read_off_its_own_word() {
        assert_eq!(shown(ON), Ok(Radio::On));
        assert_eq!(shown(OFF), Ok(Radio::Off));
        assert_eq!(shown(""), Ok(Radio::Off), "a bluez that said nothing is not a radio that is on");
    }

    #[test]
    fn a_radio_steam_turned_off_comes_back_and_one_someone_turned_off_does_not() {
        let Ok(bluetoothctl) = Program::Bluetoothctl.name();
        let on = vec![bluetoothctl.to_string(), "power".to_string(), "on".to_string()];

        assert_eq!(putting_back(Some(Radio::On), Radio::Off), Ok(Some(on)));
        assert_eq!(putting_back(Some(Radio::Off), Radio::Off), Ok(None), "they turned it off");
        assert_eq!(putting_back(Some(Radio::Off), Radio::On), Ok(None), "they turned it on there");
        assert_eq!(putting_back(Some(Radio::On), Radio::On), Ok(None), "nothing took it");
        assert_eq!(putting_back(None, Radio::Off), Ok(None), "a login is not a return");
    }

    #[test]
    fn what_was_written_down_is_what_comes_back() {
        let held = std::env::temp_dir().join(format!("console-radio-{}", std::process::id()));

        assert_eq!(remembered(&held), Ok(None), "nothing has been left here");

        let Ok(()) = remember(&held, Radio::On).map_err(|fault| panic!("{fault}"));

        assert_eq!(remembered(&held), Ok(Some(Radio::On)));

        let Ok(()) = forget(&held).map_err(|fault| panic!("{fault}"));

        assert_eq!(remembered(&held), Ok(None));

        let _ = std::fs::remove_dir_all(&held);
    }
}
