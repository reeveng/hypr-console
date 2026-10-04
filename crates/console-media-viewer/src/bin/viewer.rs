//! A picture or a film, asked for.
//!
//!     viewer FILE-OR-FOLDER
//!  What it shows and how it is driven is `console_media_viewer::card`'s own
//! head. This is the program the files open and a `.desktop` file names, and it
//! is an app, drawn by this process as a window on a workspace of its own. It
//! can answer a press by not opening at all, because the file it was handed may
//! be neither a picture nor a film -- and that has to be decided before the
//! screen is taken. Whoever is looking gets the folder read twice for it, once
//! to find out there is something to show and once to show it.
//!
//! A folder is not one of those and never was. An empty one opens and says it
//! is empty, because the entry that opens the pictures folder is on the home
//! screen, and a press there that draws nothing cannot be told from a crash.

use std::process::ExitCode;

use console_core_arguments::{Command, Operands};
use console_panel::card::{App, open_app, refusal};
use console_media_viewer::card::Worth;

const COMMAND: Command = Command {
    name: console_media_viewer::WHO,
    about: "a picture or a film; with nothing, the pictures folder",
    flags: &[],
    operands: Operands::Optional("FILE-OR-FOLDER"),
};

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(refusing) = refusal(&COMMAND, &asked);

    match refusing {
        Some(code) => return code,
        None => {},
    }

    let Ok(worth) = console_media_viewer::card::worth_opening(&asked);

    match worth {
        Worth::None => ExitCode::SUCCESS,
        Worth::Opening => {
            let Ok(code) = open_app(&asked, App { who: console_media_viewer::WHO, command: COMMAND, card: console_media_viewer::card });

            code
        }
    }
}
