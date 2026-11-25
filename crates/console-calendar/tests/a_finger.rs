//! The calendar, opened alone and asked whether a hand could walk it.
//!
//!     cargo test -p console-calendar --test a_finger
//!
//! One panel, one nested compositor, a few seconds. What `month` can be asked
//! without a screen is how a month is read and how one steps to the next; what
//! it cannot be asked is whether the panel selects the next month. A shoulder
//! press sends the same page key that moves between tabs in other panels.
//!
//! Under load a press could be sent before the compositor had handed the panel
//! the keyboard, and it went to nobody or arrived merged with the one after it,
//! so walking back read as a month lost or two months back. The panel says
//! when it holds the keyboard and a press waits for that, which is the first
//! thing pressed here.

use std::error::Error;

use console_panel::description::{Description, KeyboardFocus};
use console_test_stages::panels::Panel;

fn describe_after(keys: &[&str]) -> Result<Vec<Description>, Box<dyn Error>> {
  let Ok(mut panel) = Panel::opening("calendar-panel", &[]);

  for key in keys {
    panel.key(key)?;
  }

  let drawn = panel
    .descriptions()
    .map_err(|why| format!("the calendar could not be asked what it drew: {why}"))?;

  Ok(drawn)
}

fn month(card: &Description) -> Result<String, Box<dyn Error>> {
  match card.tab.as_str() {
    "" => Err(Box::from("the calendar has no active month tab")),
    month => Ok(month.to_owned()),
  }
}

fn first(every: &[Description]) -> Result<&Description, Box<dyn Error>> {
  every
    .first()
    .ok_or_else(|| Box::from("the calendar drew nothing at all"))
}

fn last(every: &[Description]) -> Result<&Description, Box<dyn Error>> {
  every
    .last()
    .ok_or_else(|| Box::from("the calendar drew nothing at all"))
}

#[test]
#[ignore = "run just test e2e-panels in a disposable NixOS VM"]
fn it_opens_saying_which_month_it_is_showing() -> Result<(), Box<dyn Error>> {
  let every = describe_after(&[])?;
  let card = first(&every)?;
  let said = month(card)?;
  let words: Vec<&str> = said.split_whitespace().collect();
  assert!(
    card
      .lines
      .iter()
      .all(|line| line.says != "Calendar" && line.says != said)
  );

  assert_eq!(words.len(), 2, "the month it opened on is written {said:?}");
  assert!(
    words
      .last()
      .is_some_and(|year| year.chars().all(|letter| letter.is_ascii_digit())),
    "the month it opened on names no year: {said:?}"
  );

  Ok(())
}

#[test]
#[ignore = "run just test e2e-panels in a disposable NixOS VM"]
fn the_shoulder_walks_to_the_month_after_this_one() -> Result<(), Box<dyn Error>> {
  let every = describe_after(&["Page_Down"])?;
  let card = first(&every)?;
  let opened = month(card)?;
  let card_2 = last(&every)?;
  let on = month(card_2)?;

  assert_ne!(
    opened, on,
    "moving to the next tab drew the same month again"
  );

  Ok(())
}

#[test]
#[ignore = "run just test e2e-panels in a disposable NixOS VM"]
fn the_keyboard_is_held_before_the_first_press() -> Result<(), Box<dyn Error>> {
  let every = describe_after(&["Page_Down"])?;
  let listening = every
    .iter()
    .any(|card| (card.presses, card.keyboard) == (0, KeyboardFocus::Here));

  assert!(
    listening,
    "the calendar never said it held the keyboard before it was pressed, so the press went to whoever did"
  );

  Ok(())
}

#[test]
#[ignore = "run just test e2e-panels in a disposable NixOS VM"]
fn walking_back_comes_back() -> Result<(), Box<dyn Error>> {
  let every = describe_after(&["Page_Down", "Page_Up"])?;
  let card = first(&every)?;
  let opened = month(card)?;
  let card_2 = last(&every)?;
  let back = month(card_2)?;

  assert_eq!(
    opened, back,
    "a month on and a month back is not where it started"
  );

  Ok(())
}
