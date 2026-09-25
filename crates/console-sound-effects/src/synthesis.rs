//! A cue turned into the samples pw-cat is handed, with no machine in it.
//!
//! Every pitch is a degree of the major pentatonic, the idea codincod took from
//! Kit Langton's `TaskSounds`: any two notes that overlap are consonant by
//! construction, so a press landing over the tail of the one before it is a
//! chord rather than a clash. A root moves the whole cue up the scale, which is
//! how a row further down or a louder volume sounds higher without either
//! becoming a different sound.
//!
//! The timeline is rendered rather than scheduled. codincod had a clock to
//! share with an animation and scheduled each note on it; here the notes are
//! mixed into one buffer at their offsets, so the timing is exact to the sample
//! and there is no timer to drift.
//!
//! The voices are codincod's three with the tails cut. A browser's convolution
//! reverb gave its notes a room, and a handheld stepping through a list at key
//! repeat wants each tick gone before the next, so there is no reverb and the
//! decay is short enough that a cue fits in a pipe's buffer.

use std::f64::consts::{FRAC_2_PI, TAU};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32, whole_i32};

use crate::{Cue, Degree, Voice};

pub const RATE: u32 = 48_000;

const PENTATONIC: [i32; 5] = [0, 2, 4, 7, 9];

const ROOT_HERTZ: f64 = 261.63;

const QUIET: f64 = 0.001;

