//! A cue turned into the samples pw-cat is handed, with no machine in it.
//!
//! Every pitch is a degree of the major pentatonic, the idea codincod took from
//! Kit Langton's `TaskSounds`: any two notes that overlap are consonant by
//! construction, so a press landing over the tail of the one before it is a
//! chord rather than a clash. A root moves the whole cue up the scale, which is
//! how a row further down sounds higher without becoming a different sound.
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
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32, whole_i32};

use crate::{Cue, Degree, Voice};

pub const RATE: u32 = 48_000;

const PENTATONIC: [i32; 5] = [0, 2, 4, 7, 9];

const ROOT_HERTZ: f64 = 261.63;

const QUIET: f64 = 0.001;

const LOUDEST: f64 = 32_767.0;

const MAX_CUE_SAMPLES: u32 = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderError;

impl std::fmt::Display for RenderError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      to,
      "the sound effect is longer than {MAX_CUE_SAMPLES} samples"
    )
  }
}

impl std::error::Error for RenderError {}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this wraps a frequency measured in cycles per second"
  )
)]
pub struct Hertz(pub f64);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this wraps the oscillator phase in turns"
  )
)]
struct Phase(f64);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Wave {
  Sine,
  Triangle,
  Sawtooth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "a partial has two dimensionless ratios: its pitch relative to the fundamental and its amplitude gain"
  )
)]
struct Partial {
  ratio: f64,
  gain: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "gain is a dimensionless amplitude multiplier"
  )
)]
struct Sounding {
  wave: Wave,
  attack: Duration,
  decay: Duration,
  lowpass: Hertz,
  gain: f64,
  partial: Option<Partial>,
}

