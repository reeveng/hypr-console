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

use console_controller::means::Table;
use console_pad::front::{DEVICES, Front, asking};
use console_pad::jobs::{Jobs, path_in};

pub fn at() -> PathBuf {
    path_in(&std::env::var("HOME").unwrap_or_default())
}

/// What has been said about this device's buttons, or nothing.
///
/// A file that will not read is read as no file at all. The alternative is a
/// screen that will not open on the one machine where somebody has typed into
/// this file by hand and got it wrong, which is the machine that most needs a
/// screen to put it right.
pub fn read() -> Jobs {
    std::fs::read_to_string(at())
        .ok()
        .and_then(|said| Jobs::read(&said).ok())
        .unwrap_or_default()
}

/// Say it, and leave room for it if this is the first time anybody has.
pub fn write(jobs: &Jobs) -> Result<(), String> {
    let at = at();
    if let Some(holding) = at.parent() {
        std::fs::create_dir_all(holding).map_err(|fault| format!("{}: {fault}", holding.display()))?;
    }
    std::fs::write(&at, jobs.written()).map_err(|fault| format!("{}: {fault}", at.display()))
}

/// Every job and what plays it here: this desktop's own answers, with
/// whatever the person whose desktop it is has said over the top.
pub fn table() -> Table {
    Table::of(&read())
}

/// What this machine says the front of it is.
pub fn front() -> Front {
    Front::of(&said(&asking()), &std::fs::read_to_string(DEVICES).unwrap_or_default())
}

/// A command, and what it said.
pub fn said(argv: &[&str]) -> String {
    Command::new(argv[0])
        .args(&argv[1..])
        .output()
        .map(|done| String::from_utf8_lossy(&done.stdout).trim().to_string())
        .unwrap_or_default()
}
