//! Leave the player going: the library, in any order, round for ever.
//!
//! Run right after a song is chosen, and for one reason: choosing a song and
//! being handed silence four minutes later is a machine that stops in the
//! middle of the evening and waits to be asked again. A handheld that is being
//! carried about is the last place anyone wants to go back to a panel to hear
//! a second song, so what one press means here is *play*, not *play this*.
//!
//! Which is what the song on the end of this line is for. A player told to
//! open one song used to hold one song, so next and previous had nowhere to go
//! and the evening ended when it did; told by the fork now, it builds the
//! playlist out of the library around that song. The panel asks for the song
//! itself as the press lands, so the answer is instant, and this asks again --
//! because on the press that starts the player there was no one there to hear
//! the first one.
//!
//! Its own program rather than three more lines in the panel, because it has
//! to wait: the player is being launched and a bus name is being taken while
//! this runs, and neither is instant. The panel hands it to `later`, which is a
//! thread of the panel's own, and the tab is drawn again when it comes back --
//! by which time the two marks on the transport are lit.
//!
//! Nothing here overrules anyone. Both modes are ordinary presses of the two
//! keys the transport already offers, so turning either of them off is someone
//! saying what they want rather than the machine having never decided.

use std::path::PathBuf;
use std::process::ExitCode;

use console_core_arguments::{Command, Operands, read};
use console_music::player;

const COMMAND: Command = Command {
    name: "music-onward",
    about: "leave the player going: the library, in any order, round for ever, starting from SONG if there is one",
    flags: &[],
    operands: Operands::Optional("SONG"),
};

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let line = match read(&COMMAND, &words) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    let Ok(operands) = line.operands();

    match operands.first().map(PathBuf::from) {
        Some(song) => {
            let Ok(()) = player::onward(&song);
        }
        None => {
            let Ok(_) = player::onward_only();
        }
    }

    ExitCode::SUCCESS
}
