//! Present the page and toolbar as layers when possible. Commit a complete
//! parent frame before removing a page layer, including when a patch must wait.

use std::{error::Error, io};

use console_core_geometry::Size;
use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_panel::ui::painting::Frame;
use console_panel::ui::surface::Visible;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Redraw {
  Requested,
  Current,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
  pub(crate) window: Size<u32>,
  pub(crate) toolbar: Frame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backdrop {
  Combined,
  Split(Geometry),
}

#[derive(Clone, Copy)]
pub(crate) enum Layers {
  Available,
  Combined,
}

pub(crate) struct Repaint {
  pub(crate) before: Backdrop,
  pub(crate) geometry: Geometry,
  pub(crate) layers: Layers,
  pub(crate) page: Redraw,
  pub(crate) toolbar: Redraw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
  DrawPageLayer,
  PatchToolbar,
  DrawCombined,
  ClearPageLayer,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Compositor])
  }
}

pub(crate) enum Parent {
  Current,
  Patch,
  Full,
}

pub(crate) enum State {
  DrawingLayer { geometry: Geometry, parent: Parent },
  Patching(Geometry),
  Composing(Backdrop),
  ClearingLayer,
  Presented(Backdrop),
  Failed(Box<dyn Error>),
}

type Turn = SingleTransition<State, Effect>;

pub(crate) struct Presentation;

impl Presentation {
  pub(crate) fn paint<Context>(
    repaint: &Repaint,
    context: &mut Context,
    perform: impl Fn(&mut Context, Effect) -> Result<Visible, Box<dyn Error>>,
  ) -> Result<Backdrop, Box<dyn Error>> {
    let Ok(first) = Self::initialize(repaint, None);

    match Self::run_effects(first, 3, context, perform) {
      Ok(State::Presented(backdrop)) => Ok(backdrop),
      Ok(State::Failed(fault)) => Err(fault),
      Ok(
        State::DrawingLayer { .. }
        | State::Patching(_)
        | State::Composing(_)
        | State::ClearingLayer,
      )
      | Err(_) => Err(Box::new(io::Error::other("browser presentation did not settle"))),
    }
  }
}

impl SingleEffectMachine for Presentation {
  type Input = Repaint;
  type State = State;
  type Request = Result<Visible, Box<dyn Error>>;
  type Effect = Effect;

  fn initialize(repaint: &Repaint, _previous: Option<State>) -> Result<Turn, Never> {
    let Repaint {
      before,
      geometry,
      layers,
      page,
      toolbar,
    } = *repaint;
    let parent_frame = match ((before == Backdrop::Split(geometry)).then_some(()), toolbar) {
      (Some(()), Redraw::Current) => Parent::Current,
      (Some(()), Redraw::Requested) => Parent::Patch,
      (None, _) => Parent::Full,
    };

    match (layers, page, parent_frame) {
      (Layers::Combined, _, _) => composed(Backdrop::Combined),
      (Layers::Available, Redraw::Requested, parent)
      | (Layers::Available, Redraw::Current, parent @ Parent::Full) => Ok(Turn {
        state: State::DrawingLayer { geometry, parent },
        effect: Some(Effect::DrawPageLayer),
      }),
      (Layers::Available, Redraw::Current, parent @ (Parent::Current | Parent::Patch)) => {
        layered(geometry, parent)
      }
    }
  }

  fn handle(state: State, reply: Self::Request) -> Result<Turn, Never> {
    match (state, reply) {
      (State::DrawingLayer { geometry, parent }, Ok(Visible::Yes)) => layered(geometry, parent),
      (State::DrawingLayer { .. }, Ok(Visible::NotYet)) => composed(Backdrop::Combined),
      (State::Patching(geometry), Ok(Visible::Yes)) => presented(Backdrop::Split(geometry)),
      (State::Patching(geometry), Ok(Visible::NotYet)) => composed(Backdrop::Split(geometry)),
      (State::Composing(Backdrop::Combined), Ok(Visible::Yes)) => Ok(Turn {
        state: State::ClearingLayer,
        effect: Some(Effect::ClearPageLayer),
      }),
      (State::Composing(layout @ Backdrop::Split(_)), Ok(Visible::Yes)) => presented(layout),
      (State::ClearingLayer, Ok(Visible::Yes)) => presented(Backdrop::Combined),
      (settled @ (State::Presented(_) | State::Failed(_)), _) => Ok(Turn {
        state: settled,
        effect: None,
      }),
      (_, Err(fault)) => Ok(Turn {
        state: State::Failed(fault),
        effect: None,
      }),
      (State::Composing(_) | State::ClearingLayer, Ok(Visible::NotYet)) => Ok(Turn {
        state: State::Failed(Box::new(std::io::Error::other(
          "browser parent frame could not be presented",
        ))),
        effect: None,
      }),
    }
  }
}

