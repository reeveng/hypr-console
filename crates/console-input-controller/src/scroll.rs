//! Turning the right stick into a scroll wheel.
//!
//! InputPlumber can map a stick to a wheel notch, but an axis crossing its
//! deadzone is one press and one release, and a wheel notch does not repeat
//! while it is held. One flick gave one imperceptible notch. Arrow keys
//! repeat, but in a terminal an arrow key is command history, not scrolling.
//!
//! So the stick stays an axis, and this turns how far it is pushed into how
//! fast the wheel turns.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;
use console_input_event_devices::RelativeAxisCode;
use console_input_gamepad::axis::{self, Range};

use crate::clock::Seconds;
use crate::effect::Output;

pub const DEADZONE: f64 = 0.20;

pub const MAX_HZ: f64 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thumbstick {
  pub value: i32,
  pub range: Range,
}

pub fn pushed(stick: Thumbstick) -> Result<f64, Never> {
  let Thumbstick { value, range } = stick;
  let Ok(part) = axis::part(value, range);

  match ::core::primitive::bool::then_some(part.abs() < DEADZONE, ()) {
    ::core::option::Option::Some(()) => return Ok(0.0),
    ::core::option::Option::None => {}
  }

  let past = (part.abs() - DEADZONE) / (1.0 - DEADZONE);

  Ok(match ::core::primitive::bool::then_some(part > 0.0, ()) {
    ::core::option::Option::Some(()) => past * past,
    ::core::option::Option::None => -(past * past),
  })
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the inner value is a signed fractional count of wheel notches"
  )
)]
pub struct WheelNotches(pub f64);

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Wheel {
  pub x: WheelNotches,
  pub y: WheelNotches,
}

impl Wheel {
  pub fn scroll(&mut self, by: Point<f64>, seconds: Seconds) -> Result<Vec<Output>, Never> {
    self.y.0 += -by.y * MAX_HZ * seconds.0;
    self.x.0 += by.x * MAX_HZ * seconds.0;
    let Ok(down) = notches(&mut self.y, RelativeAxisCode::REL_WHEEL.0);
    let Ok(across) = notches(&mut self.x, RelativeAxisCode::REL_HWHEEL.0);

    Ok(down.into_iter().chain(across).collect())
  }
}

fn notches(amount: &mut WheelNotches, code: u16) -> Result<Vec<Output>, Never> {
  let whole = amount.0.trunc();

  amount.0 -= whole;

  let step = match ::core::primitive::bool::then_some(whole > 0.0, ()) {
    ::core::option::Option::Some(()) => 1,
    ::core::option::Option::None => -1,
  };
  let Ok(many) = toward_zero_u32(whole.abs());
  let Ok(out) = Output::relative(code, step);

  Ok((0..many).map(|_| out).collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  const EITHER_SIDE: Range = Range {
    low: -32767,
    high: 32767,
  };

  #[test]
  fn a_stick_at_rest_is_at_rest() {
    assert_eq!(
      pushed(Thumbstick {
        value: 0,
        range: EITHER_SIDE
      }),
      Ok(0.0)
    );
    assert_eq!(
      pushed(Thumbstick {
        value: 6000,
        range: EITHER_SIDE
      }),
      Ok(0.0),
      "inside the deadzone"
    );
  }

  #[test]
  fn a_stick_that_rests_in_the_middle_of_a_range_from_zero_does_not_scroll() {
    assert_eq!(
      pushed(Thumbstick {
        value: 128,
        range: Range { low: 0, high: 255 }
      }),
      Ok(0.0)
    );
  }

  #[test]
  fn a_stick_pushed_all_the_way_is_all_the_way() {
    let Ok(right) = pushed(Thumbstick {
      value: 32767,
      range: EITHER_SIDE,
    });
    let Ok(left) = pushed(Thumbstick {
      value: -32767,
      range: EITHER_SIDE,
    });

    assert!((right - 1.0).abs() < 1e-12);
    assert!((left + 1.0).abs() < 1e-12);
  }

  #[test]
  fn a_small_push_is_slower_than_its_share() {
    let Ok(half) = pushed(Thumbstick {
      value: 16383,
      range: EITHER_SIDE,
    });
    assert!(half > 0.0 && half < 0.5, "half a push is {half}");
  }

  #[test]
  fn a_stick_held_for_a_second_turns_the_wheel_as_far_as_the_arithmetic_says() {
    let mut wheel = Wheel::default();
    let mut notches: Vec<Output> = Vec::new();

    for _ in 0..50 {
      let Ok(turned) = wheel.scroll(Point { x: 0.0, y: -1.0 }, Seconds(0.02));

      notches.extend(turned);
    }

    let Ok(turned) = console_core_number_conversion::fitted::<_, u32>(notches.len());
    let Ok(most) = console_core_number_conversion::toward_zero_u32(MAX_HZ);

    assert_eq!(turned.saturating_add(u32::from(wheel.y.0 >= 0.5)), most);
    assert!(wheel.y.0 < 1.0, "nothing whole is left unturned");
    assert!(
      notches
        .iter()
        .all(|out| out.value == 1 && out.code == RelativeAxisCode::REL_WHEEL.0)
    );
  }

  #[test]
  fn pushing_up_scrolls_up_and_pushing_down_scrolls_down() {
    let mut wheel = Wheel::default();
    let Ok(up) = wheel.scroll(Point { x: 0.0, y: -1.0 }, Seconds(1.0));
    let mut other = Wheel::default();
    let Ok(down) = other.scroll(Point { x: 0.0, y: 1.0 }, Seconds(1.0));

    assert_eq!(up.first().map(|out| out.value), Some(1));
    assert_eq!(down.first().map(|out| out.value), Some(-1));
  }

  #[test]
  fn what_is_owed_is_kept_until_it_is_a_whole_notch() {
    let mut wheel = Wheel::default();
    let Ok(first) = wheel.scroll(Point { x: 0.0, y: -0.1 }, Seconds(0.02));

    assert!(first.is_empty());

    let mut over: Vec<Output> = Vec::new();

    for _ in 0..100 {
      let Ok(turned) = wheel.scroll(Point { x: 0.0, y: -0.1 }, Seconds(0.02));

      over.extend(turned);
    }

    assert!(!over.is_empty(), "a slow push still scrolls, eventually");
  }
}
