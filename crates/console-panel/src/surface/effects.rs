//! Panel callbacks record effects through `Showing`. The dispatcher owns the
//! pending effects and applies them before it returns the callback's outcome.

use std::cell::Cell;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_input_text::TextField;

use super::{CardModel, Opened, Prompt, Secret, Sure, noted, nth, zoomed_to};
use crate::marks;
use crate::page::{Answer, Confirmation, Initial, Row, Showing, Subject};
use crate::tasks::{self, Task};

pub(super) enum Source<'a> {
  Page,
  Focused,
  Row(&'a Row),
}

pub(super) fn dispatch<Output>(
  state: &mut CardModel,
  rows: &[Row],
  source: Source<'_>,
  act: impl FnOnce(&dyn Showing, &CardModel) -> Output,
) -> Result<Output, Never> {
  let Ok(front) = match source {
    Source::Page => Front::new(),
    Source::Focused => {
      let Ok(row) = match state.at {
        Some(at) => nth(rows, at),
        None => Ok(None),
      };
      Front::over(state, row)
    }
    Source::Row(row) => Front::over(state, Some(row)),
  };
  let outcome = act(&front, state);
  let Ok(()) = front.apply(state, rows);

  Ok(outcome)
}

struct Front {
  at: Cell<Option<u32>>,
  tab: Cell<Option<u32>>,
  opened: Cell<Option<Opened>>,
  toggled: Cell<Toggled>,
  zoomed: Cell<Option<f64>>,
  note: Cell<Option<String>>,
  asking: Cell<Option<Prompt>>,
  sure: Cell<Option<Sure>>,
  framed: Option<crate::zoom::Framed>,
  selecting: Cell<Option<Vec<String>>>,
  pressed: Option<console_program_contract::Command>,
  started: Cell<Vec<Task>>,
  interrupted: Cell<Vec<console_program_contract::Command>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Toggled {
  Yes,
  No,
}

impl Front {
  fn new() -> Result<Front, Never> {
    Ok(Front {
      at: Cell::new(None),
      tab: Cell::new(None),
      opened: Cell::new(None),
      toggled: Cell::new(Toggled::No),
      zoomed: Cell::new(None),
      note: Cell::new(None),
      asking: Cell::new(None),
      sure: Cell::new(None),
      framed: None,
      selecting: Cell::new(None),
      pressed: None,
      started: Cell::new(Vec::new()),
      interrupted: Cell::new(Vec::new()),
    })
  }

  fn over(state: &CardModel, row: Option<&Row>) -> Result<Front, Never> {
    let pressed = row
      .and_then(|row| row.while_running.as_ref())
      .map(|named| named.command.clone());
    let Ok(new) = Front::new();
    let front = Front { pressed, ..new };

    let moving = match &state.shown {
      Some(moving) => moving,
      None => return Ok(front),
    };

    let zoom = match &state.zoom {
      Some((on, zoom)) => match ::core::primitive::bool::then_some(*on == moving.of, ()) {
        ::core::option::Option::Some(()) => *zoom,
        ::core::option::Option::None => crate::zoom::Zoom::default(),
      },
      None => crate::zoom::Zoom::default(),
    };

    let framed = crate::zoom::Framed {
      of: moving.of.clone(),
      zoom,
      room: moving.room,
    };

    Ok(Front {
      framed: Some(framed),
      ..front
    })
  }

