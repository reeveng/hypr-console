//! A film at another speed, with its voices left where they were.
//!
//! Samples played faster raise everything in them, which is a film at twice
//! its speed spoken by somebody who breathed helium. ffmpeg's `atempo` was
//! what kept the pitch, and this is the same idea written here:
//! waveform-similarity overlap-add. The output is laid down in short windows
//! that overlap by half; each one is taken from the input where the tempo says
//! it should be, give or take a few milliseconds, and those milliseconds are
//! spent finding the spot whose shape best carries on from the window before
//! it. Two waves added in step make the sound they were; two out of step
//! cancel into the flutter a plain cut-and-splice is known for.
//!
//! Twenty milliseconds of overlap and ten of search are the figures speech
//! wants, and a film is mostly speech. A song is never stretched at all: a
//! tempo of one never reaches this file, so the music player hears the samples
//! it decoded and nothing else.

use crate::decoding::Format;
use crate::sounding::Tempo;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, whole_u32, whole_u64};
use std::ops::RangeInclusive;

const OVERLAP: f64 = 0.02;

const SEARCH: f64 = 0.01;

pub struct Stretching {
  channels: u32,
  tempo: Tempo,
  half: u32,
  reach: u32,
  rising: Vec<f32>,
  held: Vec<f32>,
  origin: u64,
  steps: u64,
  tail: Vec<f32>,
}

impl Stretching {
  pub fn new(format: Format, tempo: Tempo) -> Result<Stretching, Never> {
    let channels = format.channels.max(1);
    let Ok(half) = whole_u32(f64::from(format.rate) * OVERLAP);
    let Ok(reach) = whole_u32(f64::from(format.rate) * SEARCH);
    let half_2 = half.max(1);

    Ok(Stretching {
      channels,
      tempo,
      half: half_2,
      reach,
      rising: Vec::new(),
      held: Vec::new(),
      origin: 0,
      steps: 0,
      tail: Vec::new(),
    })
  }

  pub fn stretched(&mut self, samples: &[f32]) -> Result<Vec<f32>, Never> {
    self.held.extend_from_slice(samples);

    let laid = console_core_iteration::iterate((self, Vec::new()), |(stretching, mut laid)| {
      let Ok(step) = stretching.step();

      Ok(match step {
        Some(piece) => {
          laid.extend(piece);

          Step::Again((stretching, laid))
        }
        None => Step::Halt(laid),
      })
    });

    Ok(match laid {
      Ok(laid_2) => laid_2,
      Err(_endless) => Vec::new(),
    })
  }

  fn step(&mut self) -> Result<Option<Vec<f32>>, Never> {
    let Ok(nominal) = self.nominal(self.steps);
    let lowest = nominal
      .saturating_sub(u64::from(self.reach))
      .max(self.origin);
    let highest = nominal.saturating_add(u64::from(self.reach));
    let needed = highest.saturating_add(u64::from(self.half).saturating_mul(2));
    let Ok(held) = self.held_frames();

    match ::core::primitive::bool::then_some(self.origin.saturating_add(held) >= needed, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    }

    match bool::then_some(self.tail.is_empty(), ()) {
      Some(()) => {
        let Ok(initial) = self.frames(self.origin, self.half);

        self.tail = match initial {
          Some(samples) => samples.iter().map(|_| 0.0).collect(),
          None => return Ok(None),
        };
        let Ok(ramp) = rising(self.half);

        self.rising = ramp;
      }
      None => {}
    }

    let Ok(start) = self.best(lowest..=highest);
    let Ok(first) = self.frames(start, self.half);
    let Ok(second) = self.frames(start.saturating_add(u64::from(self.half)), self.half);

    let (first_3, second_3) = match (first, second) {
      (Some(first_2), Some(second_2)) => (first_2, second_2.to_vec()),
      (None, _) | (_, None) => return Ok(None),
    };

    let Ok(laid) = self.crossfaded(first_3);

    self.tail = second_3;
    self.steps = self.steps.saturating_add(1);

    let Ok(next) = self.nominal(self.steps);
    let Ok(()) = self.forget(next.saturating_sub(u64::from(self.reach)));

    Ok(Some(laid))
  }

  fn nominal(&self, steps: u64) -> Result<u64, Never> {
    let Ok(steps_2) = steps.float();

    whole_u64(steps_2 * f64::from(self.half) * self.tempo.0)
  }

  fn held_frames(&self) -> Result<u64, Never> {
    let Ok(samples) = fitted::<_, u64>(self.held.len());

    Ok(match samples.checked_div(u64::from(self.channels)) {
      Some(frames) => frames,
      None => 0,
    })
  }

  fn frames(&self, from: u64, count: u32) -> Result<Option<&[f32]>, Never> {
    let Ok(at) = index(
      from
        .saturating_sub(self.origin)
        .saturating_mul(u64::from(self.channels)),
    );
    let Ok(length) = index(count.saturating_mul(self.channels));

    Ok(self.held.get(at..at.saturating_add(length)))
  }

