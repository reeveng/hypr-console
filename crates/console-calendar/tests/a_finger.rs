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

use console_panel::description::Description;
use console_test_stages::panels::Panel;

fn drawn(keys: &[&str]) -> Vec<Description> {
    let Ok(mut panel) = Panel::opening("calendar-panel", &[]);

    for key in keys {
        match panel.key(key) {
            Ok(()) => {},
            Err(why) => panic!("{why}"),
        }
    }

    match panel.drawn() {
        Ok(drawn) => drawn,
        Err(why) => panic!("the calendar could not be asked what it drew: {why}"),
    }
}

fn month(card: &Description) -> String {
    let said = card.lines.iter().find(|line| !line.says.is_empty());

    match said {
        Some(line) => line.says.clone(),
        None => String::new(),
    }
}

fn first(every: &[Description]) -> Description {
    match every.first() {
        Some(card) => card.clone(),
        None => panic!("the calendar drew nothing at all"),
    }
}

fn last(every: &[Description]) -> Description {
    match every.last() {
        Some(card) => card.clone(),
        None => panic!("the calendar drew nothing at all"),
    }
}

#[test]
fn it_opens_saying_which_month_it_is_showing() {
    let every = drawn(&[]);
    let said = month(&first(&every));
    let words: Vec<&str> = said.split_whitespace().collect();

    assert_eq!(words.len(), 2, "the month it opened on is written {said:?}");
    assert!(
        words.last().is_some_and(|year| year.chars().all(|letter| letter.is_ascii_digit())),
        "the month it opened on names no year: {said:?}"
    );
}

#[test]
fn the_dpad_walks_to_the_month_after_this_one() {
    let every = drawn(&["Right"]);
    let opened = month(&first(&every));
    let on = month(&last(&every));

    assert_ne!(opened, on, "right on the month drew the same month again");
}

#[test]
fn walking_back_comes_back() {
    let every = drawn(&["Right", "Left"]);
    let opened = month(&first(&every));
    let back = month(&last(&every));

    assert_eq!(opened, back, "a month on and a month back is not where it started");
}
