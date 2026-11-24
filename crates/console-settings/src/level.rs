//! A level, drawn so it can be read across the room.
//!
//! A level somebody sets is a slider, in steps of `STEP` from nothing to the
//! whole, so the knob a press of right moves is the same step `apply_step`
//! takes; beside it the number is said, or the word for silence. A level that
//! is only read -- how strong a network is -- stays a run of cells, which is
//! a picture with nothing on it to press.

use std::num::NonZeroU32;

use console_core_controls::Slider;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32};
pub const FULL: char = '█';
pub const EMPTY: char = '░';
pub const CELLS: u32 = 8;

pub const STEP: i32 = 5;

pub const WHOLE: i32 = 100;

pub const SILENT: &str = "silent";

const STEPS: NonZeroU32 = match NonZeroU32::new(WHOLE.unsigned_abs() / STEP.unsigned_abs()) {
  Some(steps) => steps,
  None => NonZeroU32::MIN,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Muted {
  Yes,
  #[default]
  No,
}

impl From<bool> for Muted {
  fn from(muted: bool) -> Self {
    match muted {
      true => Self::Yes,
      false => Self::No,
    }
  }
}

pub fn bar(level: i32, muted: Muted, cells: u32) -> Result<String, Never> {
  let many = f64::from(cells);
  let Ok(filled) = toward_zero_u32((f64::from(level) / 100.0 * many).round().clamp(0.0, many));
  let Ok(full) = index(filled);
  let Ok(empty) = index(cells.saturating_sub(filled));
  let drawn: String = std::iter::repeat_n(FULL, full)
    .chain(std::iter::repeat_n(EMPTY, empty))
    .collect();

  match (cells < CELLS, muted) {
    (true, _) => Ok(drawn),
    (false, Muted::Yes) => Ok(format!("{drawn} {SILENT}")),
    (false, Muted::No) => Ok(format!("{drawn} {level}%")),
  }
}

pub fn said(level: i32, muted: Muted) -> Result<String, Never> {
  Ok(match muted {
    Muted::Yes => SILENT.to_string(),
    Muted::No => format!("{level}%"),
  })
}

pub fn slider(level: i32) -> Result<Slider, Never> {
  let nearest = level
    .clamp(0, WHOLE)
    .saturating_add(STEP.saturating_div(2))
    .saturating_div(STEP);
  let Ok(at) = fitted::<i32, u32>(nearest);

  Slider::new(at, STEPS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step(pub i32);

pub fn apply_step(level: i32, step: Step) -> Result<i32, Never> {
  Ok(
    level
      .saturating_add(step.0.saturating_mul(STEP))
      .clamp(0, 100),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_level_is_a_picture_and_a_number() {
    assert_eq!(bar(50, Muted::No, CELLS), Ok("████░░░░ 50%".to_string()));
    assert_eq!(bar(100, Muted::No, CELLS), Ok("████████ 100%".to_string()));
    assert_eq!(bar(0, Muted::No, CELLS), Ok("░░░░░░░░ 0%".to_string()));
  }

  #[test]
  fn silence_is_said_rather_than_drawn() {
    assert_eq!(said(50, Muted::Yes), Ok(SILENT.to_string()));
    assert_eq!(said(50, Muted::No), Ok("50%".to_string()));
  }

  #[test]
  fn a_slider_stands_on_the_step_a_press_of_right_would_take() {
    let Ok(half) = slider(50);
    let Ok(between) = slider(52);
    let Ok(over) = slider(140);
    let Ok(stepped) = apply_step(50, Step(1));
    let Ok(next) = slider(stepped);

    assert_eq!(half.at(), Ok(10));
    assert_eq!(
      between.at(),
      Ok(10),
      "a level between two steps stands on the nearer"
    );
    assert_eq!(over.at(), Ok(20), "a level past the whole is the whole");
    assert_eq!(next.at(), Ok(11));
    assert_eq!(half.most().map(NonZeroU32::get), Ok(20));
  }

  #[test]
  fn a_shorter_bar_is_a_reading_with_no_number_on_it() {
    assert_eq!(bar(100, Muted::No, 4), Ok("████".to_string()));
    assert_eq!(bar(50, Muted::No, 4), Ok("██░░".to_string()));
  }

  #[test]
  fn a_level_never_steps_past_either_end() {
    assert_eq!(apply_step(98, Step(1)), Ok(100));
    assert_eq!(apply_step(2, Step(-1)), Ok(0));
    assert_eq!(apply_step(50, Step(1)), Ok(50_i32.saturating_add(STEP)));
  }
}
