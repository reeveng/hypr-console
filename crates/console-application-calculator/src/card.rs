//! The keypad, drawn.
//!
//! Five rows of keys in the iPhone's order under the number, so a hand that
//! has used a phone's calculator finds the keys where it expects them. Each
//! row is a row of buttons like the music transport: up and down go between
//! rows and keep the column, left and right go along one, A presses, and a
//! finger taps a key where it is. The operator waiting for its second number
//! stays lit, which is how the phone says what the next number is for.
//!
//! The keys fill the card rather than sitting in the middle of it, the
//! operators are tinted and equals is filled, and the number stands at the
//! right above a rule, where the phone puts it. The phone rubs out a digit with
//! a swipe across the number, which a pad cannot make, so backspace takes the
//! place beside equals that the wide nought had.

use console_core_arguments::{Command, Operands};
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_panel::card::{Card, Door};
use console_panel::icons::Icon;
use console_panel::page::{
  Active, Alignment, ButtonPress, ButtonStyle, Headline, Page, Picture, Row, Rows, Showing,
};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::sum::{Key, Operator, Sum};

pub const WHO: &str = "calculator";

const DOOR: &str = "calculator";

const TITLE: &str = "Calculator";

struct Calculator;

impl Machine for Calculator {
  type Input = ();
  type State = Sum;
  type Request = Key;
  type Effect = Never;

  fn initialize(
    _input: &(),
    _previous: Option<Sum>,
    _effects: &mut Queue<Never>,
  ) -> Result<Sum, Never> {
    Ok(Sum::default())
  }

  fn handle(sum: Sum, key: Key, _effects: &mut Queue<Never>) -> Result<Sum, Never> {
    sum.press(key)
  }
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the retained keypad actions and row reader share one accumulator scoped to the open panel"
  )
)]
struct Keypad(Arc<Mutex<Sum>>);

impl Keypad {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "each retained key handler and the row reader carry the same per-panel accumulator"
    )
  )]
  fn new() -> Result<Self, Never> {
    let Ok(initial) = Calculator::initial_transition(&(), None);

    Ok(Self(Arc::new(Mutex::new(initial.state))))
  }

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the lock covers a short arithmetic transition or a snapshot clone, with no external calls or waits"
    )
  )]
  fn held(&self) -> Result<MutexGuard<'_, Sum>, Never> {
    Ok(match self.0.lock() {
      Ok(held) => held,
      Err(poisoned) => poisoned.into_inner(),
    })
  }

  fn press(&self, key: Key) -> Result<(), Never> {
    let Ok(mut held) = self.held();
    let Ok(transition) = Calculator::transition(std::mem::take(&mut *held), key);
    *held = transition.state;

    Ok(())
  }
}

