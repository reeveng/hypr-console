//! The player, started once and then only ever asked.
//!
//! One argument, and it is the folder the music is under. The player is told
//! where the library is rather than reading it out of someone else's settings:
//! `console_music::library` used to open kew's `kewrc` to find out, and that
//! file goes wherever the fork goes.
//!
//! The loop is the bus. Nothing is drawn and no toolkit is started -- this is a
//! program whose whole surface is a set of method calls -- so what it costs to
//! have running is a thread carrying samples and a name on the session bus.
//! There was a glib main loop here, for no reason except that the bus was
//! gio's; `console-bus` hands over a socket to wait on instead, and waiting on
//! it is this loop.
//!
//! A song ending is the one thing that happens without anyone asking, beside
//! the clock moving on, which is only said. A private socket wakes the bus loop
//! when the audio worker has an update. The same loop owns the playlist and
//! sends every reply and notification.

use console_bus::connection::{
  Bus, ConnectionError, Incoming, NameRequestResult, Receiver, Sender,
};
use console_bus::messages::Message;
use console_core_arguments::{Command, Flag, Operands, Reason, Takes, ValidationError};
use console_core_iteration::{Endless, Step};
use console_core_never::Never;
use console_music_player::bus::{Modified, PlayerState, changed, handle_message, moved};
use console_music_player::sounding::{Completion, Progress, Sounding};
use std::io::ErrorKind;
use std::os::fd::AsFd;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::process::ExitCode;

use console_music_player::answers;

struct Answering {
  player: PlayerState,
  hearing: Receiver,
  saying: Sender,
  wakeup: UnixDatagram,
}

const SECOND: u8 = b'p';
const ENDED: u8 = b'e';

impl Answering {
  fn turn(&mut self) -> Result<(), ConnectionError> {
    let arrived = self.hearing.receive_with(self.wakeup.as_fd())?;

    match arrived {
      Incoming::Message(message) => self.answer(&message),
      Incoming::Woken => self.progress(),
    }
  }

  fn answer(&mut self, message: &Message) -> Result<(), ConnectionError> {
    let Ok(turn) = handle_message(&mut self.player, message);

    match turn.say {
      Some(say) => {
        let _answered_serial = self.saying.say(&say)?;
      }
      None => {}
    }

    match turn.changed {
      Modified::Yes => {
        let Ok(()) = changed(&self.saying, &self.player);
      }
      Modified::No => {}
    }

    Ok(())
  }

  fn progress(&mut self) -> Result<(), ConnectionError> {
    let mut signal = [0];

    match self.wakeup.recv(&mut signal) {
      Ok(_received) => {}
      Err(fault) => match bool::then_some(
        matches!(fault.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted),
        (),
      ) {
        Some(()) => return Ok(()),
        None => return Err(ConnectionError::from(fault)),
      },
    }

    let Ok(completed) = self.player.sounding.completed();

    match (completed, signal) {
      (Completion::Ended, _) => {
        let Ok(()) = self.player.finished();
        let Ok(()) = self.player.remember();
        let Ok(()) = changed(&self.saying, &self.player);
      }
      (Completion::Pending, [SECOND]) => {
        let Ok(()) = moved(&self.saying);
      }
      (Completion::Pending, _) => {}
    }

    Ok(())
  }
}

fn wake(socket: &UnixDatagram, progress: Progress) -> Result<(), Never> {
  let signal = match progress {
    Progress::Ended => ENDED,
    Progress::ASecondPlayed => SECOND,
  };

  let woken = console_core_iteration::iterate((), |()| {
    Ok(match socket.send(&[signal]) {
      Ok(_sent) => Step::Halt(()),
      Err(fault) => match (
        fault.kind() == ErrorKind::Interrupted,
        fault.kind() == ErrorKind::WouldBlock,
      ) {
        (true, _) => Step::Again(()),
        (false, true) => Step::Halt(()),
        (false, false) => {
          console_core_logging::warn!("could not wake the player for an audio update: {fault}");

          Step::Halt(())
        }
      },
    })
  });

  match woken {
    Ok(()) | Err(Endless) => Ok(()),
  }
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let Ok(code) = console_core_arguments::run_main(&COMMAND, &words, folder, answering);

  code
}

const FOLDER_URI: Flag = Flag {
  spelling: "--folder-uri",
  takes: Takes::Value("FILE_URI"),
  about: "the library folder as a local file URI",
};

const COMMAND: Command = Command {
  name: "music-player",
  about: "the player, answering on the session bus, with the music under FOLDER",
  flags: &[FOLDER_URI],
  operands: Operands::Optional("FOLDER"),
};

fn folder(words: &[String]) -> Result<Option<PathBuf>, ValidationError> {
  let line = console_core_arguments::read(&COMMAND, words)?;
  let Ok(named) = line.operands();
  let Ok(uri) = line.value(FOLDER_URI);

  match uri {
    Some(text) => {
      let Ok(location) = console_core_file_urls::path(text);

      match location {
        Some(path) => Ok(Some(path)),
        None => {
          let Ok(refusal) = line.refusal(Reason::InvalidValue {
            of: FOLDER_URI.spelling,
            value: text.to_owned(),
          });

          Err(refusal)
        }
      }
    }
    None => Ok(named.first().map(PathBuf::from)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::os::unix::ffi::OsStrExt;

  #[test]
  fn only_the_uri_option_decodes_an_encoded_folder() {
    let literal = "file://a%FF".to_owned();
    let decoded = PathBuf::from(std::ffi::OsStr::from_bytes(b"a\xff"));

    assert_eq!(
      folder(std::slice::from_ref(&literal)),
      Ok(Some(PathBuf::from(&literal)))
    );
    assert_eq!(
      folder(&[FOLDER_URI.spelling.to_owned(), literal]),
      Ok(Some(decoded))
    );
  }
}

fn answering(folder: Option<PathBuf>) -> Result<(), ConnectionError> {
  let mut bus = Bus::session()?;
  let got = bus.request_name(answers::NAME)?;

  match got {
    NameRequestResult::PrimaryOwner | NameRequestResult::AlreadyOwner => {}
    NameRequestResult::InQueue | NameRequestResult::Exists => {
      eprintln!("music-player: another player has the name, so this one answers to nothing");

      return Ok(());
    }
  }

  let (wakeup, audio) = UnixDatagram::pair()?;

  audio.set_nonblocking(true)?;
  wakeup.set_nonblocking(true)?;

  let Ok(sounding) = Sounding::new(move |progress| wake(&audio, progress));
  let Ok(player) = PlayerState::new(sounding, folder);
  let Ok((hearing, saying)) = bus.split();
  let owned = Answering {
    hearing,
    saying,
    wakeup,
    player,
  };
  let heard = console_core_iteration::iterate(owned, |mut answering| {
    Ok(match answering.turn() {
      Ok(()) => Step::Again(answering),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match heard {
    Ok(heard_2) => heard_2,
    Err(endless) => {
      eprintln!("music-player: {endless}");

      Ok(())
    }
  }
}