  fn apply(self, state: &mut CardModel, rows: &[Row]) -> Result<(), Never> {
    // Destructure every field so adding a buffered effect requires updating this code.
    let Self {
      at,
      tab,
      opened,
      toggled,
      zoomed,
      note,
      asking,
      sure,
      framed: _,
      selecting,
      pressed,
      started,
      interrupted,
    } = self;

    match at.into_inner() {
      Some(at) => state.at = Some(at),
      None => {}
    }

    match tab.into_inner() {
      Some(tab) => {
        let Ok(last) = fitted(state.pages.len().saturating_sub(1));

        state.here = tab.min(last);
        state.selected = Vec::new();
        state.opened = Opened::No;
        state.at = None;
        state.scroll = 0;
      }
      None => {}
    }

    match opened.into_inner() {
      Some(opened) => state.opened = opened,
      None => {}
    }

    match zoomed.into_inner() {
      Some(factor) => {
        let Ok(()) = zoomed_to(state, rows, |was| {
          let Ok(times) = was.times(factor);

          times
        });
      }
      None => {}
    }

    match toggled.into_inner() {
      Toggled::Yes => {
        state.opened = match state.opened {
          Opened::Expanded => Opened::No,
          Opened::No => Opened::Expanded,
        };
      }
      Toggled::No => {}
    }

    match note.into_inner() {
      Some(said) => state.note = Some(said),
      None => {}
    }

    match asking.into_inner() {
      Some(asking) => {
        state.prompt_identity = state.prompt_identity.wrapping_add(1);
        state.asking = Some(Prompt {
          identity: state.prompt_identity,
          ..asking
        });
        state.preedit = None;
      }
      None => {}
    }

    match sure.into_inner() {
      Some(sure) => state.sure = Some(sure),
      None => {}
    }

    match selecting.into_inner() {
      Some(keys) => state.selected = keys,
      None => {}
    }

    for task in started.into_inner() {
      let Ok(notes) = tasks::start_from(&state.tasks, task, pressed.as_ref());
      let Ok(()) = noted(state, notes);
    }

    for command in interrupted.into_inner() {
      let Ok(notes) = tasks::interrupt_from(&state.tasks, &command);
      let Ok(()) = noted(state, notes);
    }

    Ok(())
  }
}

impl Showing for Front {
  fn refresh(&self) {}

  fn replace(&self, standing_on: u32) {
    self.at.set(Some(standing_on));
  }

  fn forget_typing(&self) {}

  fn ask(&self, question: &str, then: Answer) {
    self.asking.replace(Some(Prompt {
      question: question.to_string(),
      typed: TextField::default(),
      secret: Secret::Hidden,
      then,
      identity: 0,
    }));
  }

  fn ask_aloud(&self, question: &str, initial: Initial<'_>, then: Answer) {
    let Ok(typed) = TextField::holding(initial.0);

    self.asking.replace(Some(Prompt {
      question: question.to_string(),
      typed,
      secret: Secret::No,
      then,
      identity: 0,
    }));
  }

  fn sure(&self, question: &str, about: Subject<'_>, does: &[&str], then: Confirmation) {
    let answers = std::iter::once(marks::CANCEL.to_string())
      .chain(does.iter().map(|says| (*says).to_string()))
      .collect();

    self.sure.replace(Some(Sure {
      question: question.to_string(),
      about: about.0.to_string(),
      answers,
      at: 0,
      then,
    }));
  }

  fn run(&self, task: Task) {
    let mut started = self.started.take();
    started.push(task);
    self.started.set(started);
  }

  fn leave_running(&self, arguments: Vec<String>) {
    let Ok(()) = crate::running::left_running(&arguments);
  }

  fn interrupt(&self, command: &console_program_contract::Command) {
    let mut interrupted = self.interrupted.take();
    interrupted.push(command.clone());
    self.interrupted.set(interrupted);
  }

  fn note(&self, said: &str) {
    self.note.replace(Some(said.to_string()));
  }

  fn open_out(&self) {
    self.opened.set(Some(Opened::Expanded));
  }

  fn toggle_full_screen(&self) {
    self.toggled.set(Toggled::Yes);
  }

  fn zoom_by(&self, factor: f64) {
    self.zoomed.set(Some(factor));
  }

  fn framed(&self) -> Result<Option<crate::zoom::Framed>, Never> {
    Ok(self.framed.clone())
  }

  fn select(&self, keys: Vec<String>) {
    self.selecting.replace(Some(keys));
  }

  fn turn_to(&self, tab: u32) {
    self.tab.set(Some(tab));
  }
}
