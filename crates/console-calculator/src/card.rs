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

use std::sync::{Arc, Mutex};

use console_core_never::Never;
use console_panel::card::{Card, Door};
use console_panel::icons::Icon;
use console_panel::page::{Active, Alignment, ButtonPress, ButtonStyle, Headline, Page, Picture, Row, Rows, Showing};

use crate::sum::{Key, Operator, Sum};

pub const WHO: &str = "calculator";

const DOOR: &str = "calculator";

const TITLE: &str = "Calculator";

type Shared = Arc<Mutex<Sum>>;

#[derive(Debug, Clone, Copy)]
enum Label {
    Written(&'static str),
    Symbol(Icon),
}

use Label::{Symbol, Written};

const KEYPAD: [&[(Label, Key)]; 5] = [
    &[(Written("AC"), Key::Clear), (Written("\u{b1}"), Key::Sign), (Written("%"), Key::Percent), (Written("\u{f7}"), Key::Operator(Operator::Divide))],
    &[(Written("7"), Key::Digit(7)), (Written("8"), Key::Digit(8)), (Written("9"), Key::Digit(9)), (Written("\u{d7}"), Key::Operator(Operator::Multiply))],
    &[(Written("4"), Key::Digit(4)), (Written("5"), Key::Digit(5)), (Written("6"), Key::Digit(6)), (Written("\u{2212}"), Key::Operator(Operator::Subtract))],
    &[(Written("1"), Key::Digit(1)), (Written("2"), Key::Digit(2)), (Written("3"), Key::Digit(3)), (Written("+"), Key::Operator(Operator::Add))],
    &[(Written("0"), Key::Digit(0)), (Written("."), Key::Point), (Symbol(Icon::Backspace), Key::Backspace), (Written("="), Key::Equals)],
];

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    let held: Shared = Arc::new(Mutex::new(Sum::default()));

    Card::new(Arc::new(move || {
        let reading = Arc::clone(&held);
        let Ok(rows) = Rows::asked(move || {
            let Ok(rows) = rows(&reading);

            rows
        });
        let Ok(page) = Page::new(TITLE, rows);

        vec![page]
    }))
}

fn now(held: &Shared) -> Result<Sum, Never> {
    Ok(match held.lock() {
        Ok(sum) => sum.clone(),
        Err(_the_lock_is_poisoned) => Sum::default(),
    })
}

fn press(held: &Shared, key: Key, showing: &dyn Showing) -> Result<(), Never> {
    match held.lock() {
        Ok(mut sum) => {
            let Ok(next) = sum.pressed(key);

            *sum = next;
        }
        Err(_the_lock_is_poisoned) => {},
    }

    showing.refresh();

    Ok(())
}

pub fn rows(held: &Shared) -> Result<Vec<Row>, Never> {
    let Ok(sum) = now(held);
    let Ok(shown) = sum.shown();
    let Ok(above) = sum.above();
    let Ok(lit) = sum.lit();
    let Ok(number) = Row::headline(Picture::None, Headline { title: above, big: shown, alignment: Alignment::Trailing, ..Headline::default() });
    let mut rows = vec![number];

    for keys in KEYPAD {
        let mut presses = Vec::new();

        for (label, key) in keys {
            let pressing = Arc::clone(held);
            let key = *key;
            let now = match (key, lit) {
                (Key::Operator(operator), Some(waiting)) => match operator == waiting {
                    true => Active::Yes,
                    false => Active::No,
                },
                (Key::Operator(_), None)
                | (Key::Digit(_) | Key::Point | Key::Equals | Key::Clear | Key::Sign | Key::Percent | Key::Backspace, _) => Active::No,
            };
            let style = match key {
                Key::Operator(_) => ButtonStyle::Tinted,
                Key::Equals => ButtonStyle::Filled,
                Key::Digit(_) | Key::Point | Key::Clear | Key::Sign | Key::Percent | Key::Backspace => ButtonStyle::Gray,
            };
            let does = move |showing: &dyn Showing| {
                let Ok(()) = press(&pressing, key, showing);
            };
            let Ok(button) = match label {
                Written(says) => ButtonPress::written(says, now, does),
                Symbol(icon) => ButtonPress::new(*icon, now, does),
            };
            let Ok(button) = button.styled(style);

            presses.push(button);
        }

        let Ok(row) = Row::pressing(presses, 0);

        rows.push(row);
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_panel::page::Nowhere;

    fn pressed_on_the_card(held: &Shared, row: u32, which: u32) {
        let Ok(rows) = rows(held);
        let Ok(at) = console_core_number_conversion::index(row);
        let Ok(along) = console_core_number_conversion::index(which);
        let Some(press) = rows.get(at).and_then(|row| row.buttons.as_ref()).and_then(|across| across.presses.get(along))
        else {
            panic!("no key at row {row}, column {which}");
        };

        let _ = (press.does)(&Nowhere);
    }

    fn big(held: &Shared) -> String {
        let Ok(rows) = rows(held);

        match rows.first().and_then(|row| row.headline.as_ref()) {
            Some(headline) => headline.big.clone(),
            None => panic!("the card lost the number"),
        }
    }

    #[test]
    fn the_keys_on_the_card_do_the_sum_they_say() {
        let held: Shared = Arc::new(Mutex::new(Sum::default()));

        pressed_on_the_card(&held, 3, 2);
        pressed_on_the_card(&held, 3, 3);
        pressed_on_the_card(&held, 2, 0);
        pressed_on_the_card(&held, 5, 3);

        assert_eq!(big(&held), "-1", "6 \u{2212} 7 = came to something else");
    }

    #[test]
    fn every_key_on_the_card_says_what_it_does() {
        let held: Shared = Arc::new(Mutex::new(Sum::default()));
        let Ok(rows) = rows(&held);
        let on_the_card: Vec<_> = rows.iter().filter_map(|row| row.buttons.as_ref()).map(|across| across.presses.len()).collect();
        let on_the_keypad: Vec<_> = KEYPAD.iter().map(|row| row.len()).collect();

        assert_eq!(on_the_card, on_the_keypad);
    }

    #[test]
    fn the_operator_waiting_is_lit_on_the_card() {
        let held: Shared = Arc::new(Mutex::new(Sum::default()));

        pressed_on_the_card(&held, 4, 0);
        pressed_on_the_card(&held, 2, 3);

        let Ok(rows) = rows(&held);
        let lit: Vec<Active> = match rows.get(2).and_then(|row| row.buttons.as_ref()) {
            Some(across) => across.presses.iter().map(|press| press.now).collect(),
            None => panic!("the card lost a row of keys"),
        };

        assert_eq!(lit, vec![Active::No, Active::No, Active::No, Active::Yes]);
    }
}
