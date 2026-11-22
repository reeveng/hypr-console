//! A key held down: once as it goes down, then -- after a delay long enough
//! that a tap is not taken for a hold -- again at one interval for as long as
//! it stays down. The keyboard repeats rather than the compositor because a
//! press leaves here as a tap, down and up at once, and a key that is never
//! down for long is a key no client repeats.
//!
//! The interval is counted from when the last repeat was due, not from when
//! the loop got round to it. Counted from the loop, every turn that woke a few
//! milliseconds late pushed every repeat after it later too, and the period was
//! the interval plus however long the loop happened to take. A turn that fell
//! more than a whole interval behind starts counting again from now rather than
//! typing what it missed all at once.
//!
//! A cadence per thing that repeats. A direction walks the selection, and a
//! repeat nobody meant costs a step back, so it starts soon. A key types, and a
//! repeat nobody meant is a letter to delete, so it waits a second -- longer
//! than a thumb rests on a key it meant to tap.
//!
//! One thing repeats at a time, which is what every keyboard does: a second key
//! takes the repeat from the first, and letting go of the first afterwards stops
//! nothing. What stops a repeat is what started it going back up, so the press
//! remembers where it came from and a release from anywhere else is somebody
//! else's.

use std::time::{Duration, Instant};

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cadence {
  pub delay: Duration,
  pub interval: Duration,
}

pub const MOVING: Cadence = Cadence {
  delay: Duration::from_millis(350),
  interval: Duration::from_millis(90),
};

pub const TYPING: Cadence = Cadence {
  delay: Duration::from_secs(1),
  interval: Duration::from_millis(90),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
  Repeating,
  Once,
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

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  const LATE: Duration = Duration::from_millis(7);

  const EARLY: Duration = Duration::from_millis(1);

  fn after(start: Instant, by: Duration) -> Result<Instant, Failure> {
    let then = start
      .checked_add(by)
      .ok_or("an instant past the end of the clock")?;

    Ok(then)
  }

  fn before(start: Instant, by: Duration) -> Result<Instant, Failure> {
    let then = start
      .checked_sub(by)
      .ok_or("an instant before the clock began")?;

    Ok(then)
  }

  fn times(by: Duration, count: u32) -> Result<Duration, Failure> {
    let long = by
      .checked_mul(count)
      .ok_or("a duration longer than the clock holds")?;

    Ok(long)
  }

  #[test]
  fn a_held_key_waits_the_delay_then_repeats_at_the_interval() -> Result<(), Failure> {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let first = after(start, TYPING.delay)?;
    let second = after(first, TYPING.interval)?;
    let third = after(second, TYPING.interval)?;
    let held_briefly = before(first, EARLY)?;

    assert_eq!(repeat.pressed(0, 'a', TYPING, start), Ok(Press::New));
    assert_eq!(
      repeat.due(start),
      Ok(None),
      "the press itself was the first one"
    );
    assert_eq!(repeat.due(held_briefly), Ok(None), "a tap is not a hold");
    assert_eq!(repeat.due(first), Ok(Some('a')), "the first repeat");
    assert_eq!(repeat.due(first), Ok(None), "and not again at once");
    assert_eq!(repeat.due(second), Ok(Some('a')));
    assert_eq!(repeat.due(third), Ok(Some('a')));

    Ok(())
  }

  #[test]
  fn a_turn_that_woke_late_does_not_make_every_repeat_after_it_late() -> Result<(), Failure> {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let first = after(start, TYPING.delay)?;
    let woke = after(first, LATE)?;
    let left = TYPING
      .interval
      .checked_sub(LATE)
      .ok_or("an interval shorter than a late wake")?;
    let second = after(first, TYPING.interval)?;
    let Ok(_) = repeat.pressed(0, 'a', TYPING, start);

    assert_eq!(repeat.due(woke), Ok(Some('a')), "a few milliseconds late");
    assert_eq!(
      repeat.until(woke),
      Ok(Some(left)),
      "the wait is what was left of the interval"
    );
    assert_eq!(
      repeat.due(second),
      Ok(Some('a')),
      "the second repeat is one interval after the first was due, whenever the first was typed"
    );

    Ok(())
  }

  #[test]
  fn a_turn_that_fell_behind_types_once_rather_than_everything_it_missed() -> Result<(), Failure>
  {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let first = after(start, TYPING.delay)?;
    let missed = times(TYPING.interval, 10)?;
    let behind = after(first, missed)?;
    let woke = after(behind, LATE)?;
    let next = after(woke, TYPING.interval)?;
    let Ok(_) = repeat.pressed(0, 'a', TYPING, start);

    assert_eq!(repeat.due(woke), Ok(Some('a')));
    assert_eq!(
      repeat.due(woke),
      Ok(None),
      "not a burst of the ten it missed"
    );
    assert_eq!(
      repeat.due(next),
      Ok(Some('a')),
      "and counting again from now"
    );

    Ok(())
  }

  #[test]
  fn letting_go_stops_the_repeat() -> Result<(), Failure> {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let long = times(TYPING.delay, 4)?;
    let much_later = after(start, long)?;
    let Ok(_) = repeat.pressed(0, 'a', TYPING, start);

    assert_eq!(repeat.released(0), Ok(()));
    assert_eq!(repeat.due(much_later), Ok(None));
    assert_eq!(repeat.until(start), Ok(None), "and nothing to wake for");

    Ok(())
  }

  #[test]
  fn letting_go_of_something_else_stops_nothing() -> Result<(), Failure> {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let first = after(start, TYPING.delay)?;
    let Ok(_) = repeat.pressed(0, 'a', TYPING, start);

    assert_eq!(repeat.released(1), Ok(()));
    assert_eq!(
      repeat.due(first),
      Ok(Some('a')),
      "a release from another button is that button's"
    );

    Ok(())
  }

  #[test]
  fn the_same_press_again_is_not_a_new_one_and_does_not_start_the_wait_over()
  -> Result<(), Failure> {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let again = after(start, LATE)?;
    let first = after(start, MOVING.delay)?;
    let Ok(_) = repeat.pressed(0, 'a', MOVING, start);

    assert_eq!(
      repeat.pressed(0, 'a', MOVING, again),
      Ok(Press::AlreadyDown)
    );
    assert_eq!(
      repeat.due(first),
      Ok(Some('a')),
      "a thumbstick that reports the same push again is one push"
    );

    Ok(())
  }

  #[test]
  fn a_second_key_takes_the_repeat_from_the_first() -> Result<(), Failure> {
    let start = Instant::now();
    let mut repeat: KeyRepeat<u8, char> = KeyRepeat::default();
    let later = after(start, LATE)?;
    let firsts = after(start, TYPING.delay)?;
    let second = after(later, TYPING.delay)?;
    let after_that = after(second, TYPING.interval)?;
    let Ok(_) = repeat.pressed(0, 'a', TYPING, start);

    assert_eq!(repeat.pressed(1, 'b', TYPING, later), Ok(Press::New));
    assert_eq!(
      repeat.due(firsts),
      Ok(None),
      "the wait is the second key's own"
    );
    assert_eq!(repeat.due(second), Ok(Some('b')));
    assert_eq!(repeat.released(0), Ok(()));
    assert_eq!(
      repeat.due(after_that),
      Ok(Some('b')),
      "letting go of the first stops nothing"
    );

    Ok(())
  }
}
