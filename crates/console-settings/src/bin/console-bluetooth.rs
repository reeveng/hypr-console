//! Introduce a device to this machine.
//!
//!     console-bluetooth introduce AA:BB:CC:DD:EE:FF
//!
//! What to ask and how to read the answer is `console_settings::introducing`,
//! which can be asked without a radio. This is the part that needs one, and the
//! notification when it did not work.
//!
//! It exists because a panel row runs one program with one list of words and
//! meeting a device is three of those. The row could have been three rows and
//! was not: someone pressing a keyboard's name means the keyboard to work, and
//! a machine that made them press Pair, then Trust, then Connect would be
//! asking them to know what bluez calls the halves of that.
//!
//! Silent when it worked. The row it was pressed from redraws into the device
//! being there, which is the whole of what anyone wanted to see; a notification
//! saying the same thing again is a card to put away. Failure is the other way
//! round -- the row goes back to reading the way it did, which on its own is
//! indistinguishable from a press that never landed -- so what bluez said is
//! carried out to where someone can read it.

use console_core_arguments::{Operands, ValidationError, read_with};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::{Notification, Content, raise};
use console_settings::introducing::{self, Bluetooth, Reply, Went};

fn bluetoothctl(arguments: &[String]) -> Result<String, Never> {
    let mut asking = Program::Bluetoothctl.command()?;

    let said = match asking.args(arguments).output() {
        Ok(message) => message,
        Err(_would_not_start) => return Ok(String::new()),
    };

    Ok(format!(
        "{}{}",
        String::from_utf8_lossy(&said.stdout),
        String::from_utf8_lossy(&said.stderr)
    ))
}

fn would_not(says: &str, said: Reply<'_>) -> Result<(), Never> {
    let words = introducing::would_not(says, said)?;
    let notification = Notification::new(Content { summary: &words, body: "" })?;
    let notification = notification.lasting(6000)?;
    let Ok(_) = raise(&notification);

    Ok(())
}

fn introduce(address: &str) -> Result<Went, Never> {
    let pairing = introducing::pairing(address)?;
    let said = bluetoothctl(&pairing)?;
    let paired = introducing::paired(&said)?;

    match paired {
        Went::Not => {
            let Ok(()) = would_not(&format!("{address} would not pair:"), Reply(&said));

            return Ok(Went::Not);
        }
        Went::Well => {},
    }

    let trusting = introducing::trusting(address)?;
    let Ok(_) = bluetoothctl(&trusting);

    let joining = introducing::joining(address)?;
    let said = bluetoothctl(&joining)?;
    let joined = introducing::joined(&said)?;

    match joined {
        Went::Not => {
            let Ok(()) = would_not(
                &format!("{address} paired but would not connect:"),
                Reply(&said),
            );
        }
        Went::Well => {},
    }

    Ok(joined)
}

const ADDRESS: [&str; 1] = ["ADDRESS"];

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
    name: "console-bluetooth",
    about: "introduce a device to this machine",
    flags: &[],
    operands: Operands::Named(&ADDRESS),
};

fn address(words: &[String]) -> Result<String, ValidationError> {
    let read = read_with::<Bluetooth, String>(&COMMAND, words);
    let line = read?;
    let required = line.require_subcommand();
    let introducing = required?;

    match introducing {
        Bluetooth::Introduce => {
            let operands = line.exactly(ADDRESS);
            let [address] = operands?;

            Ok(address.clone())
        }
    }
}

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let address = match address(&words) {
        Ok(address) => address,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return std::process::ExitCode::from(code);
        }
    };

    let Ok(went) = introduce(&address);

    match went {
        Went::Well => std::process::ExitCode::SUCCESS,
        Went::Not => std::process::ExitCode::from(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_arguments::Reason;

    fn words(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|word| (*word).to_string()).collect())
    }

    #[test]
    fn nothing_but_the_word_and_one_address_is_a_press_this_understands() {
        let Ok(pressed) = words(&["introduce", "AA:BB:CC:DD:EE:FF"]);
        let Ok(bare) = words(&["introduce"]);
        let Ok(forget) = words(&["forget", "AA:BB:CC:DD:EE:FF"]);
        let Ok(two) = words(&["introduce", "AA:BB:CC:DD:EE:FF", "11:22:33:44:55:66"]);
        let Ok(nothing) = words(&[]);

        assert_eq!(address(&pressed), Ok("AA:BB:CC:DD:EE:FF".to_string()));
        assert_eq!(address(&bare).map_err(|refusal| refusal.reason), Err(Reason::MissingOperands(vec!["ADDRESS"])));
        assert_eq!(address(&forget).map_err(|refusal| refusal.reason), Err(Reason::NoSuchSubcommand("forget".to_string())));
        assert_eq!(address(&two).map_err(|refusal| refusal.reason), Err(Reason::ExtraArgument("11:22:33:44:55:66".to_string())));
        assert_eq!(address(&nothing).map_err(|refusal| refusal.reason), Err(Reason::MissingSubcommand));
    }
}
