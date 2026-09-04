//! How big everything on the screen is.
//!
//!     console-scale                 which size the screen is standing at
//!     console-scale smaller|normal|bigger
//!                                   put it there, and remember it
//!     console-scale apply           wear what was remembered
//!
//! `apply` is the one the session runs. The compositor's own file declares the
//! size this device is set up as and is this repository's byte for byte, so a
//! machine standing somewhere else says so in a file of its own and this puts
//! it back on at every login. A desktop that forgot the size it was set to at
//! every reboot would be a setting nobody could rely on having made.
//!
//! What `hyprctl` is told is `eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."* -- the same trap `docs/screen.md`
//! describes for `dispatch`, where the command every example on the internet
//! gives comes back with a complaint and the only symptom is a setting that
//! does nothing.

use std::process::{Command, ExitCode};

use console_settings::size::{self, Size};

/// The bar, which has to be told how wide the screen became.
///
/// Its apply strip is a gradient in a box, and the box is a number of logical
/// pixels. Restarted rather than reloaded: waybar watches its own stylesheet
/// and not the one that stylesheet imports, so a bar left running would be
/// wearing the width of the size that was.
const BAR: &str = "console-bar.service";

/// The home screen, which has to be stood back on the screen there now is.
///
/// It is not told the density -- the grid takes whatever screen its surface is
/// given -- but the surface it has is the one it was mapped onto, and a
/// density changed under a running layer surface leaves it wearing the logical
/// screen that was. Restarted in the same transaction as the bar, so one
/// press is one round of the desktop coming back at the size that was asked
/// for.
const HOME: &str = "console-home.service";

fn main() -> ExitCode {
    let word = std::env::args().nth(1).unwrap_or_default();

    let Ok(home) = std::env::var("HOME") else {
        eprintln!("console-scale: no HOME, so there is nobody to remember for");
        return ExitCode::FAILURE;
    };

    let at = size::at(&home);
    let written = match std::fs::read_to_string(&at) {
        Ok(said) => said,

        // No file is a machine that has never been told, which is what the
        // default below is for. A file that is there and will not open is a
        // different thing, and it is about to be written over.
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),

        Err(fault) => {
            eprintln!("console-scale: {}: {fault}", at.display());
            String::new()
        }
    };

    // The one road that is not a press. It runs at login, before the bar is
    // up, and its job is to leave the machine wearing what it was left wearing.
    if word == "apply" {
        return applied(&home, Size::of(&written));
    }

    let wanted = match word.as_str() {
        // What the compositor says, not what was written down. The file is what
        // was last chosen and the compositor is what is on the screen, and it is
        // the screen a person is asking about.
        "" => {
            let said = asked();

            match size::standing(&said) {
                Some(size) => println!("{}", size.written()),
                // A density that is none of the three is still a density, and
                // saying the number is more use than saying nothing.
                None => match size::scale_of(&said) {
                    Some(scale) => println!("{scale}"),
                    None => {
                        eprintln!("console-scale: the compositor said nothing about a screen");
                        return ExitCode::FAILURE;
                    }
                },
            }

            return ExitCode::SUCCESS;
        }
        said => match Size::of(said) {
            Some(size) => size,
            None => {
                eprintln!("usage: console-scale [smaller|normal|bigger|apply]");
                return ExitCode::from(2);
            }
        },
    };

    let Ok(screen) = console_screen::declared() else {
        eprintln!("console-scale: this build carries no readable screen to change");
        return ExitCode::FAILURE;
    };

    // Written first, and then worn, for the reason console-warm writes first:
    // what is remembered has to survive the part that can fail. A compositor
    // that took the new size and a file that still says the old one is a
    // machine that changes size again at the next login.
    if let Some(holding) = at.parent() {
        if let Err(fault) = std::fs::create_dir_all(holding) {
            eprintln!("console-scale: {}: {fault}, so nothing was changed", holding.display());

            return ExitCode::FAILURE;
        }

        if let Err(fault) = std::fs::write(&at, format!("{}\n", wanted.written())) {
            eprintln!("console-scale: {}: {fault}, so nothing was changed", at.display());

            return ExitCode::FAILURE;
        }
    }

    // Before the compositor is told anything, so the width is already right in
    // the file the bar reads on its way back up.
    write_the_bar(&home, &screen, wanted);

    if let Some(why) = refused(&screen, wanted) {
        eprintln!("console-scale: {why}");
        return ExitCode::FAILURE;
    }

    // The surfaces last, after the screen has moved: one restarted before it
    // would come back onto the size that was and have to be restarted again.
    // Their failure is not this program's -- the screen is the size it was
    // asked to be by now, and a surface that would not come back is a missing
    // surface, which says so in the journal on its own.
    let _ = Command::new("systemctl").args(["--user", "restart", BAR, HOME]).status();
    ExitCode::SUCCESS
}

