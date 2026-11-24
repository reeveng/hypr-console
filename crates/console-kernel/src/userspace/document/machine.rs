use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_kernel::events::KeyEdge;
use console_kernel::usb_keyboard::BootReport;

use crate::input::Turn;

use super::wire;

const ESCAPE: u16 = 0x29;
const F2: u16 = 0x3b;
const R: u16 = 0x15;
const DOWN: u16 = 0x51;
const UP: u16 = 0x52;

pub(super) struct Document;

#[derive(Clone, Copy)]
pub(super) enum View {
  Keyboard,
  Opening(u64),
  Page { row: u64, restart: bool },
}

pub(super) struct Progress {
  pub rendered: Option<u64>,
  pub last_row: u64,
  pub next_scanline: Option<u16>,
}

pub(super) struct Request {
  pub turn: Turn,
  pub progress: Option<Progress>,
  pub completion: Option<Completion>,
}

pub(super) enum Completion {
  Opened,
  Rendered { last_row: u64 },
  Restarted,
  Failed,
}

pub(super) enum Effect {
  Keyboard {
    code: u16,
    edge: KeyEdge,
    report: BootReport,
  },
  Open,
  Close,
  Restart,
  Render(u64),
  Present(u16),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Keyboard { .. } | Self::Present(_) => &[Service::Display, Service::Console],
      Self::Open => &[Service::Process, Service::Display],
      Self::Close => &[Service::Process, Service::Display, Service::Console],
      Self::Restart | Self::Render(_) => &[Service::Process],
    })
  }
}

impl SingleEffectMachine for Document {
  type Input = ();
  type State = View;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_: &(), _: Option<View>) -> Result<SingleTransition<View, Effect>, Never> {
    Ok(SingleTransition {
      state: View::Keyboard,
      effect: None,
    })
  }

  fn handle(view: View, request: Request) -> Result<SingleTransition<View, Effect>, Never> {
    let Request {
      turn,
      progress,
      completion,
    } = request;
    let Ok(view_2) = match completion {
      Some(report) => completed(view, report),
      None => Ok(view),
    };

    match turn {
      Turn::Key { code, edge, report } => {
        let limit = progress.map_or(u64::from(wire::HTML_BYTES), |page| page.last_row);
        key(view_2, code, edge, report, limit)
      }
      Turn::Idle => {
        let Ok(effect) = idle(view_2, progress);
        Ok(SingleTransition {
          state: view_2,
          effect,
        })
      }
    }
  }
}

fn key(
  mut view: View,
  code: u16,
  edge: KeyEdge,
  report: BootReport,
  last_row: u64,
) -> Result<SingleTransition<View, Effect>, Never> {
  match (&mut view, edge, code) {
    (View::Keyboard, KeyEdge::Press, F2) => view = View::Opening(0),
    (View::Keyboard, _, _) => {
      return Ok(SingleTransition {
        state: view,
        effect: Some(Effect::Keyboard { code, edge, report }),
      });
    }
    (_, KeyEdge::Press, ESCAPE) => view = View::Keyboard,
    (View::Opening(row) | View::Page { row, .. }, KeyEdge::Press, UP) => {
      *row = row.saturating_sub(1);
    }
    (View::Opening(row) | View::Page { row, .. }, KeyEdge::Press, DOWN) => {
      *row = row.saturating_add(1).min(last_row);
    }
    (View::Page { restart, .. }, KeyEdge::Press, R) => *restart = true,
    (_, _, _) => {}
  }

  Ok(SingleTransition {
    state: view,
    effect: None,
  })
}

fn idle(view: View, progress: Option<Progress>) -> Result<Option<Effect>, Never> {
  Ok(match (view, progress) {
    (View::Keyboard, Some(_)) => Some(Effect::Close),
    (View::Opening(_), _) => Some(Effect::Open),
    (View::Page { restart: true, .. }, _) => Some(Effect::Restart),
    (View::Page { row, .. }, Some(page)) => match page.next_scanline {
      Some(first) => Some(Effect::Present(first)),
      None => bool::then_some(page.rendered != Some(row), Effect::Render(row)),
    },
    (View::Keyboard | View::Page { .. }, None) => None,
  })
}

fn completed(view: View, report: Completion) -> Result<View, Never> {
  Ok(match (view, report) {
    (_, Completion::Failed) => View::Keyboard,
    (View::Opening(row), Completion::Opened)
    | (View::Page { row, .. }, Completion::Restarted) => View::Page {
      row,
      restart: false,
    },
    (View::Page { row, restart }, Completion::Rendered { last_row }) => View::Page {
      row: row.min(last_row),
      restart,
    },
    (view_2, Completion::Opened | Completion::Rendered { .. } | Completion::Restarted) => view_2,
  })
}
