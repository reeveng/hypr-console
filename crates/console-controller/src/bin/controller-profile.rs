//! Switch the Legion Go controller between desktop and gamepad behaviour.
//!
//! Everything it decides is in `console_controller::profile`, where sixty
//! seconds of waiting for a bus can be pressed in no time at all. What is here
//! is the one thing that program cannot say: how many touchpads this machine
//! has and what the driver called them.

use std::path::PathBuf;
use std::process::ExitCode;

use console_controller::profile::{Buzz, Its, Profile};
use console_never::Never;
use console_program_contract::{Argv, Word};
use console_program_runtime::Carrying;

const DEVICES: &str = "/sys/bus/hid/devices";

struct Buzzing;

impl Carrying for Buzzing {
    type Hears = Never;
    type Does = Its;

    fn its(&mut self, doing: &Its) -> Vec<Word<Never>> {
        match doing {
            Its::Buzzing(buzz) => {
                let Ok(()) = buzzed(*buzz);
            },
        }

        Vec::new()
    }
}

fn buzzed(buzz: Buzz) -> Result<(), Never> {
    let Ok(written) = buzz.written();

    let devices = match std::fs::read_dir(DEVICES) {
        Ok(devices) => devices,
        Err(_) => return Ok(()),
    };

    for device in devices.flatten() {
        let at: PathBuf = device.path().join("touchpad/vibration_enabled");

        match at.is_file() {
            true => match std::fs::write(&at, written) {
                Ok(()) => {},
                Err(fault) => eprintln!("controller-profile: {}: {fault}", at.display()),
            },
            false => {},
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let said: Vec<&str> = words.iter().map(String::as_str).collect();

    let Ok(argv) = Argv::of(&said);
    let Ok(code) =
        console_program_runtime::run::<Profile, Buzzing>("controller-profile", &argv, &mut Buzzing);

    code
}
