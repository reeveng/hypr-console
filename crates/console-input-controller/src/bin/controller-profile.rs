//! Switch the Legion Go controller between desktop and gamepad behavior.
//!
//! Everything it decides is in `console_input_controller::profile`, where sixty
//! seconds of waiting for a bus can be pressed in no time at all. What is here
//! is what that program cannot say: how many touchpads this machine has and
//! what the driver called them, and whether there is a pad on this machine at
//! all. The second is read off the kernel's own list of devices and handed in
//! as a word, because a list that cannot be read is not a machine without a
//! pad -- so a kernel that says nothing is taken to have one, and the wait
//! happens as it always did.

use std::path::PathBuf;
use std::process::ExitCode;

use console_input_controller::profile::{Buzz, ProfileEffect, PAD, Profile};
use console_input_gamepad::devices::Has;
use console_input_gamepad::front;
use console_core_never::Never;
use console_program_contract::{Arguments, Event};
use console_program_runtime::Interpreter;

const HID: &str = "/sys/bus/hid/devices";

struct Buzzing;

impl Interpreter for Buzzing {
    type Event = Never;
    type Effect = ProfileEffect;

    fn interpret(&mut self, acts: &ProfileEffect) -> Vec<Event<Never>> {
        match acts {
            ProfileEffect::Buzzing(buzz) => {
                let Ok(()) = buzzed(*buzz);
            },
        }

        Vec::new()
    }
}

fn buzzed(buzz: Buzz) -> Result<(), Never> {
    let Ok(written) = buzz.written();

    let devices = match std::fs::read_dir(HID) {
        Ok(devices) => devices,
        Err(_) => return Ok(()),
    };

    for device in devices.flatten() {
        let at: PathBuf = device.path().join("touchpad/vibration_enabled");

        match at.is_file() {
            #[cfg_attr(
                dylint_lib = "explicit040_no_torn_write",
                allow(
                    explicit040_no_torn_write,
                    reason = "the touchpad buzz is a kernel knob rather than a file: there is nothing beside it to write and nothing to rename over"
                )
            )]
            true => match std::fs::write(&at, written) {
                Ok(()) => {},
                Err(fault) => eprintln!("controller-profile: {}: {fault}", at.display()),
            },
            false => {},
        }
    }

    Ok(())
}

fn listed() -> Result<String, Never> {
    Ok(match std::fs::read_to_string(front::DEVICES) {
        Ok(said) => said,

        Err(fault) => {
            eprintln!("controller-profile: {}: {fault}", front::DEVICES);
            String::new()
        }
    })
}

fn given() -> Result<Vec<String>, Never> {
    let mut words: Vec<String> = std::env::args().skip(1).collect();
    let Ok(listed) = listed();
    let Ok(pad) = front::pad(&listed);

    match pad {
        Some(Has::No) => {},
        Some(Has::Yes) | None => words.push(PAD.to_string()),
    }

    Ok(words)
}

fn main() -> ExitCode {
    let Ok(words) = given();
    let said: Vec<&str> = words.iter().map(String::as_str).collect();

    let Ok(arguments) = Arguments::of(&said);
    let Ok(code) =
        console_program_runtime::run::<Profile, Buzzing>("controller-profile", &arguments, &mut Buzzing);

    code
}
