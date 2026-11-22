//! What a press sounds like.
//!
//! The rocker changed a number that was only ever shown, and a list stepped
//! through by the pad answered with nothing but a moved highlight, so a hand on
//! the device had nothing to go on while its eyes were somewhere else. This is
//! codincod's cue engine brought across: a cue is a short timeline of notes, a
//! note is a scale degree rather than a frequency, and [`synthesis`] is what
//! turns one into sound. [`catalogue`] is every cue this desktop plays, by name.
//!
//! A cue is played by rendering it here and handing the samples to pw-cat, the
//! player the music player already speaks to. No sound file and no sound theme:
//! the freedesktop theme has no tick for a list, and a synthesised one can be
//! pitched, which is what lets a row further down sound higher than the one
//! above it.
//!
//! [`play`] never waits for the sound. The samples fit in a pipe's buffer, so
//! the write returns at once and pw-cat drains them after its input has closed;
//! a thread of its own collects pw-cat when it is done, so a panel stepping all
//! evening leaves nothing behind it.
//!
//! The volume preview uses the same note at every setting. Its sink sets its
//! loudness, and the rendered note supplies the reference for balance.
//!
//! Nothing sounds until somebody asks for it. A machine that starts making
//! noises after an update is a machine doing something nobody chose, so
//! [`SoundEffects`] is off until the switch on the Sound tab is turned, and it
//! is the callers that ask it, at their edge, rather than [`play`].

pub mod balance;
pub mod catalogue;
pub mod synthesis;

use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::process::Stdio;
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_program_lifetime::threads;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
  Tap,
  Bell,
  Bass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Degree(pub i32);

const TWO_OCTAVES: NonZeroU32 = match NonZeroU32::new(10) {
  Some(span) => span,
  None => NonZeroU32::MIN,
};

impl Degree {
  #[must_use]
  pub fn degree_at(position: u32) -> Result<Degree, Never> {
    let Ok(degree) = fitted::<u32, i32>(position % TWO_OCTAVES);

    Ok(Degree(degree))
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
  pub at: Duration,
  pub degree: Degree,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cue {
  pub voice: Voice,
  pub notes: &'static [Note],
}

const SETTING: &str = "sound-effects";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEffects {
  On,
  Off,
}

impl SoundEffects {
  #[must_use]
  pub fn current() -> Result<SoundEffects, Never> {
    let told = console_defaults::setting(SETTING)?;

    SoundEffects::read(told.as_deref())
  }

  #[must_use]
  pub fn read(told: Option<&str>) -> Result<SoundEffects, Never> {
    match told {
      Some("on") => Ok(SoundEffects::On),
      Some(_) | None => Ok(SoundEffects::Off),
    }
  }

  pub fn choose(self) -> Result<(), Never> {
    let value = match self {
      SoundEffects::On => "on",
      SoundEffects::Off => "off",
    };

    console_defaults::set(console_defaults::Setting {
      key: SETTING,
      value,
    })
  }

  #[must_use]
  pub fn flipped(self) -> Result<SoundEffects, Never> {
    match self {
      SoundEffects::On => Ok(SoundEffects::Off),
      SoundEffects::Off => Ok(SoundEffects::On),
    }
  }
}

#[derive(Debug)]
pub enum PlaybackError {
  Cue(synthesis::RenderError),
  PreviewLock(io::Error),
  NotStarted(io::Error),
  NotWritten(io::Error),
  NoInput,
}

impl fmt::Display for PlaybackError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PlaybackError::Cue(fault) => write!(formatter, "{fault}"),
      PlaybackError::PreviewLock(fault) => {
        write!(formatter, "could not reserve the volume preview: {fault}")
      }
      PlaybackError::NotStarted(fault) => write!(formatter, "pw-cat would not start: {fault}"),
      PlaybackError::NotWritten(fault) => {
        write!(formatter, "pw-cat would not take the sound: {fault}")
      }
      PlaybackError::NoInput => write!(formatter, "pw-cat was started with nothing to write to"),
    }
  }
}

