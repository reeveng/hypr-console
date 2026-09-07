//! The table on this device, and where it is.
//!
//! Two programs write it -- the screen and the card it raises -- so where it
//! is and what is in it are here once rather than in both.
//!
//! Writing the file is the whole of a move now. It used to be half: a button's
//! meaning lived in an InputPlumber profile under `/etc`, so saying a job had
//! moved meant asking root to write the profiles again, and the screen carried
//! the one line in this repository that crossed into `/etc` on somebody's say
//! so. The profile no longer decides what a button means -- it says only what
//! each button is -- and the daemon reads this file itself. There is nothing
//! left to make true.

use std::path::PathBuf;
use std::process::Command;

use console_input_controller::means::Table;
use console_input_gamepad::front::{DEVICES, Front, asking};
use console_input_gamepad::jobs::{Jobs, path_in};
use console_core_never::Never;

pub fn at() -> Result<PathBuf, Never> {
    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(std::env::VarError::NotPresent) => String::new(),

        Err(fault) => {
            eprintln!("HOME, looking for the button table: {fault}");
            String::new()
        }
    };

    path_in(&home)
}

pub fn read() -> Result<Jobs, Never> {
    let Ok(at) = at();

    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return Ok(Jobs::default()),

        Err(fault) => {
            eprintln!("{}: reading the button table: {fault}", at.display());
            return Ok(Jobs::default());
        }
    };

    Ok(match Jobs::read(&said) {
        Ok(jobs) => jobs,

        Err(fault) => {
            eprintln!("{}: {fault}", at.display());
            Jobs::default()
        }
    })
}

pub fn write(jobs: &Jobs) -> Result<(), String> {
    let Ok(at) = at();

    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{}: {fault}", holding.display()))?,
        None => {}
    }

    let Ok(written) = jobs.written();

    std::fs::write(&at, written).map_err(|fault| format!("{}: {fault}", at.display()))
}

pub fn table() -> Result<Table, Never> {
    let Ok(jobs) = read();

    Table::of(&jobs)
}

pub fn front() -> Result<Front, Never> {
    let Ok(asking) = asking();
    let Ok(asked) = said(&asking);
    let Ok(devices) = devices();

    Front::of(&asked, &devices)
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

pub fn said(argv: &[&str]) -> Result<String, Never> {
    let Some((program, rest)) = argv.split_first() else { return Ok(String::new()) };

    Ok(match Command::new(program).args(rest).output() {
        Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),

        Err(fault) => {
            eprintln!("{}: {fault}", argv.join(" "));
            String::new()
        }
    })
}
