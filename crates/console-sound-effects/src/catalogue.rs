//! Every sound this desktop makes, by name.
//!
//! codincod kept its cues as constants and a table from a name to each, so a
//! template could ask for a sound without importing one. The same two halves
//! are here, and the name is the variant's own word rather than a second list
//! of strings: a program asks for [`Sound::Confirm`], and a script or a
//! binding asks `console-sound-effect confirm`, and both reach one cue.
//!
//! The cues are codincod's, with its octaves folded into degrees: five degrees
//! is an octave, so a note codincod wrote one octave up is five higher here.
//! A cue is written from degree nothing and the caller hands in the root, which
//! is what lets one cue climb with a row or a level without becoming another.

use console_core_never::Never;
use console_core_words::Words;

use crate::{Cue, Degree, Note, Voice};

const BEAT: f64 = 0.09;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Words)]
pub enum Sound {
    #[words(word = "step")]
    Step,
    #[words(word = "volume")]
    Volume,
    #[words(word = "confirm")]
    Confirm,
    #[words(word = "success")]
    Success,
    #[words(word = "failure")]
    Failure,
    #[words(word = "warning")]
    Warning,
    #[words(word = "attention")]
    Attention,
    #[words(word = "connect")]
    Connect,
    #[words(word = "disconnect")]
    Disconnect,
    #[words(word = "celebrate")]
    Celebrate,
}

pub const EVERY: &[Sound] = &[
    Sound::Step,
    Sound::Volume,
    Sound::Confirm,
    Sound::Success,
    Sound::Failure,
    Sound::Warning,
    Sound::Attention,
    Sound::Connect,
    Sound::Disconnect,
    Sound::Celebrate,
];

const STEP: &[Note] = &[
    Note { at: 0.0, degree: Degree(5) },
];

const VOLUME: &[Note] = &[
    Note { at: 0.0, degree: Degree(-5) },
];

const CONFIRM: &[Note] = &[
    Note { at: 0.0, degree: Degree(2) },
    Note { at: BEAT, degree: Degree(4) },
];

const SUCCESS: &[Note] = &[
    Note { at: 0.0, degree: Degree(5) },
    Note { at: BEAT, degree: Degree(7) },
    Note { at: 2.0 * BEAT, degree: Degree(9) },
];

const FAILURE: &[Note] = &[
    Note { at: 0.0, degree: Degree(-5) },
    Note { at: BEAT, degree: Degree(-7) },
];

const WARNING: &[Note] = &[
    Note { at: 0.0, degree: Degree(2) },
    Note { at: BEAT, degree: Degree(5) },
];

const ATTENTION: &[Note] = &[
    Note { at: 0.0, degree: Degree(4) },
    Note { at: BEAT, degree: Degree(7) },
];

const CONNECT: &[Note] = &[
    Note { at: 0.0, degree: Degree(0) },
    Note { at: BEAT, degree: Degree(2) },
];

const DISCONNECT: &[Note] = &[
    Note { at: 0.0, degree: Degree(-2) },
];

const CELEBRATE: &[Note] = &[
    Note { at: 0.0, degree: Degree(5) },
    Note { at: BEAT, degree: Degree(7) },
    Note { at: 2.0 * BEAT, degree: Degree(9) },
    Note { at: 3.0 * BEAT, degree: Degree(10) },
];

impl Sound {
    #[must_use]
    pub fn cue(self) -> Result<Cue, Never> {
        Ok(match self {
            Sound::Step => Cue { voice: Voice::Tap, notes: STEP },
            Sound::Volume => Cue { voice: Voice::Bell, notes: VOLUME },
            Sound::Confirm => Cue { voice: Voice::Bell, notes: CONFIRM },
            Sound::Success => Cue { voice: Voice::Bell, notes: SUCCESS },
            Sound::Failure => Cue { voice: Voice::Bass, notes: FAILURE },
            Sound::Warning => Cue { voice: Voice::Tap, notes: WARNING },
            Sound::Attention => Cue { voice: Voice::Bell, notes: ATTENTION },
            Sound::Connect => Cue { voice: Voice::Tap, notes: CONNECT },
            Sound::Disconnect => Cue { voice: Voice::Bass, notes: DISCONNECT },
            Sound::Celebrate => Cue { voice: Voice::Bell, notes: CELEBRATE },
        })
    }

    #[must_use]
    pub fn named(word: &str) -> Result<Option<Sound>, Never> {
        Ok(EVERY.iter().copied().find(|sound| sound.word() == Ok(word)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesis::rendered;

    #[test]
    fn every_sound_is_found_by_its_own_word() {
        for sound in EVERY {
            let Ok(word) = sound.word();

            assert_eq!(Sound::named(word), Ok(Some(*sound)));
        }

        assert_eq!(Sound::named("Confirm"), Ok(None));
        assert_eq!(Sound::named(""), Ok(None));
    }

    #[test]
    fn every_sound_fits_in_a_pipe_so_playing_it_never_waits() {
        for sound in EVERY {
            let Ok(cue) = sound.cue();
            let Ok(bytes) = rendered(&cue, Degree(10));

            assert!(bytes.len() < 65_536, "{sound:?} is {} bytes", bytes.len());
        }
    }

    #[test]
    fn no_two_sounds_are_the_same_cue() {
        for (index, sound) in EVERY.iter().enumerate() {
            for other in EVERY.iter().skip(index + 1) {
                assert_ne!(sound.cue(), other.cue(), "{sound:?} and {other:?}");
            }
        }
    }
}
