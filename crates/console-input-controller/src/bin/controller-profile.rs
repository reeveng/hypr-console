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

use console_core_arguments::read_with;
use console_core_never::Never;
use console_input_controller::profile::{
  Buzz, COMMAND, Profile, ProfileEffect, ProfileName, Switch,
};
use console_input_gamepad::devices::Has;
use console_input_gamepad::front;
use console_operating_system_linux::program_runtime::{Clock, Interpreter};
use console_program_contract::Event;

const HID: &str = "/sys/bus/hid/devices";

struct Buzzing;

impl Interpreter for Buzzing {
  type Event = Never;
  type Effect = ProfileEffect;
  type Error = Never;

  fn interpret(
    &mut self,
    acts: &ProfileEffect,
    _clock: &Clock,
  ) -> Result<Vec<Event<Never>>, Never> {
    match acts {
      ProfileEffect::Buzzing(buzz) => {
        let Ok(()) = buzzed(*buzz);
      }
    }

    Ok(Vec::new())
  }
}

fn buzzed(buzz: Buzz) -> Result<(), Never> {
  let Ok(written) = buzz.written();

  let devices = match std::fs::read_dir(HID) {
    Ok(devices) => devices,
    Err(_no_hid_devices) => return Ok(()),
  };

  for device in devices.flatten() {
    let at: PathBuf = device.path().join("touchpad/vibration_enabled");

    #[cfg_attr(
      dylint_lib = "explicit040_no_torn_write",
      expect(
        explicit040_no_torn_write,
        reason = "this sysfs control must be written directly because the kernel owns its directory entry"
      )
    )]
    let mut knob = match std::fs::OpenOptions::new()
      .write(true)
      .truncate(true)
      .open(&at)
    {
      Ok(knob) => knob,
      Err(fault) => {
        let disappeared = matches!(
          fault.kind(),
          std::io::ErrorKind::NotFound | std::io::ErrorKind::IsADirectory
        );

        match disappeared.then_some(()) {
          Some(()) => {}
          None => eprintln!("controller-profile: {}: {fault}", at.display()),
        }

        continue;
      }
    };

    match std::io::Write::write_all(&mut knob, written.as_bytes()) {
      Ok(()) => {}
      Err(fault) => eprintln!("controller-profile: {}: {fault}", at.display()),
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

fn pad() -> Result<Has, Never> {
  let Ok(listed) = listed();
  let Ok(pad) = front::pad(&listed);

  Ok(match pad {
    Some(Has::No) => Has::No,
    Some(Has::Yes) | None => Has::Yes,
  })
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let line = match read_with::<ProfileName, String>(&COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(pad) = pad();
  let Ok(switch) = Switch::of(&line, pad);
  let Ok(code) = console_operating_system_linux::program_runtime::run::<Profile, Buzzing>(
    COMMAND.name,
    &switch,
    &mut Buzzing,
  );

  code
}
