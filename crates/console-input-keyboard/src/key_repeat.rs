//! Repeat settings for the on-screen keyboard and its controller navigation.
//! The keyboard sends each character as a tap through the virtual keyboard protocol.

use std::time::Duration;

pub use console_wayland::key_repeat::{Cadence, KeyRepeat, Press};

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

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Instant;

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
