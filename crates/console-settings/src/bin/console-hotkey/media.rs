use std::io;

use console_core_external_programs::Program;
use console_core_never::Never;
use serde::Deserialize;

use super::called;

const PLAYER_PREFIX: &str = "org.mpris.MediaPlayer2.";
const OBJECT: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Clone, Copy)]
pub enum Playback {
  PlayPause,
  Next,
  Previous,
}

impl Playback {
  fn method(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::PlayPause => "PlayPause",
      Self::Next => "Next",
      Self::Previous => "Previous",
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
enum Status {
  Playing,
  Paused,
  Stopped,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
  #[serde(rename = "type")]
  _signature: Signature,
  data: Status,
}

#[derive(Deserialize)]
enum Signature {
  #[serde(rename = "s")]
  String,
}

fn playback_status(name: &str) -> io::Result<Status> {
  let reply = called(
    Program::Busctl,
    &[
      "--user",
      "--timeout=1s",
      "--json=short",
      "get-property",
      name,
      OBJECT,
      PLAYER,
      "PlaybackStatus",
    ],
  )?;
  let status: Reply = serde_json::from_str(&reply)?;

  Ok(status.data)
}

pub fn run(playback: Playback) -> io::Result<()> {
  let running = called(
    Program::Busctl,
    &["--user", "--no-legend", "--full", "--acquired", "list"],
  )?;
  let players = running.lines().filter_map(|line| {
    let name = line.split_whitespace().next()?;

    name.starts_with(PLAYER_PREFIX).then_some(name)
  });
  let selected = players
    .filter_map(|name| match playback_status(name) {
      Ok(status) => Some((status, name)),
      Err(fault) => {
        console_core_logging::debug!(
          "console-hotkey: reading playback status from {name}: {fault}"
        );
        None
      }
    })
    .min();

  let name = match selected {
    Some((_, name)) => name,
    None => return Ok(()),
  };
  let Ok(method) = playback.method();
  let _reply = called(
    Program::Busctl,
    &[
      "--user",
      "--timeout=1s",
      "--auto-start=no",
      "call",
      name,
      OBJECT,
      PLAYER,
      method,
    ],
  )?;

  Ok(())
}
