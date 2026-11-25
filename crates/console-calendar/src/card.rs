//! A month, drawn.
//!
//! ```text
//!     calendar-panel
//! ```
//!
//! The bar's clock opens the current month. The panel's tab strip can walk
//! either way from there, adding another month as it approaches either end.
//! Each month's tab carries the localized name and year from `cal`. The rows
//! start with the weekday names and highlight today in the month that holds it.

use console_core_arguments::{Command, Operands};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::card::{Card, Door};
use console_panel::page::{Active, Cell, Neighbor, Page, Row, Rows};

use crate::month::{self, Day, Grid, Holds, Month};

pub const WHO: &str = "calendar-panel";

const DOOR: &str = "calendar";

const TODAY: &str = "+%Y %m %d";

const NO_DAY: &str = "Date Unavailable";

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
  let Ok(now) = today();
  let at = match now {
    Some(day) => {
      let Ok(month) = day.month();
      month
    }
    None => return unavailable(),
  };
  let Ok(opening) = month_page(at);
  let current = match opening {
    Some(page) => page,
    None => return unavailable(),
  };
  let title = current.title.clone();
  let Ok(card) = Card::new(move || {
    let Ok(before) = neighbor(at, Neighbor::Before);
    let Ok(after) = neighbor(at, Neighbor::After);

    before.into_iter().chain([current]).chain(after).collect()
  });

  card.opening_at(Some(&title))
}

fn unavailable() -> Result<Card, Never> {
  Card::new(|| {
    let Ok(page) = Page::new(NO_DAY, Rows::Fixed(Vec::new()));
    vec![page]
  })
}

fn neighbor(at: Month, direction: Neighbor) -> Result<Option<Page>, Never> {
  let Ok(month) = match direction {
    Neighbor::Before => at.before(),
    Neighbor::After => at.after(),
  };

  match (month.year > 0 && month != at).then_some(()) {
    Some(()) => month_page(month),
    None => Ok(None),
  }
}

fn month_page(at: Month) -> Result<Option<Page>, Never> {
  let Ok(said) = run_cal(at);
  let Ok(grid) = month::read(&said);

  match grid.said.as_str() {
    "" => Ok(None),
    _ => {
      let Ok(page) = tab(at, grid);
      Ok(Some(page))
    }
  }
}

fn tab(at: Month, grid: Grid) -> Result<Page, Never> {
  let title = grid.said.clone();
  let compute = move || {
    let Ok(now) = today();
    let Ok(rows) = rows(&grid, at, now);

    rows
  };
  let Ok(rows) = Rows::computed(compute.clone());
  let Ok(page) = Page::new(&title, rows);
  let Ok(preview) = page.meanwhile(compute);

  preview.with_neighbors(move |direction| neighbor(at, direction))
}

fn rows(grid: &Grid, at: Month, today: Option<Day>) -> Result<Vec<Row>, Never> {
  let Ok(weekdays) = weekday_row(grid);
  let mut rows = vec![weekdays];

  for week in &grid.weeks {
    let Ok(row) = week_row(week, at, today);

    rows.push(row);
  }

  Ok(rows)
}

fn weekday_row(grid: &Grid) -> Result<Row, Never> {
  let mut cells = Vec::new();

  for weekday in &grid.weekdays {
    let Ok(cell) = Cell::new(weekday, Active::No);

    cells.push(cell);
  }

  Row::naming_cells(cells)
}

fn week_row(week: &[String], at: Month, today: Option<Day>) -> Result<Row, Never> {
  let mut cells = Vec::new();

  for day in week {
    let now = match today {
      Some(today_day) => {
        let Ok(active) = highlight(day, at, today_day);
        active
      }
      None => Active::No,
    };
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
  fn the_month_name_is_in_the_tab_and_the_grid_starts_with_weekdays()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(grid) = month::read(
      "    January 2027
Su Mo Tu We Th Fr Sa
                1  2",
    );
    let at = Month {
      year: 2027,
      month: 1,
    };
    let Ok(page) = tab(at, grid);
    assert_eq!(page.title, "January 2027");
    let Ok(rows) = page.rows.read();
    let first = rows.first().ok_or("missing weekdays")?;
    let labels: Vec<_> = first.cells.iter().map(|cell| cell.says.as_str()).collect();
    assert_eq!(labels, ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]);

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
