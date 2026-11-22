//! A temporary keyboard selection for a layer that normally declines focus.

use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use console_core_never::Never;

use crate::{KeyboardFocus, Keysym};

const ACQUIRE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection<A> {
  Idle,
  Requested { target: A, started: Instant },
  Held(A),
}

impl<A: Copy + Eq> Selection<A> {
  pub fn target(self) -> Result<Option<A>, Never> {
    Ok(match self {
      Self::Idle => None,
      Self::Requested { target, .. } | Self::Held(target) => Some(target),
    })
  }

  pub fn enter(&mut self, actions: &[A], now: Instant) -> Result<(), Never> {
    match (*self, actions.first()) {
      (Self::Idle, Some(&target)) => {
        *self = Self::Requested {
          target,
          started: now,
        };
      }
      (Self::Idle, None) | (Self::Requested { .. } | Self::Held(_), _) => {}
    }

    Ok(())
  }

  pub fn keep(&mut self, actions: &[A]) -> Result<(), Never> {
    let Ok(target) = self.target();

    match target.filter(|wanted| !actions.contains(wanted)) {
      Some(_) => match actions.first() {
        Some(&first) => return self.move_to(first),
        None => *self = Self::Idle,
      },
      None => {}
    }

    Ok(())
  }

  pub fn observe(&mut self, focus: KeyboardFocus, now: Instant) -> Result<(), Never> {
    match (*self, focus) {
      (Self::Requested { target, .. }, KeyboardFocus::Here) => *self = Self::Held(target),
      (Self::Requested { started, .. }, KeyboardFocus::Elsewhere) => {
        let Ok(left) = acquisition_timeout(started, now);

        match left {
          Duration::ZERO => *self = Self::Idle,
          _still_waiting => {}
        }
      }
      (Self::Held(_), KeyboardFocus::Elsewhere) => *self = Self::Idle,
      (Self::Idle, KeyboardFocus::Here | KeyboardFocus::Elsewhere)
      | (Self::Held(_), KeyboardFocus::Here) => {}
    }

    Ok(())
  }

  pub fn timeout(self, now: Instant) -> Result<Option<Duration>, Never> {
    match self {
      Self::Requested { started, .. } => acquisition_timeout(started, now).map(Some),
      Self::Idle | Self::Held(_) => Ok(None),
    }
  }

  pub fn pressed(&mut self, key: Keysym, actions: &[A]) -> Result<Option<A>, Never> {
    let target = match *self {
      Self::Held(held) => held,
      Self::Idle | Self::Requested { .. } => return Ok(None),
    };
    let mut remaining = actions.iter().copied();
    let before = match remaining.try_fold(None, |previous, action| {
      match (action == target).then_some(previous) {
        Some(found) => ControlFlow::Break(found),
        None => ControlFlow::Continue(Some(action)),
      }
    }) {
      ControlFlow::Break(preceding) => preceding,
      ControlFlow::Continue(_) => return Ok(None),
    };

    let next = match key {
      Keysym::Left | Keysym::Up | Keysym::ISO_Left_Tab => {
        before.or_else(|| actions.last().copied())
      }
      Keysym::Right | Keysym::Down | Keysym::Tab => {
        remaining.next().or_else(|| actions.first().copied())
      }
      Keysym::Return | Keysym::KP_Enter | Keysym::space => return Ok(Some(target)),
      Keysym::Escape => {
        *self = Self::Idle;
        None
      }
      _ => None,
    };

    match next {
      Some(selected) => {
        let Ok(()) = self.move_to(selected);
      }
      None => {}
    }

    Ok(None)
  }

  fn move_to(&mut self, target: A) -> Result<(), Never> {
    *self = match *self {
      Self::Idle => Self::Idle,
      Self::Requested { started, .. } => Self::Requested { target, started },
      Self::Held(_) => Self::Held(target),
    };

    Ok(())
  }
}

fn acquisition_timeout(started: Instant, now: Instant) -> Result<Duration, Never> {
  Ok(match now.checked_duration_since(started) {
    Some(elapsed) => ACQUIRE.saturating_sub(elapsed),
    None => ACQUIRE.saturating_add(started.saturating_duration_since(now)),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn selection_requires_compositor_focus_and_leaves_on_escape_or_blur()
  -> Result<(), std::io::ErrorKind> {
    let now = Instant::now();
    let mut selection = Selection::Idle;
    let actions = [10, 20, 30];
    let Ok(()) = selection.enter(&actions, now);
    assert_eq!(selection.timeout(now), Ok(Some(ACQUIRE)));
    assert_eq!(selection.pressed(Keysym::Return, &actions), Ok(None));
    let Ok(()) = selection.observe(KeyboardFocus::Here, now);
    assert_eq!(selection.pressed(Keysym::Left, &actions), Ok(None));
    assert_eq!(selection.pressed(Keysym::Return, &actions), Ok(Some(30)));
    assert_eq!(selection.pressed(Keysym::Right, &actions), Ok(None));
    assert_eq!(selection.target(), Ok(Some(10)));
    assert_eq!(selection.pressed(Keysym::Left, &actions), Ok(None));
    let Ok(()) = selection.keep(&[40, 30, 20]);
    assert_eq!(selection.target(), Ok(Some(30)));
    let Ok(()) = selection.keep(&[40, 10]);
    assert_eq!(selection.target(), Ok(Some(40)));
    assert_eq!(selection.pressed(Keysym::Escape, &[40, 10]), Ok(None));
    assert_eq!(selection, Selection::Idle);

    let Ok(()) = selection.enter(&actions, now);
    let Ok(()) = selection.observe(KeyboardFocus::Here, now);
    let Ok(()) = selection.observe(KeyboardFocus::Elsewhere, now);
    assert_eq!(selection, Selection::Idle);

    let Ok(()) = selection.enter(&actions, now);
    let expiry = now
      .checked_add(ACQUIRE)
      .ok_or(std::io::ErrorKind::Unsupported)?;
    assert_eq!(selection.timeout(expiry), Ok(Some(Duration::ZERO)));
    let Ok(()) = selection.observe(KeyboardFocus::Elsewhere, expiry);
    assert_eq!(selection, Selection::Idle);

    Ok(())
  }

  #[test]
  fn an_empty_layer_does_not_take_or_hold_focus() {
    let now = Instant::now();
    let mut selection: Selection<u32> = Selection::Idle;
    let Ok(()) = selection.enter(&[], now);
    assert_eq!(selection, Selection::Idle);
    let Ok(()) = selection.enter(&[1], now);
    let Ok(()) = selection.observe(KeyboardFocus::Here, now);
    let Ok(()) = selection.keep(&[]);
    assert_eq!(selection, Selection::Idle);
  }
}