/// Login: wear what was left on, and leave the bar a width whether or not
/// anything was.
///
/// The width is written even on a machine nobody has ever changed, because the
/// bar's stylesheet imports it and GTK complains at every start about a file
/// that is not there. Written to what the compositor's own file declares, which
/// is the size that machine is about to come up at anyway.
///
/// The bar is not restarted. It has not started yet -- this runs from
/// `session-start`, ahead of the target the bar is under -- and restarting a
/// unit that is not running would start it early, before the screen it is to be
/// drawn on has stopped changing size.
///
/// **It never fails.** `session-start` runs its steps in order and stops at the
/// first that will not run, and the step after this one is the whole desktop.
/// A compositor that would not take a density is a screen at the wrong size; a
/// non-zero exit here would be a machine with no bar, no controller daemon and
/// no keyboard, over a preference. So what went wrong goes to the journal and
/// the session goes on.
fn applied(home: &str, written: Option<Size>) -> ExitCode {
    let Ok(screen) = console_screen::declared() else {
        eprintln!("console-scale: this build carries no readable screen to put back on");
        return ExitCode::SUCCESS;
    };

    write_the_bar(home, &screen, written.unwrap_or(Size::Normal));

    // Nothing written down is the ordinary case, and there is nothing to put
    // back on: the compositor's file already declares that size and has already
    // been read.
    if let Some(size) = written
        && let Some(why) = refused(&screen, size)
    {
        eprintln!("console-scale: {why}");
    }

    ExitCode::SUCCESS
}

/// Hand the compositor the screen. Nothing if it took it, and what it said if
/// it did not.
///
/// The complaint is returned rather than printed, because the two roads that
/// call this do different things with it: a press reports and stops, and the
/// login reports and carries on into the desktop.
fn refused(screen: &console_screen::Screen, size: Size) -> Option<String> {
    match Command::new("hyprctl").args(["eval", &size::lua(screen, size.scale())]).output() {
        // hyprctl is cheerful about a line the compositor refused -- it answers
        // `ok` when the line was taken and prints the complaint otherwise, with
        // the same exit code either way -- so what it printed is read as well as
        // how it exited.
        Ok(said) => {
            let printed = String::from_utf8_lossy(&said.stdout);

            match said.status.success() && !printed.to_lowercase().contains("error") {
                true => None,
                false => Some(format!(
                    "the compositor would not take it: {}{}",
                    printed.trim(),
                    String::from_utf8_lossy(&said.stderr).trim()
                )),
            }
        }
        Err(why) => Some(format!("no hyprctl to run: {why}")),
    }
}

/// How wide the screen became, where the bar's stylesheet imports it from.
fn write_the_bar(home: &str, screen: &console_screen::Screen, size: Size) {
    let at = size::bar_at(home);

    let Some(holding) = at.parent() else { return };

    if let Err(fault) = std::fs::create_dir_all(holding) {
        eprintln!("console-scale: {}: {fault}", holding.display());

        return;
    }

    if let Err(fault) = std::fs::write(&at, console_screen::bar_css(screen, size.scale())) {
        eprintln!("console-scale: {}: {fault}", at.display());
    }
}

/// What the compositor says about its screens.
fn asked() -> String {
    match Command::new("hyprctl").args(["monitors", "-j"]).output() {
        Ok(said) => String::from_utf8_lossy(&said.stdout).to_string(),

        Err(fault) => {
            eprintln!("console-scale: hyprctl: asking what the screens are: {fault}");
            String::new()
        }
    }
}
