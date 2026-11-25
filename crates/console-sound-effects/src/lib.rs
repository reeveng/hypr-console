//! What a press sounds like.
//!
//! The rocker changed a number that was only ever shown, and a list stepped
//! through by the pad answered with nothing but a moved highlight, so a hand on
//! the device had nothing to go on while its eyes were somewhere else. This is
//! codincod's cue engine brought across: a cue is a short timeline of notes, a
//! note is a scale degree rather than a frequency, and [`synthesis`] is what
//! turns one into sound. [`catalogue`] is every cue this desktop plays, by name.
//!
//! A cue is rendered here and played through the shared desktop audio stream.
//! No sound file and no sound theme:
//! the freedesktop theme has no tick for a list, and a synthesised one can be
//! pitched, which is what lets a row further down sound higher than the one
//! above it.
//!
//! [`play`] submits the samples and returns while the audio stream drains.
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
pub mod gain;
pub mod preview;
pub mod synthesis;

use std::fmt;
use std::io;
use std::num::NonZeroU32;
use std::time::Duration;

use console_audio_streams::{CueError, Pcm, Sample, play_cue};
use console_core_never::Never;
use console_core_number_conversion::fitted;

pub use preview::play_volume;

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
  Preview(io::Error),
  Output(CueError),
}

impl fmt::Display for PlaybackError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      PlaybackError::Cue(fault) => write!(formatter, "{fault}"),
      PlaybackError::Preview(fault) => {
        write!(formatter, "could not handle the volume preview: {fault}")
      }
      PlaybackError::Output(fault) => write!(formatter, "{fault}"),
    }
  }
}

const CUE_PCM: Pcm = Pcm {
  rate_hz: synthesis::RATE,
  channels: 1,
  sample: Sample::I16,
};

pub fn play(cue: &Cue, root: Degree) -> Result<(), PlaybackError> {
  let mut sound = synthesis::render(cue, root).map_err(PlaybackError::Cue)?;
  let format = balance::PlaybackFormat {
    sample_rate_hz: synthesis::RATE,
    channels: CUE_PCM.channels,
  };
  let mut balance = balance::Balance::new(format).map_err(PlaybackError::Cue)?;
  let Ok(()) = balance.apply_mono_pcm16(&mut sound);

  play_cue(CUE_PCM, &sound, None).map_err(PlaybackError::Output)
}

fn volume_samples() -> Result<Vec<u8>, synthesis::RenderError> {
  let Ok(cue) = catalogue::Sound::Volume.cue();

  synthesis::render(&cue, Degree(10))
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
