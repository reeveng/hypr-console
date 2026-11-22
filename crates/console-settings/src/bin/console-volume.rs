//! The volume rocker on the top edge.
//!
//!     console-volume up | down | mute
//!
//! What it does is `console_settings::rocker`, which can be asked without a
//! sound server. This is the part that needs one, and the notification afterwards.
//!
//! One notification, replaced. Held down, the rocker steps five per cent at a time
//! and every step would otherwise be another card, so the number the last one
//! came back under is kept and handed to `--replace-id`.
//!
//! The rocker plays the same reference sound through the sink it adjusted,
//! at the new loudness, when Sound Effects is enabled and the sink is audible.

use console_core_arguments::{Operands, read_with};
use console_notifications::saying::{Content, Notification, StatePath, raise_kept};
use console_settings::rocker::{self, ButtonPress, sound_server};
use std::io;

fn show_volume(sink: &str) -> io::Result<()> {
  let level = sound_server(&["get-sink-volume", sink])?;
  let mute = sound_server(&["get-sink-mute", sink])?;
  let Ok(muted) = rocker::muted(&mute);
  let Ok(level) = rocker::level(&level);
  let Ok(words) = rocker::volume_label(level, muted);

  let Ok(notification) = Notification::new(Content {
    summary: &words,
    body: "",
  });
  let Ok(mut notification) = notification.lasting(1500);

  let Ok(value) = rocker::value(level);
  match value {
    Some(value) => {
      let Ok(valued) = notification.valued(value);
      notification = valued;
    }
    None => {}
  }

  match mute.trim() {
    "Mute: no" => {
      let Ok(()) = rocker::preview(sink, muted, value);
    }
    _silent_or_unknown => {}
  }

  let Ok(kept) = StatePath::new("volume");
  let Ok(()) = raise_kept(notification, &kept);

  Ok(())
}

fn adjust(press: ButtonPress) -> io::Result<()> {
  let default = sound_server(&["get-default-sink"])?;
  let sink = default.trim();
  match sink.is_empty() {
    true => return Err(io::Error::other("pactl reported no default output")),
    false => {}
  }

  let Ok(requests) = rocker::asks(press, sink);
  for arguments in requests {
    let _reply = sound_server(&arguments)?;
  }

  show_volume(sink)
}

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-volume",
  about: "the volume rocker on the top edge",
  flags: &[],
  operands: Operands::None,
};

fn main() -> std::process::ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let read =
    read_with::<ButtonPress, String>(&COMMAND, &words).and_then(|line| line.require_subcommand());

  let press = match read {
    Ok(press) => press,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return std::process::ExitCode::from(code);
    }
  };

  match adjust(press) {
    Ok(()) => std::process::ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("console-volume: {fault}");
      std::process::ExitCode::FAILURE
    }
  }
}