  fn best(&self, among: RangeInclusive<u64>) -> Result<u64, Never> {
    let lowest = *among.start();
    let scored = among.filter_map(|start| {
      let Ok(likeness) = self.likeness(start);

      likeness.map(|likeness_2| (start, likeness_2))
    });

    Ok(
      match scored.max_by(|one, other| one.1.total_cmp(&other.1)) {
        Some((start, _likeness)) => start,
        None => lowest,
      },
    )
  }

  fn likeness(&self, start: u64) -> Result<Option<f32>, Never> {
    let Ok(candidate) = self.frames(start, self.half);

    let candidate_3 = match candidate {
      Some(candidate_2) => candidate_2,
      None => return Ok(None),
    };

    let Ok(width) = index(self.channels);
    let (together, energy) = self
      .tail
      .chunks_exact(width.max(1))
      .zip(candidate_3.chunks_exact(width.max(1)))
      .map(|(was, could)| (was.iter().sum::<f32>(), could.iter().sum::<f32>()))
      .fold((0.0_f32, 0.0_f32), |(together, energy), (was, could)| {
        (together + was * could, energy + could * could)
      });

    Ok(Some(together / (energy.sqrt() + f32::EPSILON)))
  }

  fn crossfaded(&self, coming: &[f32]) -> Result<Vec<f32>, Never> {
    let Ok(width) = index(self.channels);
    let prior = self.tail.chunks_exact(width.max(1));
    let arriving = coming.chunks_exact(width.max(1));

    Ok(
      self
        .rising
        .iter()
        .zip(prior.zip(arriving))
        .flat_map(|(rise, (going, coming_2))| {
          going
            .iter()
            .zip(coming_2)
            .map(move |(going_2, coming_3)| going_2 * (1.0 - rise) + coming_3 * rise)
        })
        .collect(),
    )
  }

  fn forget(&mut self, before: u64) -> Result<(), Never> {
    let Ok(held) = self.held_frames();
    let until = before.min(self.origin.saturating_add(held));
    let Ok(gone) = index(
      until
        .saturating_sub(self.origin)
        .saturating_mul(u64::from(self.channels)),
    );

    self.held.drain(..gone.min(self.held.len()));
    self.origin = self.origin.max(until);

    Ok(())
  }
}

fn rising(half: u32) -> Result<Vec<f32>, Never> {
  let Ok(length) = fitted::<_, u16>(half);
  let length_2 = f32::from(length);

  Ok(
    (0..half)
      .map(|frame| {
        let Ok(frame_2) = fitted::<_, u16>(frame);
        let along = (f32::from(frame_2) + 0.5) / length_2;
        let sine = (along * std::f32::consts::FRAC_PI_2).sin();

        sine * sine
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  const RATE: u32 = 8_000;

  fn a_tone(frames: u16) -> Result<Vec<f32>, Never> {
    let rate = f32::from(8_000_u16);

    Ok(
      (0..frames)
        .map(|frame| (std::f32::consts::TAU * 200.0 * f32::from(frame) / rate).sin() * 0.5)
        .collect(),
    )
  }

  fn cycles(samples: &[f32]) -> Result<u32, Never> {
    let rising = samples
      .iter()
      .zip(samples.iter().skip(1))
      .filter(|(before, after)| **before < 0.0 && **after >= 0.0);
    let Ok(count) = fitted::<_, u32>(rising.count());

    Ok(count)
  }

  fn played(tempo: Tempo, tone: &[f32]) -> Result<Vec<f32>, Never> {
    let Ok(mut stretching) = Stretching::new(
      Format {
        rate: RATE,
        channels: 1,
      },
      tempo,
    );
    let mut laid = Vec::new();

    for chunk in tone.chunks(500) {
      let Ok(piece) = stretching.stretched(chunk);

      laid.extend(piece);
    }

    Ok(laid)
  }

  #[test]
  fn at_twice_the_speed_a_second_lasts_half_as_long_and_sounds_the_same_note() {
    let Ok(tone) = a_tone(8_000);
    let Ok(laid) = played(Tempo(2.0), &tone);
    let Ok(heard) = fitted::<_, u32>(laid.len());
    let Ok(cycles) = cycles(&laid);

    assert!(
      heard.abs_diff(4_000) < 400,
      "a second at twice the speed lasted {heard} samples"
    );
    assert!(
      cycles.abs_diff(100) < 12,
      "{cycles} cycles in half a second is not two hundred hertz"
    );
  }

  #[test]
  fn at_half_the_speed_a_second_lasts_twice_as_long_and_sounds_the_same_note() {
    let Ok(tone) = a_tone(8_000);
    let Ok(laid) = played(Tempo(0.5), &tone);
    let Ok(heard) = fitted::<_, u32>(laid.len());
    let Ok(cycles) = cycles(&laid);

    assert!(
      heard.abs_diff(16_000) < 800,
      "a second at half the speed lasted {heard} samples"
    );
    assert!(
      cycles.abs_diff(400) < 24,
      "{cycles} cycles in two seconds is not two hundred hertz"
    );
  }
}
