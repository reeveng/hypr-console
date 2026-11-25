//! Repeat the latest held key or control from the time its first repeat is due.
//! A wait that falls behind the cadence emits at most one repeat.

use std::time::{Duration, Instant};

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cadence {
  pub delay: Duration,
  pub interval: Duration,
}

#[cfg(feature = "surface")]
pub(crate) struct RepeatInfo {
  pub rate: i32,
  pub delay: i32,
}

#[cfg(feature = "surface")]
impl Cadence {
  pub(crate) fn from_repeat_info(info: RepeatInfo) -> Result<Option<Self>, Never> {
    let (rate, delay) = match (info.rate, info.delay) {
      (1.., 0..) => (info.rate.unsigned_abs(), info.delay.unsigned_abs()),
      _ => return Ok(None),
    };

    Ok(
      1_000_000_000_u64
        .checked_div(u64::from(rate))
        .map(|nanos| Self {
          delay: Duration::from_millis(u64::from(delay)),
          interval: Duration::from_nanos(nanos.max(1)),
        }),
    )
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
  New,
  AlreadyDown,
}

#[derive(Debug, Clone, Copy)]
struct Pressed<S, T> {
  source: S,
  key: T,
  cadence: Cadence,
  due: Instant,
}

#[derive(Debug)]
pub struct KeyRepeat<S, T> {
  pressed: Option<Pressed<S, T>>,
}

impl<S, T> Default for KeyRepeat<S, T> {
  fn default() -> Self {
    KeyRepeat { pressed: None }
  }
}

impl<S: Copy + PartialEq, T: Copy + PartialEq> KeyRepeat<S, T> {
  pub fn pressed(
    &mut self,
    source: S,
    key: T,
    cadence: Cadence,
    now: Instant,
  ) -> Result<Press, Never> {
    let again = self
      .pressed
      .is_some_and(|pressed| pressed.source == source && pressed.key == key);

    Ok(match ::core::primitive::bool::then_some(again, ()) {
      ::core::option::Option::Some(()) => Press::AlreadyDown,
      ::core::option::Option::None => {
        self.pressed = now.checked_add(cadence.delay).map(|due| Pressed {
          source,
          key,
          cadence,
          due,
        });

        Press::New
      }
    })
  }

  pub fn released(&mut self, source: S) -> Result<(), Never> {
    let started_it = self.pressed.is_some_and(|pressed| pressed.source == source);

    match ::core::primitive::bool::then_some(started_it, ()) {
      ::core::option::Option::Some(()) => self.pressed = None,
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub fn stop(&mut self) -> Result<(), Never> {
    self.pressed = None;

    Ok(())
  }

  pub fn until(&self, now: Instant) -> Result<Option<Duration>, Never> {
    Ok(
      self
        .pressed
        .map(|pressed| pressed.due.saturating_duration_since(now)),
    )
  }

  pub fn due(&mut self, now: Instant) -> Result<Option<T>, Never> {
    let pressed = match self.pressed.as_mut() {
      Some(pressed) => pressed,
      None => return Ok(None),
    };

    match ::core::primitive::bool::then_some(now < pressed.due, ()) {
      ::core::option::Option::Some(()) => return Ok(None),
      ::core::option::Option::None => {}
    }

    let key = pressed.key;
    let counted = pressed.due.checked_add(pressed.cadence.interval);
    let from_now = now.checked_add(pressed.cadence.interval);

    let next = match counted {
      Some(on_time) => match ::core::primitive::bool::then_some(on_time > now, ()) {
        ::core::option::Option::Some(()) => Some(on_time),
        ::core::option::Option::None => from_now,
      },
      None => from_now,
    };

    match next {
      Some(then) => pressed.due = then,
      None => self.pressed = None,
    }

    Ok(Some(key))
  }
}

#[cfg(all(test, feature = "surface"))]
mod tests {
  use super::*;

  #[test]
  fn the_compositor_controls_the_hold_delay_and_rate() {
    assert_eq!(
      Cadence::from_repeat_info(RepeatInfo {
        rate: 25,
        delay: 300
      }),
      Ok(Some(Cadence {
        delay: Duration::from_millis(300),
        interval: Duration::from_millis(40),
      }))
    );

    for (rate, delay) in [(0, 300), (-1, 300), (25, -1)] {
      assert_eq!(
        Cadence::from_repeat_info(RepeatInfo { rate, delay }),
        Ok(None)
      );
    }
  }
}
