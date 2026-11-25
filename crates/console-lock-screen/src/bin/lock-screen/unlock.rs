use std::os::fd::{AsFd, BorrowedFd};

use console_core_color::palette::Wearing;
use console_core_geometry::{Containment, Point, Rectangle, Size};
use console_core_never::Never;
use console_core_shapes::{Edge, Font, Panel, Round, Shape, Text, Weight};
use console_draw_surface::{KeyPress, Keysym, PointerEvent};
use console_lock_screen::Method;
use console_login_greeter::greeting::PatternFocus;
use console_login_greeter::picture::{FONT, from_the_room, in_the_room};
use console_login_window::fingerprint::{Event, Verifier};
use console_login_window::password::{self, Check, CheckError, Entry, Password};
use console_login_window::{Availability, Verification};

const TABS_Y: i32 = 427;
const TAB_WIDTH: i32 = 130;
const TAB_GAP: i32 = 138;
const KEYS: [&str; 4] = [
  "1 2 3 4 5 6 7 8 9 0",
  "q w e r t y u i o p",
  "a s d f g h j k l",
  "z x c v b n m , . /",
];
const SHIFTED: [&str; 4] = [
  "! @ # $ % ^ & * ( )",
  "Q W E R T Y U I O P",
  "A S D F G H J K L",
  "Z X C V B N M < > ?",
];
const SYMBOLS: [&str; 4] = [
  "1 2 3 4 5 6 7 8 9 0",
  r#"- _ = + [ ] { } \ |"#,
  r#"; : ' " ` ~ @ # $ %"#,
  "< > ? ! ^ & * ( )",
];

#[derive(Clone, Copy)]
enum Action {
  Select(Method),
  Scan,
  Type(char),
  Shift,
  Symbols,
  Erase,
  EraseWord,
  Submit,
}

struct Button {
  bounds: Rectangle<i32>,
  label: &'static str,
  action: Action,
}

#[derive(Default)]
enum Keyboard {
  #[default]
  Letters,
  Shifted,
  Symbols,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDisposition {
  Handled,
  ForwardToGreeter,
}

pub enum Confirmation {
  Accepted,
  AwaitingInput,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttemptDisposition {
  Current,
  Abandoned,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Interaction {
  Waiting,
  Received,
}

enum FingerprintFeedback {
  Scan,
  Retry,
}

enum PanelStyle {
  Highlighted,
  Ordinary,
}

struct PasswordAttempt {
  check: Check,
  disposition: AttemptDisposition,
}

pub struct Unlock {
  method: Method,
  preferred: Method,
  interaction: Interaction,
  fingerprint: Option<Verifier>,
  password_ready: Availability,
  password: Password,
  check: Option<PasswordAttempt>,
  keyboard: Keyboard,
  fingerprint_feedback: FingerprintFeedback,
  verification_feedback: Option<&'static str>,
}

impl Unlock {
  #[must_use]
  pub fn new(preferred: Method) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(password_ready) = password::enable_for_lock();
    let method = match (preferred, password_ready) {
      (Method::Password, Availability::Available) => Method::Password,
      _ => Method::Pattern,
    };

    Ok(Self {
      method,
      preferred,
      interaction: Interaction::Waiting,
      fingerprint: None,
      password_ready,
      password: Password::default(),
      check: None,
      keyboard: Keyboard::default(),
      fingerprint_feedback: FingerprintFeedback::Scan,
      verification_feedback: None,
    })
  }

  pub fn acquired(&mut self) -> Result<(), Never> {
    let Ok(verifier) = Verifier::for_session();
    self.fingerprint = Some(verifier);
    Ok(())
  }

  #[must_use]
  pub fn method(&self) -> core::result::Result<Method, console_core_never::Never> {
    Ok(self.method)
  }

  #[must_use]
  pub fn descriptors(
    &self,
  ) -> core::result::Result<Vec<BorrowedFd<'_>>, console_core_never::Never> {
    let scanner = match &self.fingerprint {
      Some(verifier) => {
        let Ok(descriptor) = verifier.fd();
        descriptor
      }
      None => None,
    };
    Ok(
      scanner
        .into_iter()
        .chain(self.check.as_ref().map(|attempt| attempt.check.as_fd()))
        .collect(),
    )
  }

