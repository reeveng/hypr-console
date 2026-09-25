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
//! Every five per cent the rocker moves is a note of its own, from the C below
//! middle C at the bottom to under C7 at the top: four octaves of the
//! pentatonic, which is the most there is before a bell turns shrill.
//!
//! Nothing sounds until somebody asks for it. A machine that starts making
//! noises after an update is a machine doing something nobody chose, so
//! [`SoundEffects`] is off until the switch on the Sound tab is turned, and it
//! is the callers that ask it, at their edge, rather than [`play`].

pub mod catalogue;
pub mod synthesis;

use std::fmt;
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::process::Stdio;

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

const A_STEP: NonZeroU32 = match NonZeroU32::new(5) {
    Some(step) => step,
    None => NonZeroU32::MIN,
};

impl Degree {
    #[must_use]
    pub fn climbing(position: u32) -> Result<Degree, Never> {
        let Ok(degree) = fitted::<u32, i32>(position % TWO_OCTAVES);

        Ok(Degree(degree))
    }

    #[must_use]
    pub fn at_level(percent: u32) -> Result<Degree, Never> {
        let Ok(degree) = fitted::<u32, i32>(percent.min(100) / A_STEP);

        Ok(Degree(degree))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub at: f64,
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
    pub fn chosen() -> Result<SoundEffects, Never> {
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

        console_defaults::set(console_defaults::Setting { key: SETTING, value })
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
    NotStarted(io::Error),
    NotWritten(io::Error),
    NoInput,
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaybackError::NotStarted(fault) => write!(formatter, "pw-cat would not start: {fault}"),
            PlaybackError::NotWritten(fault) => write!(formatter, "pw-cat would not take the sound: {fault}"),
            PlaybackError::NoInput => write!(formatter, "pw-cat was started with nothing to write to"),
        }
    }
}

pub fn play(cue: &Cue, root: Degree) -> Result<(), PlaybackError> {
    let Ok(sound) = synthesis::rendered(cue, root);
    let Ok(mut asking) = Program::PwCat.command();

    asking
        .args(["--playback", "--raw", "--format", "s16", "--rate", &synthesis::RATE.to_string()])
        .args(["--channels", "1", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut playing = console_program_lifetime::let_go(&mut asking).map_err(PlaybackError::NotStarted)?;
    let Ok(writing) = playing.writing();

    let written = match writing {
        Some(mut input) => input.write_all(&sound).map_err(PlaybackError::NotWritten),
        None => Err(PlaybackError::NoInput),
    };

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let _ = playing.waiting();
    }));

    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn climbing_goes_up_two_octaves_and_starts_again() {
        assert_eq!(Degree::climbing(0), Ok(Degree(0)));
        assert_eq!(Degree::climbing(9), Ok(Degree(9)));
        assert_eq!(Degree::climbing(10), Ok(Degree(0)));
        assert_eq!(Degree::climbing(23), Ok(Degree(3)));
    }

    #[test]
    fn a_louder_level_is_a_higher_note_and_the_top_is_the_highest() {
        assert_eq!(Degree::at_level(0), Ok(Degree(0)));
        assert_eq!(Degree::at_level(55), Ok(Degree(11)));
        assert_eq!(Degree::at_level(100), Ok(Degree(20)));
        assert_eq!(Degree::at_level(150), Ok(Degree(20)));
    }

    #[test]
    fn every_step_of_the_rocker_is_its_own_note() {
        let notes: Vec<i32> = (0..=20u32).map(|step| match Degree::at_level(step * 5) {
            Ok(Degree(degree)) => degree,
        }).collect();

        assert_eq!(notes, (0..=20).collect::<Vec<i32>>());
    }

    #[test]
    fn the_rocker_climbs_from_the_octave_below_middle_c() {
        let Ok(volume) = catalogue::Sound::Volume.cue();
        let Ok(quietest) = synthesis::hertz(Degree(volume.notes.iter().map(|played| played.degree.0).sum()));
        let Ok(loudest) = synthesis::hertz(Degree(20 - 5));

        assert!((quietest - 130.8).abs() < 0.1, "the bottom step is C3, and it was {quietest}");
        assert!(loudest < 2100.0, "the top step stays under C7, and it was {loudest}");
    }

    #[test]
    fn sound_effects_are_off_until_someone_turns_them_on() {
        assert_eq!(SoundEffects::read(None), Ok(SoundEffects::Off));
        assert_eq!(SoundEffects::read(Some("off")), Ok(SoundEffects::Off));
        assert_eq!(SoundEffects::read(Some("yes please")), Ok(SoundEffects::Off));
        assert_eq!(SoundEffects::read(Some("on")), Ok(SoundEffects::On));
    }
}