#[derive(Debug, Clone, Copy)]
enum Label {
  Written(&'static str),
  Symbol(Icon),
}

use Label::{Symbol, Written};

const KEYPAD: [&[(Label, Key)]; 5] = [
  &[
    (Written("AC"), Key::Clear),
    (Written("\u{b1}"), Key::Sign),
    (Written("%"), Key::Percent),
    (Written("\u{f7}"), Key::Operator(Operator::Divide)),
  ],
  &[
    (Written("7"), Key::Digit(7)),
    (Written("8"), Key::Digit(8)),
    (Written("9"), Key::Digit(9)),
    (Written("\u{d7}"), Key::Operator(Operator::Multiply)),
  ],
  &[
    (Written("4"), Key::Digit(4)),
    (Written("5"), Key::Digit(5)),
    (Written("6"), Key::Digit(6)),
    (Written("\u{2212}"), Key::Operator(Operator::Subtract)),
  ],
  &[
    (Written("1"), Key::Digit(1)),
    (Written("2"), Key::Digit(2)),
    (Written("3"), Key::Digit(3)),
    (Written("+"), Key::Operator(Operator::Add)),
  ],
  &[
    (Written("0"), Key::Digit(0)),
    (Written("."), Key::Point),
    (Symbol(Icon::Backspace), Key::Backspace),
    (Written("="), Key::Equals),
  ],
];

pub const COMMAND: Command = Command {
  name: env!("CARGO_PKG_NAME"),
  about: "a calculator",
  flags: &[],
  operands: Operands::None,
};

pub fn door(_argv: &[String]) -> Result<Door, Never> {
  Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
  let Ok(held) = Keypad::new();

  Card::new(move || {
    let Ok(initial) = rows_for(&held, Sum::default());
    let reading = held.clone();
    let Ok(rows) = Rows::computed(move || {
      let Ok(rows) = rows(&reading);

      rows
    });
    let Ok(page) = Page::new(TITLE, rows);
    let Ok(fixed) = page.fixed_opening(WHO);
    let Ok(ready) = fixed.meanwhile(move || initial.clone());

    vec![ready]
  })
}

fn press(held: &Keypad, key: Key, showing: &dyn Showing) -> Result<(), Never> {
  let Ok(()) = held.press(key);
  showing.refresh();

  Ok(())
}

fn rows(held: &Keypad) -> Result<Vec<Row>, Never> {
  let sum = {
    let Ok(current) = held.held();

    current.clone()
  };

  rows_for(held, sum)
}

fn rows_for(held: &Keypad, sum: Sum) -> Result<Vec<Row>, Never> {
  let Ok(shown) = sum.shown();
  let Ok(above) = sum.above();
  let Ok(lit) = sum.lit();
  let Ok(number) = Row::headline(
    Picture::None,
    Headline {
      title: above,
      big: shown,
      alignment: Alignment::Trailing,
      ..Headline::default()
    },
  );
  let mut rows = vec![number];

  for keys in KEYPAD {
    let mut presses = Vec::new();

    for (label, key) in keys {
      let pressing = held.clone();
      let key_2 = *key;
      let now = match (key_2, lit) {
        (Key::Operator(operator), Some(waiting)) => Active::from(operator == waiting),
        (Key::Operator(_), None)
        | (
          Key::Digit(_)
          | Key::Point
          | Key::Equals
          | Key::Clear
          | Key::Sign
          | Key::Percent
          | Key::Backspace,
          _,
        ) => Active::No,
      };
      let style = match key_2 {
        Key::Operator(_) => ButtonStyle::Tinted,
        Key::Equals => ButtonStyle::Filled,
        Key::Digit(_) | Key::Point | Key::Clear | Key::Sign | Key::Percent | Key::Backspace => {
          ButtonStyle::Gray
        }
      };
      let does = move |showing: &dyn Showing| {
        let Ok(()) = press(&pressing, key_2, showing);
      };
      let Ok(button) = match label {
        Written(says) => ButtonPress::labelled(says, now, does),
        Symbol(icon) => ButtonPress::new(*icon, now, does),
      };
      let Ok(button_2) = button.styled(style);

      presses.push(button_2);
    }

    let Ok(row) = Row::pressing(presses, 0);

    rows.push(row);
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_panel::card::Finalizer;
  use console_panel::page::Nowhere;

  type Failure = Box<dyn std::error::Error>;

  #[derive(Debug, PartialEq, Eq)]
  enum Pressed {
    Yes,
    NoKeyThere,
  }

  fn keypad() -> Result<(Page, Finalizer), Failure> {
    let Ok(Card { build, done, .. }) = card(&[]);
    let mut pages = build();
    let page = pages
      .pop()
      .ok_or("the calculator opened without a keypad")?;

    Ok((page, done))
  }

  fn pressed_on_the_card(page: &Page, (row, which): (u32, u32)) -> Result<Pressed, Never> {
    let Ok(rows) = page.rows.read();
    let Ok(at) = console_core_number_conversion::index(row);
    let Ok(along) = console_core_number_conversion::index(which);
    let press = rows
      .get(at)
      .and_then(|row_2| row_2.buttons.as_ref())
      .and_then(|across| across.presses.get(along));

    Ok(match press {
      Some(press_2) => {
        let _ = (press_2.does)(&Nowhere);

        Pressed::Yes
      }
      None => Pressed::NoKeyThere,
    })
  }

  fn big(page: &Page) -> Result<Option<String>, Never> {
    let Ok(rows) = page.rows.read();

    Ok(
      rows
        .first()
        .and_then(|row| row.headline.as_ref())
        .map(|headline| headline.big.clone()),
    )
  }

  #[test]
  fn the_keys_on_the_card_do_the_sum_they_say() -> Result<(), Failure> {
    let (page, done) = keypad()?;

    for key in [(3, 2), (3, 3), (2, 0), (5, 3)] {
      assert_eq!(
        pressed_on_the_card(&page, key),
        Ok(Pressed::Yes),
        "no key at {key:?}"
      );
    }

    assert_eq!(
      big(&page),
      Ok(Some(String::from("-1"))),
      "6 \u{2212} 7 = came to something else"
    );
    let Ok(()) = done();

    Ok(())
  }

  #[test]
  fn every_key_on_the_card_says_what_it_does() -> Result<(), Failure> {
    let (page, done) = keypad()?;
    let Ok(rows) = page.rows.read();
    let on_the_card: Vec<_> = rows
      .iter()
      .filter_map(|row| row.buttons.as_ref())
      .map(|across| across.presses.len())
      .collect();
    let on_the_keypad: Vec<_> = KEYPAD.iter().map(|row| row.len()).collect();

    assert_eq!(on_the_card, on_the_keypad);
    let Ok(()) = done();

    Ok(())
  }

  #[test]
  fn the_operator_waiting_is_lit_on_the_card() -> Result<(), Failure> {
    let (page, done) = keypad()?;

    for key in [(4, 0), (2, 3)] {
      assert_eq!(
        pressed_on_the_card(&page, key),
        Ok(Pressed::Yes),
        "no key at {key:?}"
      );
    }

    let Ok(rows) = page.rows.read();
    let across = rows
      .get(2)
      .and_then(|row| row.buttons.as_ref())
      .ok_or("the card lost a row of keys")?;
    let lit: Vec<Active> = across.presses.iter().map(|press| press.now).collect();

    assert_eq!(lit, vec![Active::No, Active::No, Active::No, Active::Yes]);
    let Ok(()) = done();

    Ok(())
  }
}
