//! A month, drawn.
//!
//! ```text
//!     calendar-panel
//! ```
//!
//! The bar's clock opens it, and what it is for is the thing a clock cannot
//! say: which day of the week the fourteenth is, and how far that is from
//! today. So it is one page, a month wide, with today lit the way every other
//! panel lights what is already true.
//!
//! The month is a level rather than a pair of buttons. Left and right on a
//! level are already a promise this desktop keeps -- the d-pad, a swipe across
//! the row, and the two marks drawn on its ends -- so a month that is a level
//! is three ways of reaching the month before it, and none of them had to be
//! written here. What the row carries is [`crate::month::Month`] stepped by
//! however far the panel has walked from the month it opened in, which is the
//! whole of this panel's state: nothing is remembered between openings,
//! because a calendar opened tomorrow that is still showing last March is a
//! calendar that lied about what day it is.
//!
//! Today is read each time the rows are built rather than once when the card
//! is, because a panel left open over midnight is a panel that would otherwise
//! keep lighting yesterday.

use console_actor::Actor;
use console_core_arguments::{Command, Operands};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_panel::card::{Card, Door};
use console_panel::marks;
use console_panel::page::{Active, Aside, Cell, Ends, Page, Row, Rows};

use crate::month::{self, Day, Grid, Holds, Month};

pub const WHO: &str = "calendar-panel";

const DOOR: &str = "calendar";

const TITLE: &str = "Calendar";

const WHERE_IT_OPENED: i32 = 0;

const TODAY: &str = "+%Y %m %d";

const NO_DAY: &str = "Date Unavailable";

struct Calendar;

impl Machine for Calendar {
  type Input = ();
  type State = i32;
  type Request = i32;
  type Effect = Never;

  fn initialize(
    _input: &(),
    _previous: Option<i32>,
    _effects: &mut Queue<Never>,
  ) -> Result<i32, Never> {
    Ok(WHERE_IT_OPENED)
  }

  fn handle(offset: i32, step: i32, _effects: &mut Queue<Never>) -> Result<i32, Never> {
    Ok(offset.wrapping_add(step))
  }
}

pub const COMMAND: Command = Command {
  name: "calendar-panel",
  about: "the calendar",
  flags: &[],
  operands: Operands::None,
};

pub fn door(_argv: &[String]) -> Result<Door, Never> {
  Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
  Card::supervised::<Calendar, _>((), pages)
}

fn pages(held: &Actor<Calendar>) -> Result<Vec<Page>, Never> {
  let reading = held.clone();
  let compute = move || {
    let Ok(rows) = rows(&reading);

    rows
  };
  let Ok(rows) = Rows::computed(compute.clone());
  let Ok(page) = Page::new(TITLE, rows);
  let Ok(with_preview) = page.meanwhile(compute);

  Ok(vec![with_preview])
}

fn rows(held: &Actor<Calendar>) -> Result<Vec<Row>, Never> {
  let Ok(today) = today();
  let offset = held.get();

  let (today_3, months) = match (today, offset) {
    (Some(today_2), Ok(months)) => (today_2, months),
    (None, _) | (Some(_), Err(console_actor::Closed)) => {
      let Ok(row) = Row::placeholder(NO_DAY);

      return Ok(vec![row]);
    }
  };

  let Ok(opened) = today_3.month();
  let Ok(at) = opened.add_months(months);
  let Ok(said) = run_cal(at);
  let Ok(grid) = month::read(&said);

  let Ok(over) = month_row(held, &grid);
  let Ok(weekdays) = weekday_row(&grid);
  let mut rows = vec![over, weekdays];

  for week in &grid.weeks {
    let Ok(row) = week_row(week, at, today_3);

    rows.push(row);
  }

  Ok(rows)
}

fn month_row(held: &Actor<Calendar>, grid: &Grid) -> Result<Row, Never> {
  let stepping = held.clone();
  let Ok(row) = Row::text(&grid.said, Aside(""));
  let Ok(row_2) = row.adjusting(move |by| {
        #[cfg_attr(dylint_lib = "explicit060_no_reasonless_discard", expect(explicit060_no_reasonless_discard, reason = "the callback can only fail after the calendar actor has closed, when no panel remains to move or show an error"))]
        let _ = stepping.send(by);
    });

  row_2.ended(Ends {
    less: marks::BEFORE,
    more: marks::AFTER,
  })
}

fn weekday_row(grid: &Grid) -> Result<Row, Never> {
  let mut cells = Vec::new();

  for weekday in &grid.weekdays {
    let Ok(cell) = Cell::new(weekday, Active::No);

    cells.push(cell);
  }

  Row::naming_cells(cells)
}

