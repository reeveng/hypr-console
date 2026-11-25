//! Desktop actions invoked by the dedicated keyboard keys.

use std::{io, process::ExitCode};

use console_core_arguments::{Operands, Subcommand, read_with};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_settings::power_profile::Profile;

#[path = "console-hotkey/media.rs"]
mod media;
#[path = "console-hotkey/touchpad.rs"]
mod touchpad;

#[derive(Clone, Copy, Words)]
enum Action {
  #[words(word = "power-profile", about = "cycle available power modes")]
  PowerProfile,
  #[words(word = "play-pause", about = "play or pause media")]
  PlayPause,
  #[words(word = "next-track", about = "play the next track")]
  Next,
  #[words(word = "previous-track", about = "play the previous track")]
  Previous,
  #[words(word = "toggle-touchpad", about = "enable or disable the touchpad")]
  Touchpad,
}

impl Subcommand for Action {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    Ok(Self::VARIANTS.iter().copied())
  }

  fn spelling(self) -> Result<&'static str, Never> {
    self.word()
  }

  fn about(self) -> Result<&'static str, Never> {
    Action::about(self)
  }
}

const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-hotkey",
  about: "desktop actions for dedicated keys",
  flags: &[],
  operands: Operands::None,
};

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let read =
    read_with::<Action, String>(&COMMAND, &words).and_then(|line| line.require_subcommand());
  let action = match read {
    Ok(action) => action,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let result = match action {
    Action::PowerProfile => cycle_profile(),
    Action::PlayPause => media::run(media::Playback::PlayPause),
    Action::Next => media::run(media::Playback::Next),
    Action::Previous => media::run(media::Playback::Previous),
    Action::Touchpad => touchpad::toggle(),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::warn!("console-hotkey: {fault}");

      ExitCode::FAILURE
    }
  }
}

fn called(program: Program, arguments: &[&str]) -> io::Result<String> {
  let Ok(mut command) = program.command();
  let output = command.env("LC_ALL", "C").args(arguments).output()?;

  match output.status.success().then_some(()) {
    Some(()) => String::from_utf8(output.stdout).map_err(io::Error::other),
    None => {
      let Ok(name) = program.name();
      let message = String::from_utf8(output.stderr).map_err(io::Error::other)?;

      Err(io::Error::other(format!(
        "{name} {}: {}",
        output.status,
        message.trim()
      )))
    }
  }
}

fn cycle_profile() -> io::Result<()> {
  let list = called(Program::Powerprofilesctl, &["list"])?;
  let Ok(next) = Profile::next(&list);
  let profile = next.ok_or_else(|| io::Error::other("no power profiles available"))?;
  let Ok(name) = profile.written();
  let _reply = called(Program::Powerprofilesctl, &["set", name])?;
  let Ok(label) = profile.label();
  let Ok(()) =
    console_notifications::notify(console_notifications::Application("Power"), &label, None);

  Ok(())
}
