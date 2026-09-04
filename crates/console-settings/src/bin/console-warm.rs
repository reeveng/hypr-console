//! Whether the screen follows the clock, and the curve it follows.
//!
//!     console-warm            follow the clock, or stop following it
//!     console-warm get        which way the switch is standing
//!     console-warm wanted     nothing said; the exit code is the answer
//!     console-warm curve      print the config the daemon reads
//!
//! The colour itself is `hyprsunset`'s, out of a config written from
//! `console_settings::warm`. Nothing here sends it a temperature: the whole
//! curve is in the file it reads at startup, and a temperature sent afterwards
//! is undone by the next profile. So the switch is the daemon running or not
//! running, which `console-warm.service` asks about in `ExecCondition=` and
//! this restarts when the answer changes.
//!
//! `wanted` is that question and nothing else, so it is the exit code rather
//! than a word: systemd reads the code, and a program run before every start of
//! a unit should print nothing into the journal for the ordinary case.
//!
//! `curve` is how `files/home/@user@/.config/hypr/hyprsunset.conf` is made. It
//! is not something the device runs; it is run here, into the tree, and a test
//! holds the file to it.

use std::process::{Command, ExitCode};

use console_settings::warm::{Wanted, Warmth, at, config};

/// The unit that holds the daemon, which is the thing being switched.
const UNIT: &str = "console-warm.service";

fn main() -> ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();

    // Asked before anything is read, because it is the one road that does not
    // need a home: it is run by systemd, once per start of the unit.
    if word == "curve" {
        print!("{}", config());
        return ExitCode::SUCCESS;
    }

    let Ok(home) = std::env::var("HOME") else {
        eprintln!("console-warm: no HOME, so there is nobody to remember for");
        return ExitCode::FAILURE;
    };

    let at = at(&home);
    let said = match std::fs::read_to_string(&at) {
        Ok(said) => said,

        // No file is a machine that has never been told, which is what the
        // default below is for. A file that is there and will not open is a
        // different thing, and it is about to be written over.
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("console-warm: {}: {fault}", at.display());
            String::new()
        }
    };

    let standing = Warmth::read(&said);

    let wanted = match word.as_str() {
        "get" => {
            println!("{}", standing.written().trim());
            return ExitCode::SUCCESS;
        }
        // Nothing said. The unit is asking whether to start at all, and a
        // condition that printed a line would print it on every boot.
        "wanted" => {
            return match standing.wanted() {
                Wanted::Running => ExitCode::SUCCESS,
                Wanted::Off => ExitCode::FAILURE,
            };
        }
        "" => standing.other(),
        _ => {
            eprintln!("usage: console-warm [get|wanted|curve]");
            return ExitCode::from(2);
        }
    };

    // Written first, and then the unit is restarted onto it.
    //
    // The other way round -- restart, then remember -- cannot work here: what
    // the unit does when it starts is ask this file, so a restart before the
    // writing is a restart that reads the old answer and lands on the state
    // that was just pressed away.
    //
    // It also means the file is the whole of the memory. There is no second
    // question anybody can ask the daemon: hyprsunset does not report whether
    // it is wearing a profile or where in the curve it is, and the panel draws
    // the switch out of this file for exactly that reason.
    if let Some(holding) = at.parent() {
        if let Err(fault) = std::fs::create_dir_all(holding) {
            eprintln!("console-warm: {}: {fault}, so nothing was changed", holding.display());

            return ExitCode::FAILURE;
        }

        if let Err(fault) = std::fs::write(&at, wanted.written()) {
            eprintln!("console-warm: {}: {fault}, so nothing was changed", at.display());

            return ExitCode::FAILURE;
        }
    }

    // Restarted rather than started or stopped, because one word covers both:
    // the condition decides which of the two a restart turns out to be, and
    // this does not have to know.
    let done = Command::new("systemctl").args(["--user", "restart", UNIT]).status();

    if !done.is_ok_and(|how| how.success()) {
        eprintln!(
            "console-warm: {UNIT} would not restart, so the screen is still what it was. \
             It is written down, and the next start of the desktop will wear it."
        );
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
