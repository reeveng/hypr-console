//! What the controller asked the keyboard to do.  The on-screen keyboard reads
//! the controller itself while it is up, and that is a claim rather than an
//! arrangement: `console_input_focus` takes the pad and the keyboard
//! InputPlumber publishes beside it for exactly as long as the layer surface is
//! on the screen, with `EVIOCGRAB` underneath, so nothing else receives a press
//! meanwhile. Two readers of one device would both act on the right stick,
//! which navigates and scrolls at once and flickers.  It used to be a profile
//! instead -- `keyboard.yaml`, which translated nothing, loaded when the
//! keyboard came up and taken off when it went. That worked and cost more than
//! it bought: a profile load destroys the pad and builds another, which is the
//! flake where X stopped showing the keyboard until the next reboot, and the
//! `After=` on the unit that was put there to survive it. The claim is the same
//! promise made where it can be kept.  Nothing here opens a device, in the same
//! way and for the same reason as `console_input_controller`: what arrives is
//! handed in and what to do about it is handed back, so every decision can be
//! asked of it twice and answered the same way. The binary is the only part
//! that touches the kernel.  Which button is which is no one's to say here.
//! `console_input_focus` names a press in the profile's own words and
//! `console_input_gamepad::vocabulary` turns that into the word a person uses,
//! which is what this table is written in. The trap that vocabulary exists for
//! is live in this file: the button labelled X on this device is `BTN_NORTH` --
//! `North` -- and the one labelled Y is `BTN_WEST`, and X is the button that
//! puts the keyboard on the screen.  What comes back is a change rather than
//! a command: a press and the button or axis it was made on, or that same
//! button or axis let go. A repeat stops when what started it comes back up and
//! nothing else stops it. Before, anything that was not a press read as nothing
//! being pressed, and the report the kernel ends every batch with is not a
//! press, so a held direction was let go of in the read that pressed it and
//! never repeated; a stick held over pressed again at every value it reported
//! instead. The dpad is a hat here, and is let go of as an axis coming back to
//! the middle rather than as a button.

use console_core_never::Never;
use console_input_event_devices::{AbsoluteAxisCode, EventType, InputEvent as KernelEvent};
use console_input_focus::{DeviceKind, Direction, InputEvent, Spans, translate};
use console_input_gamepad::axis::Range;
use console_input_gamepad::vocabulary::spoken_for;

use crate::key_repeat::{Cadence, MOVING, TYPING};

const DEADZONE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardCommand {
  Up,
  Down,
  Left,
  Right,
  Select,
  Backspace,
  Enter,
  Shift,
  Toggle,
  PreviousLanguage,
  NextLanguage,
}

impl KeyboardCommand {
  pub fn direction(self) -> Result<Option<(i32, i32)>, Never> {
    Ok(match self {
      KeyboardCommand::Up => Some((0, -1)),
      KeyboardCommand::Down => Some((0, 1)),
      KeyboardCommand::Left => Some((-1, 0)),
      KeyboardCommand::Right => Some((1, 0)),
      KeyboardCommand::Select
      | KeyboardCommand::Backspace
      | KeyboardCommand::Enter
      | KeyboardCommand::Shift
      | KeyboardCommand::Toggle
      | KeyboardCommand::PreviousLanguage
      | KeyboardCommand::NextLanguage => None,
    })
  }

