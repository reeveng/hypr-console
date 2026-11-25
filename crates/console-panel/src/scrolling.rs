//! Preserve the subpixel part of wheel motion across input turns.

use console_core_never::Never;
use console_core_number_conversion::toward_zero_i32;
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};

pub struct Wheel;

#[derive(Clone, Copy, Default)]
pub struct Position(Offset);

#[derive(Clone, Copy, Default)]
enum Offset {
  #[default]
  Ready,
  BetweenPixels {
    at: i32,
    fraction: f64,
  },
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "wheel input distances are in logical pixels"
  )
)]
pub struct LogicalPixels(pub f64);

pub struct Motion {
  pub at: i32,
  pub by: LogicalPixels,
  pub limit: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ScrollTo(pub i32);

impl EffectDependencies for ScrollTo {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Display])
  }
}

impl SingleEffectMachine for Wheel {
  type Input = ();
  type State = Position;
  type Request = Motion;
  type Effect = ScrollTo;

  fn initialize(
    (): &(),
    _previous: Option<Position>,
  ) -> Result<SingleTransition<Position, ScrollTo>, Never> {
    Ok(SingleTransition {
      state: Position::default(),
      effect: None,
    })
  }

  fn handle(
    state: Position,
    Motion {
      at,
      by: LogicalPixels(by),
      limit,
    }: Motion,
  ) -> Result<SingleTransition<Position, ScrollTo>, Never> {
    match by.is_finite().then_some(()) {
      Some(()) => {}
      None => {
        return Ok(SingleTransition {
          state,
          effect: None,
        });
      }
    }

    let fraction = match state.0 {
      Offset::BetweenPixels {
        at: previous,
        fraction,
      } => match (previous == at).then_some(()) {
        Some(()) => fraction,
        None => 0.0,
      },
      Offset::Ready => 0.0,
    };
    let next = (f64::from(at) + (fraction + by))
      .max(0.0)
      .min(f64::from(limit.max(0)));
    let Ok(y) = toward_zero_i32(next);

    Ok(SingleTransition {
      state: Position(Offset::BetweenPixels {
        at: y,
        fraction: next - f64::from(y),
      }),
      effect: (y != at).then_some(ScrollTo(y)),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn small_movements_accumulate_and_reverse() {
    let Ok(first) = Wheel::handle(
      Position::default(),
      Motion {
        at: 5,
        by: LogicalPixels(0.75),
        limit: 10,
      },
    );
    assert_eq!(first.effect, None);

    let Ok(second) = Wheel::handle(
      first.state,
      Motion {
        at: 5,
        by: LogicalPixels(0.75),
        limit: 10,
      },
    );
    assert_eq!(second.effect, Some(ScrollTo(6)));

    let Ok(back) = Wheel::handle(
      second.state,
      Motion {
        at: 6,
        by: LogicalPixels(-0.75),
        limit: 10,
      },
    );
    assert_eq!(back.effect, Some(ScrollTo(5)));
  }

  #[test]
  fn the_end_does_not_keep_overscroll_to_pay_back() {
    let Ok(end) = Wheel::handle(
      Position::default(),
      Motion {
        at: 10,
        by: LogicalPixels(9.0),
        limit: 10,
      },
    );
    assert_eq!(end.effect, None);

    let Ok(back) = Wheel::handle(
      end.state,
      Motion {
        at: 10,
        by: LogicalPixels(-1.0),
        limit: 10,
      },
    );
    assert_eq!(back.effect, Some(ScrollTo(9)));
  }
}
