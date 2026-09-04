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
    // Unset is ordinary and says nothing: this is asked from programs a
    // systemd unit starts, and the empty string is the path that has always
    // been built from it. A name set to something that is not text is somebody
    // pointing at a home and missing, and that arrived here as the same
    // silence.
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

/// What has been said about this device's buttons, or nothing.
///
/// A file that will not read is still read as no file at all. The alternative
/// is a screen that will not open on the one machine where somebody has typed
/// into this file by hand and got it wrong, which is the machine that most
/// needs a screen to put it right.
///
/// What has changed is that it now says which. No file is ordinary and stays
/// quiet; a file that is there and will not parse puts the reason in the
/// journal, because that machine is exactly the one where somebody is about to
/// ask why the screen shows none of what they wrote.
pub fn read() -> Jobs {
    let at = at();

    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return Jobs::default(),

        Err(fault) => {
            eprintln!("{}: reading the button table: {fault}", at.display());
            return Jobs::default();
        }
    };

    match Jobs::read(&said) {
        Ok(jobs) => jobs,

        Err(fault) => {
            eprintln!("{}: {fault}", at.display());
            Jobs::default()
        }
    }
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
    Front::of(&said(&asking()), &devices())
}

/// What the kernel lists as the input devices on this machine.
///
/// Nothing to read leaves `Front` knowing nothing about the front of the
/// machine, which is the right answer on a machine that lists no inputs. A
/// file that is there and will not read is not that, and used to arrive as the
/// same empty string.
fn devices() -> String {
    match std::fs::read_to_string(DEVICES) {
        Ok(said) => said,

        Err(fault) => {
            eprintln!("{DEVICES}: reading what this machine's inputs are: {fault}");
            String::new()
        }
    }
}

/// A command, and what it said.
///
/// A command that will not run answers with nothing, because every caller here
/// is asking the machine to describe itself and can carry on not knowing. It
/// says which command it was first: a missing program and a program that
/// answered nothing are the same string otherwise.
pub fn said(argv: &[&str]) -> String {
    match Command::new(argv[0]).args(&argv[1..]).output() {
        Ok(done) => String::from_utf8_lossy(&done.stdout).trim().to_string(),

        Err(fault) => {
            eprintln!("{}: {fault}", argv.join(" "));
            String::new()
        }
    }
}
