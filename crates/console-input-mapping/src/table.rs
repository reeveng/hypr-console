//! The table on this device, and where it is.
//!
//! Two programs write it -- the screen and the card it raises -- so where it
//! is and what is in it are here once rather than in both.
//!
//! Writing the file is the whole of a move now. It used to be half: a button's
//! meaning lived in an InputPlumber profile under `/etc`, so saying a job had
//! moved meant asking root to write the profiles again, and the screen carried
//! the one line in this repository that crossed into `/etc` on someone's say
//! so. The profile no longer decides what a button means -- it says only what
//! each button is -- and the daemon reads this file itself. There is nothing
//! left to make true.

use std::path::PathBuf;
use std::process::Command;

use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_input_bindings::moved::{Tasks, path_in};
use console_input_controller::actions::Table;
use console_input_gamepad::front::{DEVICES, Front, Read, capabilities_command};

use crate::Unmapped;

pub fn at() -> Result<Option<PathBuf>, Never> {
  let Ok(home) = console_core_places::home();

  let home_3 = match home {
    Some(home_2) => home_2,
    None => return Ok(None),
  };

  let Ok(at) = path_in(&home_3);

  Ok(Some(at))
}

pub fn read() -> Result<Tasks, Never> {
  let Ok(table_path) = at();

  let at = match table_path {
    Some(path) => path,
    None => return Ok(Tasks::default()),
  };

  let Ok(held) = console_core_atomic_writes::read(&at);

  let said = match held {
    Stored::Text(said) => said,
    Stored::Absent => return Ok(Tasks::default()),

    Stored::Failed(fault) => {
      eprintln!("{}: reading the button table: {fault}", at.display());
      return Ok(Tasks::default());
    }
  };

  Ok(match Tasks::read(&said) {
    Ok(jobs) => jobs,

    Err(fault) => {
      eprintln!("{}: {fault}", at.display());
      Tasks::default()
    }
  })
}

pub fn write(jobs: &Tasks) -> Result<(), Unmapped> {
  let Ok(at) = at();

  let at_3 = match at {
    Some(at_2) => at_2,
    None => return Err(Unmapped::NoOnes),
  };

  match at_3.parent() {
    Some(holding) => std::fs::create_dir_all(holding)
      .map_err(|fault| Unmapped::Holding(holding.to_path_buf(), fault))?,
    None => {}
  }

  let Ok(written) = jobs.serialize();

  console_core_atomic_writes::whole(&at_3, written.as_bytes()).map_err(Unmapped::Writing)
}

pub fn table() -> Result<Table, Never> {
  let Ok(jobs) = read();

  Table::of(&jobs)
}

pub fn front() -> Result<Front, Never> {
  let Ok(asking) = capabilities_command();
  let Ok(asked) = run_output(&asking);
  let Ok(devices) = devices();

  Front::of(Read {
    said: &asked,
    devices: &devices,
  })
}

fn devices() -> Result<String, Never> {
  Ok(match std::fs::read_to_string(DEVICES) {
    Ok(said) => said,

    Err(fault) => {
      eprintln!("{DEVICES}: reading what this machine's inputs are: {fault}");
      String::new()
    }
  })
}

pub fn run_output(arguments: &[&str]) -> Result<String, Never> {
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Ok(String::new()),
  };

  Ok(match Command::new(program).args(rest).output() {
    Ok(done) => match std::str::from_utf8(&done.stdout) {
      Ok(text) => text.trim().to_string(),
      Err(fault) => {
        eprintln!(
          "{}: command output is not UTF-8: {fault}",
          arguments.join(" ")
        );

        String::new()
      }
    },

    Err(fault) => {
      eprintln!("{}: {fault}", arguments.join(" "));
      String::new()
    }
  })
}
