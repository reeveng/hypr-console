//! Pointer selection uses the same grapheme positions in fields and text areas.

use std::time::{Duration, Instant};

use console_core_geometry::Point;
use console_core_never::Never;

#[derive(Default)]
pub struct Gesture {
  phase: Phase,
  previous: Option<Click>,
}

#[derive(Default)]
enum Phase {
  #[default]
  Released,
  Pressed,
}

struct Click {
  at: Point<i32>,
  when: Instant,
}

#[derive(Clone, Copy)]
pub enum Selection {
  Place,
  Word,
  Extend,
}

impl Gesture {
  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    expect(
      explicit039_no_reading_the_clock,
      reason = "the pointer event boundary observes the time of the click"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit077_no_ambient_randomness",
    expect(
      explicit077_no_ambient_randomness,
      reason = "a pointer click is timestamped at the event boundary before the gesture is interpreted"
    )
  )]
  pub fn press_now(&mut self, at: Point<i32>) -> Result<Selection, Never> {
    self.pressed(at, Instant::now())
  }

  fn pressed(&mut self, at: Point<i32>, when: Instant) -> Result<Selection, Never> {
    let repeated = self.previous.take().is_some_and(|previous| {
      when.saturating_duration_since(previous.when) <= Duration::from_millis(500)
        && at.x.abs_diff(previous.at.x) <= 4
        && at.y.abs_diff(previous.at.y) <= 4
    });
    self.phase = Phase::Pressed;

    Ok(match repeated.then_some(()) {
      Some(()) => Selection::Word,
      None => {
        self.previous = Some(Click { at, when });
        Selection::Place
      }
    })
  }

  pub fn moved(&mut self, at: Point<i32>) -> Result<Option<Selection>, Never> {
    match self.phase {
      Phase::Released => return Ok(None),
      Phase::Pressed => {}
    }

    match self.previous.as_ref() {
      Some(click) => {
        match (click.at.x.abs_diff(at.x) > 4 || click.at.y.abs_diff(at.y) > 4).then_some(()) {
          Some(()) => self.previous = None,
          None => return Ok(None),
        }
      }
      None => {}
    }

    Ok(Some(Selection::Extend))
  }

  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    expect(
      explicit039_no_reading_the_clock,
      reason = "the pointer event boundary observes when the hold ended"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit077_no_ambient_randomness",
    expect(
      explicit077_no_ambient_randomness,
      reason = "the release time belongs to the pointer event boundary"
    )
  )]
  pub fn release_now(&mut self) -> Result<Option<Point<i32>>, Never> {
    let phase = std::mem::take(&mut self.phase);
    let long = match (&self.previous, phase) {
      (Some(press), Phase::Pressed) => (Instant::now().saturating_duration_since(press.when)
        >= Duration::from_millis(650))
      .then_some(press.at),
      _ => None,
    };

    match long {
      Some(_) => self.previous = None,
      None => {}
    }

    Ok(long)
  }

  pub fn cancel(&mut self) -> Result<(), Never> {
    *self = Self::default();
    Ok(())
  }
}