fn week_row(week: &[String], at: Month, today: Day) -> Result<Row, Never> {
  let mut cells = Vec::new();

  for day in week {
    let Ok(now) = highlight(day, at, today);
    let Ok(cell) = Cell::new(day, now);

    cells.push(cell);
  }

  Row::celled(cells)
}

fn highlight(day: &str, at: Month, today: Day) -> Result<Active, Never> {
  let Ok(holds) = at.holds(today);

  match holds {
    Holds::No => return Ok(Active::No),
    Holds::Yes => {}
  }

  Ok(
    match ::core::primitive::bool::then_some(day.parse::<i32>() == Ok(today.day), ()) {
      ::core::option::Option::Some(()) => Active::Yes,
      ::core::option::Option::None => Active::No,
    },
  )
}

fn run_cal(at: Month) -> Result<String, Never> {
  let Ok(mut command) = Program::Cal.command();

  let out = command
    .arg(at.month.to_string())
    .arg(at.year.to_string())
    .output();

  let text = match out {
    Ok(output) => String::from_utf8(output.stdout),
    Err(_the_machine_will_not_draw_a_month) => return Ok(String::new()),
  };

  Ok(match text {
    Ok(said) => said,
    Err(_calendar_output_is_not_utf8) => String::new(),
  })
}

fn today() -> Result<Option<Day>, Never> {
  let Ok(mut command) = Program::Date.command();

  let out = command.arg(TODAY).output();

  let output = match out {
    Ok(output) => output,
    Err(_the_machine_will_not_say_what_day_it_is) => return Ok(None),
  };

  match std::str::from_utf8(&output.stdout) {
    Ok(said) => read_day(said),
    Err(_date_output_is_not_utf8) => Ok(None),
  }
}

fn read_day(said: &str) -> Result<Option<Day>, Never> {
  let mut words = said.split_whitespace();

  let Ok(year) = number(words.next());
  let Ok(month) = number(words.next());
  let Ok(day) = number(words.next());

  let year_3 = match year {
    Some(year_2) => year_2,
    None => return Ok(None),
  };

  let month_3 = match month {
    Some(month_2) => month_2,
    None => return Ok(None),
  };

  let day_3 = match day {
    Some(day_2) => day_2,
    None => return Ok(None),
  };

  Ok(Some(Day {
    year: year_3,
    month: month_3,
    day: day_3,
  }))
}

fn number(word: Option<&str>) -> Result<Option<i32>, Never> {
  let word_3 = match word {
    Some(word_2) => word_2,
    None => return Ok(None),
  };

  Ok(match word_3.parse::<i32>() {
    Ok(number) => Some(number),
    Err(_the_machine_said_something_that_is_not_a_number) => None,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_level_steps_the_month_in_both_directions() -> Result<(), Box<dyn std::error::Error>> {
    let console_actor::Booted { actor, scope, .. } = console_actor::boot::<Calendar>(())?;
    let Ok(grid) = month::read("October 2026");
    let Ok(row) = month_row(&actor, &grid);
    let adjust = row.level.ok_or("the month row must expose its level")?;

    adjust(1);
    assert_eq!(actor.get(), Ok(1));
    adjust(-1);
    assert_eq!(actor.get(), Ok(0));
    let Ok(()) = scope.close();

    Ok(())
  }

  #[test]
  fn the_day_the_machine_said_is_read_through_the_noughts_in_front_of_it() {
    let Ok(day) = read_day("2026 09 07");

    assert_eq!(
      day,
      Some(Day {
        year: 2026,
        month: 9,
        day: 7
      })
    );
  }

  #[test]
  fn a_machine_that_would_not_say_what_day_it_is_is_not_a_day_in_1970() {
    let Ok(nothing) = read_day("");

    assert_eq!(nothing, None);

    let Ok(half) = read_day("2026 09");

    assert_eq!(half, None);
  }

  #[test]
  fn today_is_lit_only_in_the_month_that_holds_it() {
    let today = Day {
      year: 2026,
      month: 10,
      day: 9,
    };
    let Ok(at) = today.month();
    let Ok(next) = at.after();

    let Ok(lit) = highlight("9", at, today);
    let Ok(elsewhere) = highlight("9", next, today);
    let Ok(another) = highlight("8", at, today);
    let Ok(blank) = highlight("", at, today);

    assert_eq!(lit, Active::Yes);
    assert_eq!(elsewhere, Active::No);
    assert_eq!(another, Active::No);
    assert_eq!(blank, Active::No);
  }
}