fn voice_envelope(voice: Voice) -> Result<Sounding, Never> {
  Ok(match voice {
    Voice::Tap => Sounding {
      wave: Wave::Sine,
      attack: Duration::from_millis(5),
      decay: Duration::from_millis(250),
      lowpass: Hertz(3500.0),
      gain: 0.2,
      partial: None,
    },
    Voice::Bell => Sounding {
      wave: Wave::Triangle,
      attack: Duration::from_millis(6),
      decay: Duration::from_millis(400),
      lowpass: Hertz(6000.0),
      gain: 0.16,
      partial: Some(Partial {
        ratio: 2.0,
        gain: 0.35,
      }),
    },
    Voice::Bass => Sounding {
      wave: Wave::Sawtooth,
      attack: Duration::from_millis(20),
      decay: Duration::from_millis(400),
      lowpass: Hertz(900.0),
      gain: 0.2,
      partial: None,
    },
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
pub fn hertz(degree: Degree) -> Result<Hertz, Never> {
  let Ok(semitones) = semitones(degree);

  Ok(Hertz(ROOT_HERTZ * 2f64.powf(f64::from(semitones) / 12.0)))
}

fn wave(shape: Wave, Phase(turns): Phase) -> Result<f64, Never> {
  Ok(match shape {
    Wave::Sine => (TAU * turns).sin(),
    Wave::Triangle => FRAC_2_PI * (TAU * turns).sin().asin(),
    Wave::Sawtooth => 2.0 * (turns - (turns + 0.5).floor()),
  })
}

enum EnvelopePart {
  Attack,
  Decay,
}

impl From<bool> for EnvelopePart {
  fn from(in_attack: bool) -> Self {
    match bool::then_some(in_attack, ()) {
      Some(()) => Self::Attack,
      None => Self::Decay,
    }
  }
}

fn envelope(sound: Sounding, sample: u32) -> Result<f64, Never> {
  let seconds = f64::from(sample) / f64::from(RATE);
  let attack = sound.attack.as_secs_f64();
  let decay = sound.decay.as_secs_f64();

  Ok(match EnvelopePart::from(seconds < attack) {
    EnvelopePart::Attack => sound.gain * seconds / attack,
    EnvelopePart::Decay => sound.gain * (QUIET / sound.gain).powf((seconds - attack) / (decay - attack)),
  })
}

fn samples(span: Duration) -> Result<u32, Never> {
  toward_zero_u32(span.as_secs_f64() * f64::from(RATE))
}

fn note(sound: Sounding, pitch: Hertz) -> Result<impl Iterator<Item = f64>, Never> {
  let Ok(count) = samples(sound.decay);
  let smoothing = 1.0 - (-TAU * sound.lowpass.0 / f64::from(RATE)).exp();

  Ok((0..count).scan(0.0, move |held, sample| {
    let seconds = f64::from(sample) / f64::from(RATE);
    let Ok(fundamental) = wave(sound.wave, Phase(pitch.0 * seconds));

    let over = match sound.partial {
      Some(partial) => {
        let Ok(over) = wave(Wave::Sine, Phase(pitch.0 * partial.ratio * seconds));

        over * partial.gain
      }
      None => 0.0,
    };

    *held += smoothing * (fundamental + over - *held);

    let Ok(shaped) = envelope(sound, sample);

    Some(*held * shaped)
  }))
}

#[must_use]
pub fn render(cue: &Cue, root: Degree) -> Result<Vec<u8>, RenderError> {
  let Ok(sound) = voice_envelope(cue.voice);
  let last = cue
    .notes
    .iter()
    .map(|played| played.at)
    .fold(Duration::ZERO, Duration::max);
  let Ok(length) = samples(last.saturating_add(sound.decay));

  match length {
    0..=MAX_CUE_SAMPLES => {}
    _too_long => return Err(RenderError),
  }

  let Ok(sample_count) = index(length);
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the sample count was checked against MAX_CUE_SAMPLES before converting it to an index"
    )
  )]
  let mut mixed = vec![0.0; sample_count];

  for played in cue.notes {
    let Ok(pitch) = hertz(Degree(root.0.saturating_add(played.degree.0)));
    let Ok(from) = samples(played.at);
    let Ok(from_2) = index(from);
    let Ok(sounded) = note(sound, pitch);

    for (slot, sample) in mixed.iter_mut().skip(from_2).zip(sounded) {
      *slot += sample;
    }
  }

  Ok(
    mixed
      .iter()
      .flat_map(|sample| {
        let Ok(level) = whole_i32(sample.clamp(-1.0, 1.0) * LOUDEST);
        let Ok(level_2) = fitted::<i32, i16>(level);

        level_2.to_le_bytes()
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Note;
  use crate::catalogue::Sound;
  use std::error::Error;

  fn levels(bytes: &[u8]) -> Result<Vec<i16>, Never> {
    let (pairs, _) = bytes.as_chunks::<2>();

    Ok(pairs.iter().map(|pair| i16::from_le_bytes(*pair)).collect())
  }

  fn crossings(bytes: &[u8]) -> Result<u32, Never> {
    let Ok(levels) = levels(bytes);

    fitted(
      levels
        .iter()
        .zip(levels.iter().skip(1))
        .filter(|(before, after)| **before < 0 && **after >= 0)
        .count(),
    )
  }

  #[test]
  fn five_degrees_are_an_octave() {
    assert_eq!(semitones(Degree(0)), Ok(0));
    assert_eq!(semitones(Degree(3)), Ok(7));
    assert_eq!(semitones(Degree(5)), Ok(12));
    assert_eq!(semitones(Degree(-1)), Ok(-3));
  }

  #[test]
  fn degree_nothing_is_middle_c() {
    let Ok(Hertz(pitch)) = hertz(Degree(0));
    assert!((pitch - ROOT_HERTZ).abs() < 0.01);

    let Ok(Hertz(octave)) = hertz(Degree(5));
    assert!((octave - 2.0 * ROOT_HERTZ).abs() < 0.01);
  }

  #[test]
  fn a_cue_lasts_until_its_last_note_has_decayed() -> Result<(), RenderError> {
    const NOTES: &[Note] = &[
      Note {
        at: Duration::ZERO,
        degree: Degree(0),
      },
      Note {
        at: Duration::from_millis(100),
        degree: Degree(2),
      },
    ];

    let cue = Cue {
      voice: Voice::Tap,
      notes: NOTES,
    };
    let bytes = render(&cue, Degree(0))?;
    let Ok(sound) = voice_envelope(Voice::Tap);
    let Ok(expected) = samples(Duration::from_millis(100).saturating_add(sound.decay));

    assert_eq!(
      fitted::<_, u32>(bytes.len()),
      Ok(expected.saturating_mul(2))
    );

    Ok(())
  }

  #[test]
  fn a_cue_is_heard_and_then_is_silent() -> Result<(), Box<dyn Error>> {
    let Ok(step) = Sound::Step.cue();
    let bytes = render(&step, Degree(0))?;
    let Ok(levels) = levels(&bytes);
    let loudest = levels
      .iter()
      .map(|level| level.unsigned_abs())
      .max()
      .ok_or("no samples")?;
    let last = levels
      .last()
      .map(|level| level.unsigned_abs())
      .ok_or("no samples")?;

    assert!(loudest > 1000, "loudest was {loudest}");
    assert!(last < 100, "the tail ended at {last}");
    Ok(())
  }

  #[test]
  fn a_rooted_cue_sounds_higher() -> Result<(), RenderError> {
    let Ok(step) = Sound::Step.cue();
    let low_sound = render(&step, Degree(0))?;
    let high_sound = render(&step, Degree(5))?;
    let Ok(low) = crossings(&low_sound);
    let Ok(high) = crossings(&high_sound);

    assert!(
      high > low.saturating_add(low.div_euclid(2)),
      "{low} crossings against {high}"
    );

    Ok(())
  }

  #[test]
  fn a_cue_fits_in_a_pipe_so_writing_it_never_waits() -> Result<(), RenderError> {
    for voice in [Voice::Tap, Voice::Bell, Voice::Bass] {
      let cue = Cue {
        voice,
        notes: &[Note {
          at: Duration::ZERO,
          degree: Degree(0),
        }],
      };
      let bytes = render(&cue, Degree(0))?;

      assert!(bytes.len() < 65_536, "{voice:?} is {} bytes", bytes.len());
    }

    Ok(())
  }

  #[test]
  fn a_cue_past_the_sample_budget_is_refused() {
    const NOTES: &[Note] = &[Note {
      at: Duration::from_secs(2),
      degree: Degree(0),
    }];

    let cue = Cue {
      voice: Voice::Tap,
      notes: NOTES,
    };

    assert_eq!(render(&cue, Degree(0)), Err(RenderError));
  }
}
