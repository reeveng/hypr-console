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

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_panel::card::{Card, Door};
use console_panel::marks;
use console_panel::page::{Aside, Cell, Ends, Active, Page, Row, Rows};

use crate::month::{self, Day, Grid, Holds, Month};

pub const WHO: &str = "calendar-panel";

const DOOR: &str = "calendar";

const TITLE: &str = "Calendar";

const WHERE_IT_OPENED: i32 = 0;

const TODAY: &str = "+%Y %m %d";

const NO_DAY: &str = "Date Unavailable";

type Shared = Arc<AtomicI32>;

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    let held: Shared = Arc::new(AtomicI32::new(WHERE_IT_OPENED));

    Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }))
}

fn pages(held: &Shared) -> Result<Vec<Page>, Never> {
    let reading = Arc::clone(held);
    let Ok(rows) = Rows::asked(move || {
        let Ok(rows) = rows(&reading);

        rows
    });
    let Ok(page) = Page::new(TITLE, rows);

    Ok(vec![page])
}

fn rows(held: &Shared) -> Result<Vec<Row>, Never> {
    let Ok(today) = today();

    let today = match today {
        Some(today) => today,
        None => {
            let Ok(row) = Row::nothing(NO_DAY);

            return Ok(vec![row]);
        }
    };

    let Ok(opened) = today.month();
    let Ok(at) = opened.stepped(held.load(Ordering::Relaxed));
    let Ok(said) = drawn(at);
    let Ok(grid) = month::read(&said);

    let Ok(over) = month_row(held, &grid);
    let Ok(weekdays) = named(&grid);
    let mut rows = vec![over, weekdays];

    for week in &grid.weeks {
        let Ok(row) = week_row(week, at, today);

        rows.push(row);
    }

    Ok(rows)
}

fn month_row(held: &Shared, grid: &Grid) -> Result<Row, Never> {
    let stepping = Arc::clone(held);
    let Ok(row) = Row::said(&grid.said, Aside(""));
    let Ok(row) = row.leveled(Arc::new(move |by| {
        stepping.fetch_add(by, Ordering::Relaxed);
    }));

    row.ended(Ends { less: marks::BEFORE, more: marks::AFTER })
}

fn named(grid: &Grid) -> Result<Row, Never> {
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
        let Ok(now) = standing(day, at, today);
        let Ok(cell) = Cell::new(day, now);

        cells.push(cell);
    }

    Row::celled(cells)
}

fn standing(day: &str, at: Month, today: Day) -> Result<Active, Never> {
    let Ok(holds) = at.holds(today);

    match holds {
        Holds::No => return Ok(Active::No),
        Holds::Yes => {},
    }

    Ok(match day.parse::<i32>() == Ok(today.day) {
        true => Active::Yes,
        false => Active::No,
    })
}

fn drawn(at: Month) -> Result<String, Never> {
    let Ok(mut command) = Program::Cal.command();

    let out = command.arg(at.month.to_string()).arg(at.year.to_string()).output();

    Ok(match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_the_machine_will_not_draw_a_month) => String::new(),
    })
}

fn today() -> Result<Option<Day>, Never> {
    let Ok(mut command) = Program::Date.command();

    let out = command.arg(TODAY).output();

    let said = match out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_the_machine_will_not_say_what_day_it_is) => return Ok(None),
    };

    read_day(&said)
}

fn read_day(said: &str) -> Result<Option<Day>, Never> {
    let mut words = said.split_whitespace();

    let Ok(year) = number(words.next());
    let Ok(month) = number(words.next());
    let Ok(day) = number(words.next());

    let year = match year {
        Some(year) => year,
        None => return Ok(None),
    };

    let month = match month {
        Some(month) => month,
        None => return Ok(None),
    };

    let day = match day {
        Some(day) => day,
        None => return Ok(None),
    };

    Ok(Some(Day { year, month, day }))
}

fn number(word: Option<&str>) -> Result<Option<i32>, Never> {
    let word = match word {
        Some(word) => word,
        None => return Ok(None),
    };

    Ok(match word.parse::<i32>() {
        Ok(number) => Some(number),
        Err(_the_machine_said_something_that_is_not_a_number) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_day_the_machine_said_is_read_through_the_noughts_in_front_of_it() {
        let Ok(day) = read_day("2026 09 07");

        assert_eq!(day, Some(Day { year: 2026, month: 9, day: 7 }));
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
        let today = Day { year: 2026, month: 10, day: 9 };
        let Ok(at) = today.month();
        let Ok(next) = at.after();

        let Ok(lit) = standing("9", at, today);
        let Ok(elsewhere) = standing("9", next, today);
        let Ok(another) = standing("8", at, today);
        let Ok(blank) = standing("", at, today);

        assert_eq!(lit, Active::Yes);
        assert_eq!(elsewhere, Active::No);
        assert_eq!(another, Active::No);
        assert_eq!(blank, Active::No);
    }
}
