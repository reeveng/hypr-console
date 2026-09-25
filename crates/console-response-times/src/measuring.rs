//! Whether anything on this device writes down what it measured.
//!
//! Every ledger on the device -- what someone waited for, what the machine
//! spent while nobody watched, and whatever is measured next -- asks this one
//! setting before it writes, so turning measuring off is one line in the
//! defaults file rather than a list of services somebody has to remember. The
//! word is spelled here and nowhere else: a ledger that read the key itself
//! would be a second opinion about what `off` means.
//!
//! Nothing chosen is `On`, because that is what the device did before there
//! was a choice. A value that is neither word is said out loud and read as
//! `On` too: losing a week of lines to a typo is the quieter fault.

use console_core_never::Never;
use console_core_words::Words;

pub const SETTING: &str = "measuring";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Measuring {
    #[words(key = "on")]
    On,
    #[words(key = "off")]
    Off,
}

impl Measuring {
    pub fn flipped(self) -> Result<Measuring, Never> {
        match self {
            Measuring::On => Ok(Measuring::Off),
            Measuring::Off => Ok(Measuring::On),
        }
    }
}

pub fn read(said: Option<&str>) -> Result<Measuring, Never> {
    let said = match said {
        Some(said) => said,
        None => return Ok(Measuring::On),
    };

    let Ok(found) = Measuring::from_key(said);

    match found {
        Some(chosen) => Ok(chosen),
        None => {
            eprintln!("console-response-times: {SETTING}={said} is neither on nor off; measuring stays on");

            Ok(Measuring::On)
        }
    }
}

pub fn chosen() -> Result<Measuring, Never> {
    let told = console_defaults::setting(SETTING)?;

    read(told.as_deref())
}

pub fn choose(chosen: Measuring) -> Result<(), Never> {
    let Ok(key) = chosen.key();

    console_defaults::set(console_defaults::Setting { key: SETTING, value: key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_chosen_measures_as_the_device_always_has() {
        assert_eq!(read(None), Ok(Measuring::On));
    }

    #[test]
    fn off_is_off_and_on_is_on() {
        assert_eq!(read(Some("off")), Ok(Measuring::Off));
        assert_eq!(read(Some("on")), Ok(Measuring::On));
    }

    #[test]
    fn a_word_that_is_neither_keeps_measuring() {
        assert_eq!(read(Some("no")), Ok(Measuring::On));
    }
}