  pub fn verified(&mut self) -> Result<Confirmation, Never> {
    let fingerprint_result = match &mut self.fingerprint {
      Some(verifier) => {
        let Ok(event) = verifier.response();
        event
      }
      None => None,
    };

    match fingerprint_result {
      Some(Event::Accepted) => return Ok(Confirmation::Accepted),
      Some(Event::Available) => match (self.preferred, self.interaction) {
        (Method::Fingerprint, Interaction::Waiting) => {
          let Ok(()) = self.select(Method::Fingerprint);
        }
        _ => {}
      },
      Some(Event::Refused) => self.fingerprint_feedback = FingerprintFeedback::Retry,
      None => {}
    }

    let check = match self.check.as_mut() {
      Some(check) => check,
      None => return Ok(Confirmation::AwaitingInput),
    };
    let Ok(outcome) = check.check.result();

    Ok(match (outcome, check.disposition) {
      (Verification::Pending, _) => Confirmation::AwaitingInput,
      (Verification::Accepted | Verification::Refused, AttemptDisposition::Abandoned) => {
        self.check = None;
        Confirmation::AwaitingInput
      }
      (Verification::Accepted, AttemptDisposition::Current) => {
        self.check = None;
        self.verification_feedback = None;
        Confirmation::Accepted
      }
      (Verification::Refused, AttemptDisposition::Current) => {
        self.check = None;
        self.verification_feedback = Some("Password was not accepted");
        Confirmation::AwaitingInput
      }
    })
  }

  #[must_use]
  fn methods(&self) -> Result<impl Iterator<Item = Method> + '_, Never> {
    Ok(
      Method::ALL
        .into_iter()
        .filter(|method| self.availability(*method) == Ok(Availability::Available)),
    )
  }

  #[must_use]
  fn availability(&self, method: Method) -> Result<Availability, Never> {
    match method {
      Method::Pattern => Ok(Availability::Available),
      Method::Fingerprint => match &self.fingerprint {
        Some(verifier) => verifier.availability(),
        None => Ok(Availability::Unavailable),
      },
      Method::Password => Ok(self.password_ready),
    }
  }

  fn cycle(&mut self) -> Result<(), Never> {
    let Ok(methods) = self.methods();
    let next = match methods.skip_while(|method| *method != self.method).nth(1) {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => Method::Pattern,
    };
    self.select(next)
  }

  fn select(&mut self, method: Method) -> Result<(), Never> {
    match { ::core::primitive::bool::then_some(method != self.method, ()) } {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.password.clear();

        match &mut self.check {
          Some(attempt) => {
            attempt.disposition = AttemptDisposition::Abandoned;
          }
          ::core::option::Option::None => {}
        }

        match &mut self.fingerprint {
          Some(verifier) => {
            let Ok(()) = verifier.cancel_scan();
          }
          ::core::option::Option::None => {}
        }

        self.fingerprint_feedback = FingerprintFeedback::Scan;
        self.verification_feedback = None;
        self.method = method;
        self.keyboard = Keyboard::default();
      }
      ::core::option::Option::None => {}
    }

