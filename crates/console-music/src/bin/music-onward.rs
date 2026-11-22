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
//! this runs, and neither is instant. The panel runs it as a task, on a thread
//! of the panel's own, and the tab is drawn again when it comes back -- by
//! which time the two marks on the transport are lit.
//!
//! Nothing here overrules anyone. Both modes are ordinary presses of the two
//! keys the transport already offers, so turning either of them off is someone
//! saying what they want rather than the machine having never decided.

use std::path::PathBuf;
use std::process::ExitCode;

use console_core_arguments::{Command, Flag, Operands, Reason, Takes, ValidationError, read};
use console_music::player;

const SONG_URI: Flag = Flag {
  spelling: "--song-uri",
  takes: Takes::Value("FILE_URI"),
  about: "the song as a local file URI",
};

const COMMAND: Command = Command {
  name: "music-onward",
  about: "leave the player going: the library, in any order, round for ever, starting from SONG if there is one",
  flags: &[SONG_URI],
  operands: Operands::Optional("SONG"),
};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let song = match chosen(&words) {
    Ok(song) => song,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  match song {
    Some(song_2) => {
      let Ok(()) = player::onward(&song_2);
    }
    None => {
      let Ok(_) = player::onward_only();
    }
  }

  ExitCode::SUCCESS
}

fn chosen(words: &[String]) -> Result<Option<PathBuf>, ValidationError> {
  let line = read(&COMMAND, words)?;
  let Ok(operands) = line.operands();
  let Ok(uri) = line.value(SONG_URI);

  match uri {
    Some(text) => {
      let Ok(path) = console_core_file_urls::path(text);

      match path {
        Some(path_2) => Ok(Some(path_2)),
        None => {
          let Ok(refusal) = line.refusal(Reason::InvalidValue {
            of: SONG_URI.spelling,
            value: text.to_owned(),
          });

          Err(refusal)
        }
      }
    }
    None => Ok(operands.first().map(PathBuf::from)),
  }
}