pub fn play(cue: &Cue, root: Degree) -> Result<(), PlaybackError> {
  let mut sound = synthesis::render(cue, root).map_err(PlaybackError::Cue)?;
  let mut balance = balance::Balance::new(synthesis::RATE, 1).map_err(PlaybackError::Cue)?;
  let Ok(()) = balance.apply_mono_pcm16(&mut sound);

  play_samples(&sound, None)
}

fn volume_samples() -> Result<Vec<u8>, synthesis::RenderError> {
  let Ok(cue) = catalogue::Sound::Volume.cue();

  synthesis::render(&cue, Degree(10))
}

pub fn play_volume(sink: &str) -> Result<(), PlaybackError> {
  let Ok(runtime) = console_core_places::runtime_or_user();
  let lock = OpenOptions::new()
    .create(true)
    .append(true)
    .open(runtime.join("console-volume-preview.lock"))
    .map_err(PlaybackError::PreviewLock)?;

  match lock.try_lock() {
    Ok(()) => {
      let sound = volume_samples().map_err(PlaybackError::Cue)?;

      play_samples(&sound, Some((sink, lock)))
    }
    Err(TryLockError::WouldBlock) => Ok(()),
    Err(TryLockError::Error(fault)) => Err(PlaybackError::PreviewLock(fault)),
  }
}

fn play_samples(sound: &[u8], preview: Option<(&str, File)>) -> Result<(), PlaybackError> {
  let Ok(mut asking) = Program::PwCat.command();

  match preview {
    Some((sink, lock)) => {
      asking.args([
        "--target",
        sink,
        "--properties",
        concat!(
          "{ application.name = \"Volume Preview\" ",
          "console.balance.preview = true ",
          "state.restore-props = false state.default-volume = 1.0 ",
          "state.restore-target = false node.dont-fallback = true ",
          "node.dont-reconnect = true node.dont-move = true }",
        ),
      ]);
      // The detached player inherits the lock on its unused standard output.
      // This keeps repeated presses from summing until the cue finishes.
      asking.stdout(Stdio::from(lock));
    }
    None => {
      asking.stdout(Stdio::null());
    }
  }

  asking
    .args([
      "--playback",
      "--raw",
      "--format",
      "s16",
      "--rate",
      &synthesis::RATE.to_string(),
    ])
    .args(["--channels", "1", "-"])
    .stdin(Stdio::piped())
    .stderr(Stdio::null());

  let mut playing =
    console_program_lifetime::let_go(&mut asking).map_err(PlaybackError::NotStarted)?;
  let Ok(writing) = playing.take_stdin();

  let written = match writing {
    Some(mut input) => input.write_all(sound).map_err(PlaybackError::NotWritten),
    None => Err(PlaybackError::NoInput),
  };

  let Ok(()) = threads::let_go(std::thread::spawn(move || {
    #[cfg_attr(
      dylint_lib = "explicit060_no_reasonless_discard",
      expect(
        explicit060_no_reasonless_discard,
        reason = "this worker reaps a detached optional sound effect; its exit status cannot change the input that already triggered the cue"
      )
    )]
    let _ = playing.wait();
  }));

  written
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn climbing_goes_up_two_octaves_and_starts_again() {
    assert_eq!(Degree::degree_at(0), Ok(Degree(0)));
    assert_eq!(Degree::degree_at(9), Ok(Degree(9)));
    assert_eq!(Degree::degree_at(10), Ok(Degree(0)));
    assert_eq!(Degree::degree_at(23), Ok(Degree(3)));
  }

  #[test]
  fn sound_effects_are_off_until_someone_turns_them_on() {
    assert_eq!(SoundEffects::read(None), Ok(SoundEffects::Off));
    assert_eq!(SoundEffects::read(Some("off")), Ok(SoundEffects::Off));
    assert_eq!(
      SoundEffects::read(Some("yes please")),
      Ok(SoundEffects::Off)
    );
    assert_eq!(SoundEffects::read(Some("on")), Ok(SoundEffects::On));
  }
}
