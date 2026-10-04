//! Meeting a device for the first time: what to ask, in what order, and how to
//! read what came back.
//!
//! Three things have to be true before a keyboard someone has just bought
//! works twice. Pairing exchanges the keys. Trusting is what lets it back in
//! after a reboot without anyone being asked again. Connecting is what makes
//! it work now. bluez has a separate word for each, a panel row runs one
//! program with one list of words, and the tab used to offer only the third --
//! which is the one word bluez refuses for anything it has not been introduced
//! to, so the device that most needed the tab was the device the tab could do
//! nothing with.
//!
//! The order is not a preference. Trusting something that would not pair is a
//! promise about a machine that is not there, and connecting it asks bluez for
//! a road it holds no keys for, so a step that did not work stops the rest.
//!
//! Pairing is the one that waits, and it is told how long to. Every other word
//! here comes back the moment bluez has an answer, but `pair` and a `connect`
//! aimed at something out of range sit there for ever -- non-interactively,
//! with nothing on stdin, no tty and no timer of their own -- and a panel that
//! spawns one of those on every press leaves one behind on every press. That
//! is the `pactl subscribe` fault again, wearing different clothes.
//!
//! Nothing here reads an exit status, because there is not one to read:
//! bluetoothctl says "Device not available" and leaves with nothing wrong, and
//! it does the same when it worked. What it says is the only answer it gives,
//! so what it says is what is read.

use console_core_arguments::Subcommand;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;

pub const PATIENCE: &str = "20";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    Well,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Bluetooth {
    #[words(word = "introduce", about = "pair with the device at ADDRESS, trust it and connect it")]
    Introduce,
}

impl Subcommand for Bluetooth {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(Bluetooth::VARIANTS.iter().copied())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        self.word()
    }

    fn about(self) -> Result<&'static str, Never> {
        Bluetooth::about(self)
    }
}

pub fn pairing(address: &str) -> Result<Vec<String>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    Ok(vec![
        bluetoothctl.to_string(),
        "--timeout".to_string(),
        PATIENCE.to_string(),
        "pair".to_string(),
        address.to_string(),
    ])
}

pub fn trusting(address: &str) -> Result<Vec<String>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    Ok(vec![bluetoothctl.to_string(), "trust".to_string(), address.to_string()])
}

pub fn joining(address: &str) -> Result<Vec<String>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    Ok(vec![bluetoothctl.to_string(), "connect".to_string(), address.to_string()])
}

pub fn paired(said: &str) -> Result<Went, Never> {
    let done = said.contains("Pairing successful") || said.contains("AlreadyExists");

    Ok(match done {
        true => Went::Well,
        false => Went::Not,
    })
}

pub fn joined(said: &str) -> Result<Went, Never> {
    let done = said.contains("Connection successful") || said.contains("AlreadyConnected");

    Ok(match done {
        true => Went::Well,
        false => Went::Not,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply<'a>(pub &'a str);

pub fn would_not(says: &str, said: Reply<'_>) -> Result<String, Never> {
    let last = said.0.lines().map(str::trim).rfind(|line| !line.is_empty());

    Ok(match last {
        Some(last) => format!("{says} {last}"),
        None => format!("{says} and said nothing about why"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(arguments: &[String]) -> Result<Vec<&str>, Never> {
        Ok(arguments.iter().map(String::as_str).collect())
    }

    #[test]
    fn the_one_that_can_wait_for_ever_is_the_one_told_how_long() {
        let Ok(pairing) = pairing("AA:BB:CC:DD:EE:FF");

        assert_eq!(words(&pairing), Ok(vec!["bluetoothctl", "--timeout", PATIENCE, "pair", "AA:BB:CC:DD:EE:FF"]));

        let Ok(trusting) = trusting("AA:BB:CC:DD:EE:FF");

        assert_eq!(words(&trusting), Ok(vec!["bluetoothctl", "trust", "AA:BB:CC:DD:EE:FF"]));
    }

    #[test]
    fn what_bluez_said_is_the_whole_of_whether_it_worked() {
        assert_eq!(paired("Attempting to pair\nPairing successful\n"), Ok(Went::Well));
        assert_eq!(paired("Failed to pair: org.bluez.Error.AuthenticationFailed"), Ok(Went::Not));
        assert_eq!(paired("Device AA:BB:CC:DD:EE:FF not available"), Ok(Went::Not));
        assert_eq!(paired(""), Ok(Went::Not));
        assert_eq!(joined("Connection successful"), Ok(Went::Well));
        assert_eq!(joined("Failed to connect: org.bluez.Error.NotReady"), Ok(Went::Not));
    }

    #[test]
    fn a_device_already_known_has_arrived_where_the_press_meant_to_put_it() {
        assert_eq!(paired("Failed to pair: org.bluez.Error.AlreadyExists"), Ok(Went::Well));
        assert_eq!(joined("Failed to connect: org.bluez.Error.AlreadyConnected"), Ok(Went::Well));
    }


    #[test]
    fn what_is_said_afterwards_is_the_last_thing_bluez_said() {
        assert_eq!(
            would_not(
                "Blue Keys would not pair:",
                Reply("Attempting to pair\nDevice AA not available\n")
            ),
            Ok("Blue Keys would not pair: Device AA not available".to_string())
        );
        assert_eq!(
            would_not("Blue Keys would not pair:", Reply("   \n")),
            Ok("Blue Keys would not pair: and said nothing about why".to_string())
        );
    }
}
