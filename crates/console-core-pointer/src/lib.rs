//! Pointer input, cursor shapes, and hover transitions for Console controls.
//!
//! The caller resolves a position against its current layout. A changed target
//! requests one repaint without changing keyboard or controller focus.

#![no_std]
#![forbid(unsafe_code)]

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerEvent {
  Down { at: (f64, f64) },
  Moved { at: (f64, f64) },
  Scrolled { by: f64 },
  Pinched { by: f64 },
  Up,
  Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerCursor {
  Default,
  Hand,
  Text,
}

pub struct Hover<T>(Option<T>);

impl<T> Default for Hover<T> {
  fn default() -> Self {
    Self(None)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepaintHover;

impl EffectDependencies for RepaintHover {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Display])
  }
}

impl<T: Copy + PartialEq> Hover<T> {
  pub fn target(&self) -> Result<Option<T>, Never> {
    Ok(self.0)
  }

  pub fn observe(&mut self, target: Option<T>) -> Result<Option<RepaintHover>, Never> {
    let Ok(next) = Self::handle(Self(self.0), target);
    *self = next.state;

    Ok(next.effect)
  }
}

impl<T: PartialEq> SingleEffectMachine for Hover<T> {
  type Input = ();
  type State = Self;
  type Request = Option<T>;
  type Effect = RepaintHover;

  fn initialize(_: &(), _: Option<Self>) -> Result<SingleTransition<Self, RepaintHover>, Never> {
    Ok(SingleTransition {
      state: Self::default(),
      effect: None,
    })
  }

  fn handle(
    state: Self,
    target: Option<T>,
  ) -> Result<SingleTransition<Self, RepaintHover>, Never> {
    Ok(SingleTransition {
      effect: (state.0 != target).then_some(RepaintHover),
      state: Self(target),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_target_changes_request_a_repaint() {
    let mut hover = Hover::default();
    assert_eq!(hover.observe(None), Ok(None));
    assert_eq!(hover.observe(Some("workspace")), Ok(Some(RepaintHover)));
    assert_eq!(hover.observe(Some("workspace")), Ok(None));
    assert_eq!(hover.observe(Some("sound")), Ok(Some(RepaintHover)));
    assert_eq!(hover.observe(None), Ok(Some(RepaintHover)));
    assert_eq!(hover.target(), Ok(None));
  }
}