    match method {
      Method::Fingerprint => self.scan(),
      Method::Pattern | Method::Password => Ok(()),
    }
  }

  fn scan(&mut self) -> Result<(), Never> {
    match &mut self.fingerprint {
      Some(verifier) => {
        let Ok(event) = verifier.scan();
        self.fingerprint_feedback = match event {
          Some(Event::Refused) => FingerprintFeedback::Retry,
          Some(Event::Accepted | Event::Available) | None => FingerprintFeedback::Scan,
        };
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub fn key(&mut self, press: KeyPress, focus: PatternFocus) -> Result<InputDisposition, Never> {
    self.interaction = Interaction::Received;

    let Ok(()) = match (self.method, press.key, focus) {
      (_, Keysym::Tab | Keysym::ISO_Left_Tab, _)
      | (Method::Fingerprint | Method::Password, Keysym::Down, _)
      | (Method::Pattern, Keysym::Down, PatternFocus::Submit) => self.cycle(),
      (Method::Pattern, _, _) => return Ok(InputDisposition::ForwardToGreeter),
      (_, Keysym::Escape, _) => self.select(Method::Pattern),
      (Method::Fingerprint, Keysym::Return | Keysym::KP_Enter | Keysym::space, _) => self.scan(),
      (Method::Fingerprint, _, _) => Ok(()),
      (Method::Password, Keysym::Return | Keysym::KP_Enter, _) => self.act(Action::Submit),
      (Method::Password, Keysym::BackSpace, _) => {
        let action = match (press.modifiers & KeyPress::CONTROL != 0).then_some(()) {
          Some(()) => Action::EraseWord,
          None => Action::Erase,
        };

        self.act(action)
      }
      (Method::Password, key_2, _) => {
        let Ok(character) = key_2.char();

        match character {
          Some(character_2) => self.act(Action::Type(character_2)),
          None => Ok(()),
        }
      }
    };

    Ok(InputDisposition::Handled)
  }

  pub fn pointer(
    &mut self,
    event: PointerEvent,
    canvas: Option<Size<u32>>,
  ) -> Result<InputDisposition, Never> {
    match (event, canvas) {
      (PointerEvent::Down { at }, Some(size)) => {
        self.interaction = Interaction::Received;
        let Ok(point) = in_the_room(size, Point { x: at.0, y: at.1 });
        let Ok(buttons) = self.buttons();
        let pressed = buttons
          .into_iter()
          .find(|button| button.bounds.contains(point) == Ok(Containment::Inside));

        match pressed {
          Some(button) => {
            let Ok(()) = self.act(button.action);
            return Ok(InputDisposition::Handled);
          }
          None => {}
        }
      }
      (PointerEvent::Down { .. }, None)
      | (
        PointerEvent::Moved { .. }
        | PointerEvent::Scrolled { .. }
        | PointerEvent::Pinched { .. }
        | PointerEvent::Up
        | PointerEvent::Left,
        _,
      ) => {}
    }

    Ok(match self.method {
      Method::Pattern => InputDisposition::ForwardToGreeter,
      Method::Fingerprint | Method::Password => InputDisposition::Handled,
    })
  }

  fn act(&mut self, action: Action) -> Result<(), Never> {
    match (action, self.check.as_ref()) {
      (Action::Select(method), _) => return self.select(method),
      (Action::Scan, _) => return self.scan(),
      (
        Action::Type(_)
        | Action::Shift
        | Action::Symbols
        | Action::Erase
        | Action::EraseWord
        | Action::Submit,
        Some(_),
      ) => {}
      (Action::Type(letter), None) => {
        let Ok(()) = self.password.push(letter);
        self.verification_feedback = None;
      }
      (Action::Shift, None) => {
        self.keyboard = match self.keyboard {
          Keyboard::Shifted => Keyboard::Letters,
          Keyboard::Letters | Keyboard::Symbols => Keyboard::Shifted,
        };
      }
      (Action::Symbols, None) => {
        self.keyboard = match self.keyboard {
          Keyboard::Symbols => Keyboard::Letters,
          Keyboard::Letters | Keyboard::Shifted => Keyboard::Symbols,
        };
      }
      (Action::Erase, None) => {
        let Ok(()) = self.password.erase();
      }
      (Action::EraseWord, None) => {
        let Ok(()) = self.password.erase_previous_word();
      }
      (Action::Submit, None) => {
        let Ok(entry) = self.password.entry();

        match entry {
          Entry::Empty => return Ok(()),
          Entry::Entered => {}
        }

        match Check::start(std::mem::take(&mut self.password)) {
          Ok(check) => {
            self.check = Some(PasswordAttempt {
              check,
              disposition: AttemptDisposition::Current,
            })
          }
          Err(fault) => {
            self.verification_feedback = Some(match fault {
              CheckError::InputTooLong => "Password exceeds the input limit",
              CheckError::SystemLimit => "System password limit is 511 bytes",
              CheckError::Unavailable => "Password verification is unavailable",
            });
          }
        }
      }
    }

    Ok(())
  }

  #[must_use]
  fn buttons(&self) -> core::result::Result<Vec<Button>, console_core_never::Never> {
    let mut buttons = Vec::new();
    let Ok(methods) = self.methods();
    let mut available = methods.peekable();
    let first = available.next();
    let first_tab = match available.peek() {
      Some(_) => first,
      None => None,
    };

    for method in first_tab.into_iter().chain(available) {
      let x = match method {
        Method::Pattern => 13,
        Method::Fingerprint => 13_i32.saturating_add(TAB_GAP),
        Method::Password => 13_i32.saturating_add(TAB_GAP.saturating_mul(2)),
      };

      let Ok(label) = method.label();
      let Ok(tab) = button(
        Point { x, y: TABS_Y },
        TAB_WIDTH,
        label,
        Action::Select(method),
      );
      buttons.push(tab);
    }

    match self.method {
      Method::Pattern => {}
      Method::Fingerprint => {
        let label = match self.fingerprint_feedback {
          FingerprintFeedback::Retry => "Retry fingerprint",
          FingerprintFeedback::Scan => "Scan fingerprint",
        };
        buttons.push({
          let Ok(explicit_value) = button(Point { x: 110, y: 260 }, 220, label, Action::Scan);
          explicit_value
        });
      }
      Method::Password => {
        let Ok(()) = self.keyboard_buttons(&mut buttons);
      }
    }

    Ok(buttons)
  }

  fn keyboard_buttons(&self, buttons: &mut Vec<Button>) -> Result<(), Never> {
    let rows = match self.keyboard {
      Keyboard::Letters => KEYS,
      Keyboard::Shifted => SHIFTED,
      Keyboard::Symbols => SYMBOLS,
    };

    for (row, y) in rows.into_iter().zip([147, 194, 241, 288]) {
      for (label, x) in row.split_whitespace().zip((2..440).step_by(44)) {
        match label.chars().next() {
          Some(character) => {
            let Ok(key) = button(Point { x, y }, 41, label, Action::Type(character));
            buttons.push(key);
          }
          None => {}
        }
      }
    }

    buttons.extend([
      ({
        let Ok(explicit_value) = button(Point { x: 2, y: 335 }, 70, "Shift", Action::Shift);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = button(Point { x: 76, y: 335 }, 80, "Symbols", Action::Symbols);
        explicit_value
      }),
      ({
        let Ok(explicit_value) =
          button(Point { x: 160, y: 335 }, 157, "Space", Action::Type(' '));
        explicit_value
      }),
      ({
        let Ok(explicit_value) =
          button(Point { x: 321, y: 335 }, 117, "Backspace", Action::Erase);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = button(Point { x: 154, y: 381 }, 132, "Unlock", Action::Submit);
        explicit_value
      }),
    ]);
    Ok(())
  }

  pub fn paint(
    &self,
    canvas: Size<u32>,
    wearing: &Wearing,
    shapes: &mut Vec<Shape>,
  ) -> Result<(), Never> {
    let Ok(buttons) = self.buttons();

    for button in buttons {
      let style = match matches!(button.action, Action::Select(method) if method == self.method)
        .then_some(())
      {
        Some(()) => PanelStyle::Highlighted,
        None => PanelStyle::Ordinary,
      };
      let Ok(()) = panel(canvas, wearing, button.bounds, style, button.label, shapes);
    }

    match self.method {
      Method::Pattern => {}
      Method::Fingerprint => {
        let feedback = match self.fingerprint_feedback {
          FingerprintFeedback::Retry => "Fingerprint was not verified",
          FingerprintFeedback::Scan => "Touch or swipe the fingerprint reader",
        };
        let Ok(bounds) = rectangle(Point { x: 0, y: 198 }, 440);
        let Ok(()) = panel(
          canvas,
          wearing,
          bounds,
          PanelStyle::Ordinary,
          feedback,
          shapes,
        );
      }
      Method::Password => {
        let label = match &self.check {
          Some(PasswordAttempt {
            disposition: AttemptDisposition::Abandoned,
            ..
          }) => "Waiting for previous password check…",
          Some(PasswordAttempt {
            disposition: AttemptDisposition::Current,
            ..
          }) => "Checking password…",
          None => match self.verification_feedback {
            Some(message) => message,
            None => "Account password",
          },
        };
        let Ok(label_bounds) = rectangle(Point { x: 0, y: 45 }, 440);
        let Ok(()) = panel(
          canvas,
          wearing,
          label_bounds,
          PanelStyle::Ordinary,
          label,
          shapes,
        );
        let Ok(input_bounds) = rectangle(Point { x: 2, y: 97 }, 436);
        let Ok(mask) = self.password.mask();
        let Ok(()) = panel(
          canvas,
          wearing,
          input_bounds,
          PanelStyle::Ordinary,
          &mask,
          shapes,
        );
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn method_tabs_appear_only_when_there_is_a_choice() {
    let Ok(mut screen) = Unlock::new(Method::Pattern);
    screen.password_ready = Availability::Unavailable;
    let Ok(pattern_buttons) = screen.buttons();
    assert!(pattern_buttons.is_empty());

    screen.password_ready = Availability::Available;
    let Ok(method_tabs) = screen.buttons();
    assert!(matches!(
      method_tabs.as_slice(),
      [
        Button {
          action: Action::Select(Method::Pattern),
          ..
        },
        Button {
          action: Action::Select(Method::Password),
          ..
        },
      ]
    ));
  }

  #[test]
  fn an_unavailable_fingerprint_preference_starts_on_pattern() {
    let Ok(screen) = Unlock::new(Method::Fingerprint);
    assert_eq!(
      ({
        let Ok(explicit_value) = screen.method();
        explicit_value
      }),
      Method::Pattern
    );
    let Ok(mut methods) = screen.methods();
    assert!(!methods.any(|method| method == Method::Fingerprint));
  }

  #[test]
  fn switching_methods_clears_the_pending_password() {
    let Ok(mut screen) = Unlock::new(Method::Pattern);
    let Ok(tab) = KeyPress::unmodified(Keysym::Tab);
    let Ok(letter) = KeyPress::unmodified(Keysym::a);
    screen.password_ready = Availability::Available;
    assert_eq!(
      screen.key(tab, PatternFocus::Dot),
      Ok(InputDisposition::Handled)
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = screen.method();
        explicit_value
      }),
      Method::Password
    );
    assert_eq!(
      screen.key(letter, PatternFocus::Dot),
      Ok(InputDisposition::Handled)
    );
    let Ok(mask) = screen.password.mask();
    assert_eq!(mask, "•");
    assert_eq!(
      screen.key(tab, PatternFocus::Dot),
      Ok(InputDisposition::Handled)
    );
    assert_eq!(
      ({
        let Ok(explicit_value) = screen.method();
        explicit_value
      }),
      Method::Pattern
    );
    assert_eq!(
      screen.key(tab, PatternFocus::Dot),
      Ok(InputDisposition::Handled)
    );
    assert_eq!(screen.password.entry(), Ok(Entry::Empty));
  }
}

#[must_use]
fn rectangle(
  origin: Point<i32>,
  width: i32,
) -> core::result::Result<Rectangle<i32>, console_core_never::Never> {
  Ok(Rectangle {
    origin,
    size: Size { width, height: 40 },
  })
}

#[must_use]
fn button(
  at: Point<i32>,
  width: i32,
  label: &'static str,
  action: Action,
) -> core::result::Result<Button, console_core_never::Never> {
  let Ok(bounds) = rectangle(at, width);
  Ok(Button {
    bounds,
    label,
    action,
  })
}

fn panel(
  canvas: Size<u32>,
  wearing: &Wearing,
  bounds: Rectangle<i32>,
  style: PanelStyle,
  label: &str,
  shapes: &mut Vec<Shape>,
) -> Result<(), Never> {
  let Ok(at) = from_the_room(canvas, bounds.origin);
  let Ok(corner) = bounds.end();
  let Ok(end) = from_the_room(canvas, corner);
  let size = Size {
    width: end.x.abs_diff(at.x),
    height: end.y.abs_diff(at.y),
  };
  let fill = match style {
    PanelStyle::Highlighted => wearing.fill,
    PanelStyle::Ordinary => wearing.panel,
  };
  let font = Font {
    family: FONT.to_owned(),
    height: ::core::primitive::u32::wrapping_div(size.height.saturating_mul(55), 100),
  };
  let Ok(measured) = console_draw_painting::measure_text(
    console_draw_painting::Run {
      said: label,
      weight: Weight::Plain,
      width: size.width,
    },
    &font,
  );

  let left = {
    let outcome_left = i32::try_from(::core::primitive::u32::wrapping_div(
      size.width.saturating_sub(measured.width),
      2,
    ));

    match outcome_left {
      Ok(matched_left) => matched_left,
      Err(_horizontal_offset_exceeds_coordinate_range) => return Ok(()),
    }
  };

  let top = {
    let outcome_top = i32::try_from(::core::primitive::u32::wrapping_div(
      size.height.saturating_sub(measured.height),
      2,
    ));

    match outcome_top {
      Ok(matched_top) => matched_top,
      Err(_vertical_offset_exceeds_coordinate_range) => return Ok(()),
    }
  };

  shapes.extend([
    Shape::Panel(Panel {
      at,
      size,
      round: Round(::core::primitive::u32::wrapping_div(size.height, 5)),
      fill,
      edge: Edge::None,
    }),
    Shape::Text(Text {
      at: Point {
        x: at.x.saturating_add(left),
        y: at.y.saturating_add(top),
      },
      width: size.width,
      said: label.to_owned(),
      weight: Weight::Plain,
      font,
      ink: wearing.text,
    }),
  ]);
  Ok(())
}