fn layered(geometry: Geometry, parent: Parent) -> Result<Turn, Never> {
  match parent {
    Parent::Current => presented(Backdrop::Split(geometry)),
    Parent::Patch => Ok(Turn {
      state: State::Patching(geometry),
      effect: Some(Effect::PatchToolbar),
    }),
    Parent::Full => composed(Backdrop::Split(geometry)),
  }
}

fn composed(backdrop: Backdrop) -> Result<Turn, Never> {
  Ok(Turn {
    state: State::Composing(backdrop),
    effect: Some(Effect::DrawCombined),
  })
}

fn presented(backdrop: Backdrop) -> Result<Turn, Never> {
  Ok(Turn {
    state: State::Presented(backdrop),
    effect: None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const TOOLBAR: Size<u32> = Size {
    width: 1000,
    height: 100,
  };
  const GEOMETRY: Geometry = Geometry {
    window: Size {
      width: 1000,
      height: 750,
    },
    toolbar: Frame {
      points: TOOLBAR,
      device: TOOLBAR,
    },
  };

  #[test]
  fn hovering_before_the_first_page_frame_submits_one_complete_frame() {
    let repaint = Repaint {
      before: Backdrop::Combined,
      geometry: GEOMETRY,
      layers: Layers::Available,
      page: Redraw::Current,
      toolbar: Redraw::Requested,
    };
    let Ok(first) = Presentation::initialize(&repaint, None);
    let mut effects = Vec::new();
    let outcome = Presentation::run_effects(first, 3, &mut effects, |recorded, effect| {
      recorded.push(effect);
      Ok(match effect {
        Effect::DrawPageLayer => Visible::NotYet,
        Effect::DrawCombined | Effect::ClearPageLayer | Effect::PatchToolbar => Visible::Yes,
      })
    });

    assert_eq!(
      effects,
      [
        Effect::DrawPageLayer,
        Effect::DrawCombined,
        Effect::ClearPageLayer
      ]
    );
    assert!(matches!(outcome, Ok(State::Presented(Backdrop::Combined))));
  }

  #[test]
  fn a_busy_toolbar_buffer_falls_back_without_removing_the_page_layer() {
    let repaint = Repaint {
      before: Backdrop::Split(GEOMETRY),
      geometry: GEOMETRY,
      layers: Layers::Available,
      page: Redraw::Current,
      toolbar: Redraw::Requested,
    };
    let Ok(first) = Presentation::initialize(&repaint, None);
    let mut effects = Vec::new();
    let outcome = Presentation::run_effects(first, 2, &mut effects, |recorded, effect| {
      recorded.push(effect);
      Ok(match effect {
        Effect::PatchToolbar => Visible::NotYet,
        Effect::DrawPageLayer | Effect::DrawCombined | Effect::ClearPageLayer => Visible::Yes,
      })
    });

    assert_eq!(effects, [Effect::PatchToolbar, Effect::DrawCombined]);
    assert!(matches!(
      outcome,
      Ok(State::Presented(Backdrop::Split(GEOMETRY)))
    ));
  }

  #[test]
  fn a_failed_overlay_commit_leaves_the_page_layer_in_place() {
    let repaint = Repaint {
      before: Backdrop::Split(GEOMETRY),
      geometry: GEOMETRY,
      layers: Layers::Combined,
      page: Redraw::Requested,
      toolbar: Redraw::Current,
    };
    let Ok(first) = Presentation::initialize(&repaint, None);
    let outcome = Presentation::run_effects(first, 1, &mut (), |(), effect| {
      assert_eq!(effect, Effect::DrawCombined);
      Err(Box::new(io::Error::other("commit failed")))
    });

    assert!(matches!(outcome, Ok(State::Failed(_))));
  }
}
