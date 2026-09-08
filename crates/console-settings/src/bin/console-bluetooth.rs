//! Introduce a device to this machine.
//!
//!     console-bluetooth introduce AA:BB:CC:DD:EE:FF
//!
//! What to ask and how to read the answer is `console_settings::introducing`,
//! which can be asked without a radio. This is the part that needs one, and the
//! notice when it did not work.
//!
//! It exists because a panel row runs one program with one list of words and
//! meeting a device is three of those. The row could have been three rows and
//! was not: somebody pressing a keyboard's name means the keyboard to work, and
//! a machine that made them press Pair, then Trust, then Connect would be
//! asking them to know what bluez calls the halves of that.
//!
//! Silent when it worked. The row it was pressed from redraws into the device
//! being there, which is the whole of what anybody wanted to see; a notice
//! saying the same thing again is a card to put away. Failure is the other way
//! round -- the row goes back to reading the way it did, which on its own is
//! indistinguishable from a press that never landed -- so what bluez said is
//! carried out to where somebody can read it.

use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::{Notice, raise};
use console_settings::introducing::{self, Asked, INTRODUCE, Went};

fn bluetoothctl(argv: &[String]) -> Result<String, Never> {
    let mut asking = Program::Bluetoothctl.command()?;

    let said = match asking.args(argv).output() {
        Ok(said) => said,
        Err(_) => return Ok(String::new()),
    };

    Ok(format!(
        "{}{}",
        String::from_utf8_lossy(&said.stdout),
        String::from_utf8_lossy(&said.stderr)
    ))
}

fn would_not(says: &str, said: &str) -> Result<(), Never> {
    let words = introducing::would_not(says, said)?;
    let notice = Notice::new(&words, "")?;
    let notice = notice.lasting(6000)?;
    let Ok(_) = raise(&notice);

    Ok(())
}

fn introduce(address: &str) -> Result<Went, Never> {
    let pairing = introducing::pairing(address)?;
    let said = bluetoothctl(&pairing)?;
    let paired = introducing::paired(&said)?;

    match paired {
        Went::Not => {
            let Ok(()) = would_not(&format!("{address} would not pair:"), &said);

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
            let Ok(()) = would_not(&format!("{address} paired but would not connect:"), &said);
        }
        Went::Well => {},
    }

    Ok(joined)
}

fn said_how() -> Result<std::process::ExitCode, Never> {
    eprintln!("usage: console-bluetooth {INTRODUCE} ADDRESS");

    Ok(std::process::ExitCode::from(2))
}

fn main() -> std::process::ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Ok(asked) = introducing::asked(&words);

    let address = match asked {
        Asked::Introduce(address) => address,
        Asked::Nothing => {
            let Ok(how) = said_how();

            return how;
        }
    };

    let Ok(went) = introduce(&address);

    match went {
        Went::Well => std::process::ExitCode::SUCCESS,
        Went::Not => std::process::ExitCode::from(1),
    }
}