const LOUDEST: f64 = 32_767.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Wave {
    Sine,
    Triangle,
    Sawtooth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Partial {
    ratio: f64,
    gain: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Sounding {
    wave: Wave,
    attack: f64,
    decay: f64,
    lowpass: f64,
    gain: f64,
    partial: Option<Partial>,
}

fn sounding(voice: Voice) -> Result<Sounding, Never> {
    Ok(match voice {
        Voice::Tap => Sounding { wave: Wave::Sine, attack: 0.005, decay: 0.25, lowpass: 3500.0, gain: 0.2, partial: None },
        Voice::Bell => Sounding {
            wave: Wave::Triangle,
            attack: 0.006,
            decay: 0.4,
            lowpass: 6000.0,
            gain: 0.16,
            partial: Some(Partial { ratio: 2.0, gain: 0.35 }),
        },
        Voice::Bass => Sounding { wave: Wave::Sawtooth, attack: 0.02, decay: 0.4, lowpass: 900.0, gain: 0.2, partial: None },
    })
}

#[must_use]
pub fn semitones(degree: Degree) -> Result<i32, Never> {
    let Ok(across) = fitted::<_, i32>(PENTATONIC.len());
    let octave = degree.0.div_euclid(across);
    let Ok(within) = index(degree.0.rem_euclid(across));

    let step = match PENTATONIC.get(within) {
        Some(step) => *step,
        None => 0,
    };

    Ok(step.saturating_add(octave.saturating_mul(12)))
}

#[must_use]
pub fn hertz(degree: Degree) -> Result<f64, Never> {
    let Ok(semitones) = semitones(degree);

    Ok(ROOT_HERTZ * 2f64.powf(f64::from(semitones) / 12.0))
}

fn wave(shape: Wave, turns: f64) -> Result<f64, Never> {
    Ok(match shape {
        Wave::Sine => (TAU * turns).sin(),
        Wave::Triangle => FRAC_2_PI * (TAU * turns).sin().asin(),
        Wave::Sawtooth => 2.0 * (turns - (turns + 0.5).floor()),
    })
}

fn envelope(sound: Sounding, seconds: f64) -> Result<f64, Never> {
    Ok(match seconds < sound.attack {
        true => sound.gain * seconds / sound.attack,
        false => sound.gain * (QUIET / sound.gain).powf((seconds - sound.attack) / (sound.decay - sound.attack)),
    })
}

fn samples(seconds: f64) -> Result<u32, Never> {
    toward_zero_u32(seconds * f64::from(RATE))
}

fn note(sound: Sounding, pitch: f64) -> Result<impl Iterator<Item = f64>, Never> {
    let Ok(count) = samples(sound.decay);
    let smoothing = 1.0 - (-TAU * sound.lowpass / f64::from(RATE)).exp();

    Ok((0..count).scan(0.0, move |held, sample| {
        let seconds = f64::from(sample) / f64::from(RATE);
        let Ok(fundamental) = wave(sound.wave, pitch * seconds);

        let over = match sound.partial {
            Some(partial) => {
                let Ok(over) = wave(Wave::Sine, pitch * partial.ratio * seconds);

                over * partial.gain
            }
            None => 0.0,
        };

        *held += smoothing * (fundamental + over - *held);

        let Ok(shaped) = envelope(sound, seconds);

        Some(*held * shaped)
    }))
}

#[must_use]
pub fn rendered(cue: &Cue, root: Degree) -> Result<Vec<u8>, Never> {
    let Ok(sound) = sounding(cue.voice);
    let last = cue.notes.iter().map(|played| played.at).fold(0.0, f64::max);
    let Ok(length) = samples(last + sound.decay);
    let Ok(length) = index(length);
    let mut mixed = vec![0.0; length];

    for played in cue.notes {
        let Ok(pitch) = hertz(Degree(root.0.saturating_add(played.degree.0)));
        let Ok(from) = samples(played.at);
        let Ok(from) = index(from);
        let Ok(sounded) = note(sound, pitch);

        for (slot, sample) in mixed.iter_mut().skip(from).zip(sounded) {
            *slot += sample;
        }
    }

    Ok(mixed.iter().flat_map(|sample| {
        let Ok(level) = whole_i32(sample.clamp(-1.0, 1.0) * LOUDEST);
        let Ok(level) = fitted::<i32, i16>(level);

        level.to_le_bytes()
    }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::Sound;
    use crate::Note;

    #[test]
    fn five_degrees_are_an_octave() {
        assert_eq!(semitones(Degree(0)), Ok(0));
        assert_eq!(semitones(Degree(3)), Ok(7));
        assert_eq!(semitones(Degree(5)), Ok(12));
        assert_eq!(semitones(Degree(-1)), Ok(-3));
    }

    #[test]
    fn degree_nothing_is_middle_c() {
        let Ok(pitch) = hertz(Degree(0));
        assert!((pitch - ROOT_HERTZ).abs() < 0.01);

        let Ok(octave) = hertz(Degree(5));
        assert!((octave - 2.0 * ROOT_HERTZ).abs() < 0.01);
    }

    #[test]
    fn a_cue_lasts_until_its_last_note_has_decayed() {
        let cue = Cue { voice: Voice::Tap, notes: &[Note { at: 0.0, degree: Degree(0) }, Note { at: 0.1, degree: Degree(2) }] };
        let Ok(bytes) = rendered(&cue, Degree(0));
        let Ok(sound) = sounding(Voice::Tap);
        let Ok(expected) = samples(0.1 + sound.decay);

        assert_eq!(fitted::<_, u32>(bytes.len()), Ok(2 * expected));
    }

    #[test]
    fn a_cue_is_heard_and_then_is_silent() {
        let Ok(step) = Sound::Step.cue();
        let Ok(bytes) = rendered(&step, Degree(0));
        let levels: Vec<i16> = bytes.chunks(2).map(|pair| i16::from_le_bytes([pair[0], pair[1]])).collect();
        let loudest = levels.iter().map(|level| level.unsigned_abs()).max().expect("some samples");
        let last = levels.last().expect("some samples").unsigned_abs();

        assert!(loudest > 1000, "loudest was {loudest}");
        assert!(last < 100, "the tail ended at {last}");
    }

    #[test]
    fn a_rooted_cue_sounds_higher() {
        let crossings = |bytes: Vec<u8>| {
            let levels: Vec<i16> = bytes.chunks(2).map(|pair| i16::from_le_bytes([pair[0], pair[1]])).collect();
            fitted::<_, u32>(levels.windows(2).filter(|pair| pair[0] < 0 && pair[1] >= 0).count()).expect("counted")
        };

        let Ok(step) = Sound::Step.cue();
        let low = crossings(rendered(&step, Degree(0)).expect("rendered"));
        let high = crossings(rendered(&step, Degree(5)).expect("rendered"));

        assert!(high > low + low / 2, "{low} crossings against {high}");
    }

    #[test]
    fn a_cue_fits_in_a_pipe_so_writing_it_never_waits() {
        for voice in [Voice::Tap, Voice::Bell, Voice::Bass] {
            let cue = Cue { voice, notes: &[Note { at: 0.0, degree: Degree(0) }] };
            let Ok(bytes) = rendered(&cue, Degree(0));

            assert!(bytes.len() < 65_536, "{voice:?} is {} bytes", bytes.len());
        }
    }
}
