//! The volume rocker on the top edge.
//!
//!     console-volume up | down | mute | mic-mute
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
use console_settings::level::Muted;
use console_settings::rocker::{self, ButtonPress, sound_server};
use std::io;

fn show_volume(sink: &str) -> io::Result<()> {
  let level = sound_server(&["get-sink-volume", sink])?;
  let mute = sound_server(&["get-sink-mute", sink])?;
  let Ok(muted) = rocker::muted(&mute);
  let Ok(level_2) = rocker::level(&level);
  let Ok(words) = rocker::volume_label(level_2, muted);

  let Ok(notification) = Notification::new(Content {
    summary: &words,
    body: "",
  });
  let Ok(mut notification_2) = notification.lasting(1500);

  let Ok(value) = rocker::value(level_2);

  match value {
    Some(value_2) => {
      let Ok(valued) = notification_2.valued(value_2);
      notification_2 = valued;
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
  let Ok(()) = raise_kept(notification_2, &kept);

  Ok(())
}

fn show_microphone(source: &str) -> io::Result<()> {
  let mute = sound_server(&["get-source-mute", source])?;
  let Ok(muted) = rocker::muted(&mute);
  let summary = match muted {
    Muted::Yes => "Microphone muted",
    Muted::No => "Microphone unmuted",
  };
  let Ok(notification) = Notification::new(Content { summary, body: "" });
  let Ok(notification_2) = notification.lasting(1500);
  let Ok(kept) = StatePath::new("microphone");
  let Ok(()) = raise_kept(notification_2, &kept);

  Ok(())
}

fn adjust(press: ButtonPress) -> io::Result<()> {
  let (query, missing) = match press {
    ButtonPress::Up | ButtonPress::Down | ButtonPress::Mute => {
      ("get-default-sink", "pactl reported no default output")
    }
    ButtonPress::MicMute => ("get-default-source", "pactl reported no default microphone"),
  };
  let default = sound_server(&[query])?;
  let device = default.trim();

  match ::core::primitive::bool::then_some(device.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Err(io::Error::other(missing)),
    ::core::option::Option::None => {}
  }

  let current = match press {
    ButtonPress::Up | ButtonPress::Down => {
      let said = sound_server(&["get-sink-volume", device])?;
      let Ok(level) = rocker::level(&said);
      let Ok(value) = rocker::value(level);
      value
        .and_then(|value_2| match i32::try_from(value_2) {
          ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__),
          ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
        })
        .ok_or_else(|| io::Error::other("pactl reported no volume for the default output"))?
    }
    ButtonPress::Mute | ButtonPress::MicMute => 0,
  };
  let Ok(requests) = rocker::asks(press, device, current);

  for arguments in requests {
    let _reply = sound_server(&arguments)?;
  }

  match press {
    ButtonPress::Up | ButtonPress::Down | ButtonPress::Mute => show_volume(device),
    ButtonPress::MicMute => show_microphone(device),
  }
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
