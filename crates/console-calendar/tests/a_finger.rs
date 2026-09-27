//! The calendar, opened alone and asked whether a hand could walk it.
//!
//!     cargo test -p console-calendar --test a_finger
//!
//! One panel, one nested compositor, a few seconds. What `month` can be asked
//! without a screen is how a month is read and how one steps to the next; what
//! it cannot be asked is whether anything on the panel steps it. The row that
//! carries the month is a level, and a level is three inputs at once -- the
//! d-pad, the two marks, a swipe -- none of which is written in this crate. So
//! the one thing worth pressing here is that the month a person is looking at
//! is the month the d-pad left them on, and that walking back comes back.

use std::error::Error;

use console_panel::description::Description;
use console_test_stages::panels::Panel;

fn describe_after(keys: &[&str]) -> Result<Vec<Description>, Box<dyn Error>> {
    let Ok(mut panel) = Panel::opening("calendar-panel", &[]);

    for key in keys {
        panel.key(key)?;
    }

    let drawn = panel.descriptions().map_err(|why| format!("the calendar could not be asked what it drew: {why}"))?;

    Ok(drawn)
}

fn month(card: &Description) -> Result<String, Box<dyn Error>> {
    match card.lines.iter().find(|line| !line.says.is_empty()) {
        Some(line) => Ok(line.says.clone()),
        None => Err(Box::from("the card says nothing at all")),
    }
}

fn first(every: &[Description]) -> Result<&Description, Box<dyn Error>> {
    every.first().ok_or_else(|| Box::from("the calendar drew nothing at all"))
}

fn last(every: &[Description]) -> Result<&Description, Box<dyn Error>> {
    every.last().ok_or_else(|| Box::from("the calendar drew nothing at all"))
}

#[test]
fn it_opens_saying_which_month_it_is_showing() -> Result<(), Box<dyn Error>> {
    let every = describe_after(&[])?;
    let card = first(&every)?;
    let said = month(card)?;
    let words: Vec<&str> = said.split_whitespace().collect();

    assert_eq!(words.len(), 2, "the month it opened on is written {said:?}");
    assert!(
        words.last().is_some_and(|year| year.chars().all(|letter| letter.is_ascii_digit())),
        "the month it opened on names no year: {said:?}"
    );

    Ok(())
}

#[test]
fn the_dpad_walks_to_the_month_after_this_one() -> Result<(), Box<dyn Error>> {
    let every = describe_after(&["Right"])?;
    let card = first(&every)?;
    let opened = month(card)?;
    let card = last(&every)?;
    let on = month(card)?;

    assert_ne!(opened, on, "right on the month drew the same month again");

    Ok(())
}

#[test]
fn walking_back_comes_back() -> Result<(), Box<dyn Error>> {
    let every = describe_after(&["Right", "Left"])?;
    let card = first(&every)?;
    let opened = month(card)?;
    let card = last(&every)?;
    let back = month(card)?;

    assert_eq!(opened, back, "a month on and a month back is not where it started");

    Ok(())
}