  pub fn cadence(self) -> Result<Cadence, Never> {
    let Ok(direction) = self.direction();

    Ok(match direction {
      Some(_) => MOVING,
      None => TYPING,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
  Button(&'static str),
  Axis(AbsoluteAxisCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
  Pressed {
    element: Element,
    command: KeyboardCommand,
  },
  Released {
    element: Element,
  },
}

const BUTTONS: [(&str, KeyboardCommand); 13] = [
  ("a", KeyboardCommand::Select),
  ("b", KeyboardCommand::Backspace),
  ("x", KeyboardCommand::Toggle),
  ("y", KeyboardCommand::Shift),
  ("menu", KeyboardCommand::Enter),
  ("l1", KeyboardCommand::PreviousLanguage),
  ("r1", KeyboardCommand::NextLanguage),
  ("l3", KeyboardCommand::Select),
  ("r3", KeyboardCommand::Select),
  ("dpad-up", KeyboardCommand::Up),
  ("dpad-down", KeyboardCommand::Down),
  ("dpad-left", KeyboardCommand::Left),
  ("dpad-right", KeyboardCommand::Right),
];

pub fn change(
  which: DeviceKind,
  event: KernelEvent,
  spans: &Spans,
) -> Result<Option<Change>, Never> {
  let Ok(named) = translate(which, event.kind, event.code, event.value);
  let axis = match event.kind {
    EventType::ABSOLUTE => Some((AbsoluteAxisCode(event.code), event.value)),
    _ => None,
  };

  change_for(named, axis, spans)
}

fn change_for(
  event: InputEvent,
  axis: Option<(AbsoluteAxisCode, i32)>,
  spans: &Spans,
) -> Result<Option<Change>, Never> {
  match event {
    InputEvent::Pressed { button, direction } => {
      let element = match axis {
        Some((hat, _)) => Element::Axis(hat),
        None => Element::Button(button),
      };

      pressed(element, button, direction)
    }
    InputEvent::None => moved(axis, spans),
    InputEvent::Trigger {
      trigger: _,
      direction: _,
    }
    | InputEvent::Pulled {
      trigger: _,
      value: _,
    }
    | InputEvent::Typed {
      code: _,
      direction: _,
    }
    | InputEvent::Unnamed {
      code: _,
      direction: _,
    } => Ok(None),
  }
}

fn pressed(
  element: Element,
  button: &str,
  direction: Direction,
) -> Result<Option<Change>, Never> {
  Ok(match direction {
    Direction::Up => Some(Change::Released { element }),
    Direction::Down => {
      let Ok(spoken) = spoken_for(button);

      BUTTONS
        .iter()
        .find(|(named, _)| *named == spoken)
        .map(|(_, command)| Change::Pressed {
          element,
          command: *command,
        })
    }
  })
}

fn moved(axis: Option<(AbsoluteAxisCode, i32)>, spans: &Spans) -> Result<Option<Change>, Never> {
  let (axis_3, value) = match axis {
    Some((axis_2, value)) => (axis_2, value),
    None => return Ok(None),
  };

  let element = Element::Axis(axis_3);

  let range = match spans.iter().find(|(named, _)| *named == axis_3) {
    Some((_, range)) => *range,
    None => return Ok(Some(Change::Released { element })),
  };

  let Ok(pushed) = from_stick(axis_3, value, range);

  Ok(Some(match pushed {
    Some(command) => Change::Pressed { element, command },
    None => Change::Released { element },
  }))
}

pub fn from_stick(
  axis: AbsoluteAxisCode,
  value: i32,
  range: (i32, i32),
) -> Result<Option<KeyboardCommand>, Never> {
  let (low, high) = range;
  let Ok(pushed) = console_input_gamepad::axis::part(value, Range { low, high });

  match ::core::primitive::bool::then_some(pushed.abs() < DEADZONE, ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  Ok(match axis {
    AbsoluteAxisCode::ABS_X | AbsoluteAxisCode::ABS_RX => match ::core::primitive::bool::then_some(pushed < 0.0, ()) {
      ::core::option::Option::Some(()) => Some(KeyboardCommand::Left),
      ::core::option::Option::None => Some(KeyboardCommand::Right),
    },
    AbsoluteAxisCode::ABS_Y | AbsoluteAxisCode::ABS_RY => match ::core::primitive::bool::then_some(pushed < 0.0, ()) {
      ::core::option::Option::Some(()) => Some(KeyboardCommand::Up),
      ::core::option::Option::None => Some(KeyboardCommand::Down),
    },
    _ => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_input_gamepad::vocabulary::BUTTONS as EVERY;

  const STICK: (i32, i32) = (-32768, 32767);

  fn command(event: InputEvent) -> Result<Option<KeyboardCommand>, Never> {
    let Ok(change) = change_for(event, None, &Spans::new());

    Ok(match change {
      Some(Change::Pressed {
        element: _,
        command,
      }) => Some(command),
      Some(Change::Released { element: _ }) | None => None,
    })
  }

  fn pressed(button: &'static str) -> Result<Option<KeyboardCommand>, Never> {
    command(InputEvent::Pressed {
      button,
      direction: Direction::Down,
    })
  }

  fn on_the_pad(
    kind: EventType,
    code: u16,
    value: i32,
    spans: &Spans,
  ) -> Result<Option<Change>, Never> {
    change(DeviceKind::Pad, KernelEvent { kind, code, value }, spans)
  }

  fn sticks() -> Result<Spans, Never> {
    Ok(vec![
      (AbsoluteAxisCode::ABS_X, STICK),
      (AbsoluteAxisCode::ABS_HAT0X, (-1, 1)),
      (AbsoluteAxisCode::ABS_HAT0Y, (-1, 1)),
    ])
  }

  #[test]
  fn the_button_labelled_x_is_the_one_that_raises_the_keyboard() {
    assert_eq!(pressed("North"), Ok(Some(KeyboardCommand::Toggle)));
    assert_eq!(pressed("West"), Ok(Some(KeyboardCommand::Shift)));
    assert_eq!(pressed("South"), Ok(Some(KeyboardCommand::Select)));
    assert_eq!(pressed("East"), Ok(Some(KeyboardCommand::Backspace)));
  }

  #[test]
  fn a_button_asks_on_the_way_down_and_is_let_go_of_on_the_way_up() {
    let south = Element::Button("South");

    assert_eq!(
      change_for(
        InputEvent::Pressed {
          button: "South",
          direction: Direction::Down
        },
        None,
        &Spans::new()
      ),
      Ok(Some(Change::Pressed {
        element: south,
        command: KeyboardCommand::Select
      }))
    );
    assert_eq!(
      change_for(
        InputEvent::Pressed {
          button: "South",
          direction: Direction::Up
        },
        None,
        &Spans::new()
      ),
      Ok(Some(Change::Released { element: south })),
      "the release names the button the press did, or a held A would type forever"
    );
  }

  #[test]
  fn a_button_this_keyboard_does_nothing_with_asks_for_nothing() {
    assert_eq!(pressed("Select"), Ok(None));
    assert_eq!(
      command(InputEvent::Unnamed {
        code: 999,
        direction: Direction::Down
      }),
      Ok(None)
    );
    assert_eq!(
      command(InputEvent::Trigger {
        trigger: "LeftTrigger",
        direction: Direction::Down
      }),
      Ok(None)
    );
  }

  #[test]
  fn the_dpad_arrives_named_rather_than_as_a_hat_to_be_read_here() {
    assert_eq!(pressed("DPadUp"), Ok(Some(KeyboardCommand::Up)));
    assert_eq!(pressed("DPadRight"), Ok(Some(KeyboardCommand::Right)));
  }

  #[test]
  fn the_report_that_ends_every_batch_lets_go_of_nothing() {
    let report = KernelEvent::REPORT;
    let Ok(spans) = sticks();

    assert_eq!(
      on_the_pad(report.kind, report.code, report.value, &spans),
      Ok(None),
      "a held direction stopped repeating at the report that came with its own press"
    );
  }

  #[test]
  fn a_dpad_let_go_is_let_go_of_on_the_hat_it_was_pushed_on() {
    let Ok(spans) = sticks();
    let hat = Element::Axis(AbsoluteAxisCode::ABS_HAT0Y);

    assert_eq!(
      on_the_pad(
        EventType::ABSOLUTE,
        AbsoluteAxisCode::ABS_HAT0Y.0,
        1,
        &spans
      ),
      Ok(Some(Change::Pressed {
        element: hat,
        command: KeyboardCommand::Down
      }))
    );
    assert_eq!(
      on_the_pad(
        EventType::ABSOLUTE,
        AbsoluteAxisCode::ABS_HAT0Y.0,
        0,
        &spans
      ),
      Ok(Some(Change::Released { element: hat }))
    );
    assert_eq!(
      on_the_pad(
        EventType::ABSOLUTE,
        AbsoluteAxisCode::ABS_HAT0Y.0,
        0,
        &Spans::new()
      ),
      Ok(Some(Change::Released { element: hat })),
      "a hat the device said nothing about still comes back to the middle"
    );
  }

  #[test]
  fn a_stick_held_over_reports_one_push_however_often_it_reports() {
    let Ok(spans) = sticks();
    let stick = Element::Axis(AbsoluteAxisCode::ABS_X);
    let held = Ok(Some(Change::Pressed {
      element: stick,
      command: KeyboardCommand::Right,
    }));

    assert_eq!(
      on_the_pad(
        EventType::ABSOLUTE,
        AbsoluteAxisCode::ABS_X.0,
        32767,
        &spans
      ),
      held
    );
    assert_eq!(
      on_the_pad(
        EventType::ABSOLUTE,
        AbsoluteAxisCode::ABS_X.0,
        31000,
        &spans
      ),
      held,
      "the same push, a little less far"
    );
    assert_eq!(
      on_the_pad(EventType::ABSOLUTE, AbsoluteAxisCode::ABS_X.0, 400, &spans),
      Ok(Some(Change::Released { element: stick })),
      "back near the middle"
    );
  }

  #[test]
  fn every_button_this_table_names_is_one_the_vocabulary_carries() {
    for (name, _) in BUTTONS {
      assert!(
        EVERY.iter().any(|(spoken, _)| *spoken == name),
        "{name} is not a button `vocabulary` knows"
      );
    }
  }

  #[test]
  fn a_stick_near_the_middle_asks_for_nothing() {
    let spans: Spans = vec![
      (AbsoluteAxisCode::ABS_X, (0, 255)),
      (AbsoluteAxisCode::ABS_RY, (0, 255)),
    ];
    let pushed = |axis, value| {
      let Ok(change) = change_for(InputEvent::None, Some((axis, value)), &spans);

      match change {
        Some(Change::Pressed {
          element: _,
          command,
        }) => Some(command),
        Some(Change::Released { element: _ }) | None => None,
      }
    };

    assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 128), None);
    assert_eq!(pushed(AbsoluteAxisCode::ABS_X, 140), None);
    assert_eq!(
      pushed(AbsoluteAxisCode::ABS_X, 255),
      Some(KeyboardCommand::Right)
    );
    assert_eq!(
      pushed(AbsoluteAxisCode::ABS_X, 0),
      Some(KeyboardCommand::Left)
    );
    assert_eq!(
      pushed(AbsoluteAxisCode::ABS_RY, 0),
      Some(KeyboardCommand::Up)
    );
  }

  #[test]
  fn a_stick_the_device_said_nothing_about_moves_nothing() {
    let nothing = Spans::new();

    assert_eq!(
      change_for(
        InputEvent::None,
        Some((AbsoluteAxisCode::ABS_X, 32767)),
        &nothing
      ),
      Ok(Some(Change::Released {
        element: Element::Axis(AbsoluteAxisCode::ABS_X)
      }))
    );
    assert_eq!(change_for(InputEvent::None, None, &nothing), Ok(None));
  }

  #[test]
  fn a_key_waits_longer_before_it_repeats_than_a_direction_does() {
    assert_eq!(KeyboardCommand::Backspace.cadence(), Ok(TYPING));
    assert_eq!(KeyboardCommand::Select.cadence(), Ok(TYPING));
    assert_eq!(KeyboardCommand::Left.cadence(), Ok(MOVING));
  }
}
