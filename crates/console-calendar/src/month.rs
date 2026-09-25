//! Which month is being looked at, and the reading of the one `cal` draws.
//!
//! None of the arithmetic a calendar is made of is here, because none of it is
//! this desktop's to get right: which day a month starts on, how long February
//! is this year, and which day the week starts on in the language someone set
//! are three questions `cal` has answered correctly since before this machine
//! existed, in one process, in the locale it is run in. What is here is the
//! walk from one month to the next, which is the only thing a person pressing
//! the ends of a row is asking for, and the reading of what came back.
//!
//! A month is read as columns rather than as words between spaces, because the
//! blanks are where the answer is: a week beginning on a Wednesday is three
//! empty columns and then a 1, and a line split on whitespace says only that
//! it has five days in it and not which five. [`COLUMN`] is how wide `cal`
//! writes one, and the first line of its own answer -- the month and the year,
//! centered -- is what the panel says over the grid, so the words stay the
//! locale's rather than being spelled again here.

use console_core_never::Never;
use console_core_number_conversion::index;

pub const WEEK: u32 = 7;

const COLUMN: u32 = 3;

const A_YEAR: i32 = 12;

const THE_FIRST_MONTH: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month {
    pub year: i32,
    pub month: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    pub year: i32,
    pub month: i32,
    pub day: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grid {
    pub said: String,
    pub weekdays: Vec<String>,
    pub weeks: Vec<Vec<String>>,
}

impl Day {
    pub fn month(self) -> Result<Month, Never> {
        Ok(Month { year: self.year, month: self.month })
    }
}

impl Month {
    pub fn after(self) -> Result<Self, Never> {
        Ok(match self.month >= A_YEAR {
            true => Month { year: self.year.saturating_add(1), month: THE_FIRST_MONTH },
            false => Month { year: self.year, month: self.month.saturating_add(1) },
        })
    }

    pub fn before(self) -> Result<Self, Never> {
        Ok(match self.month <= THE_FIRST_MONTH {
            true => Month { year: self.year.saturating_sub(1), month: A_YEAR },
            false => Month { year: self.year, month: self.month.saturating_sub(1) },
        })
    }

    pub fn stepped(self, by: i32) -> Result<Self, Never> {
        let mut at = self;
        let mut left = by.saturating_abs();

        while left > 0 {
            let Ok(stepped) = match by > 0 {
                true => at.after(),
                false => at.before(),
            };

            at = stepped;
            left = left.saturating_sub(1);
        }

        Ok(at)
    }

    pub fn holds(self, day: Day) -> Result<Holds, Never> {
        Ok(match self.year == day.year && self.month == day.month {
            true => Holds::Yes,
            false => Holds::No,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    Yes,
    No,
}

pub fn read(said: &str) -> Result<Grid, Never> {
    let mut lines = said.lines();

    let over = match lines.next() {
        Some(line) => line.trim().to_string(),
        None => String::new(),
    };

    let weekdays = match lines.next() {
        Some(line) => {
            let Ok(cells) = cells(line);

            cells
        }
        None => Vec::new(),
    };

    let weeks = lines
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let Ok(cells) = cells(line);

            cells
        })
        .collect();

    Ok(Grid { said: over, weekdays, weeks })
}

fn cells(line: &str) -> Result<Vec<String>, Never> {
    let letters: Vec<char> = line.chars().collect();
    let Ok(column) = index(COLUMN);
    let Ok(week) = index(WEEK);

    let mut cells: Vec<String> = letters
        .chunks(column)
        .map(|column| column.iter().collect::<String>().trim().to_string())
        .collect();

    cells.resize(week, String::new());

    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OCTOBER: &str = r"    October 2026    
Su Mo Tu We Th Fr Sa
             1  2  3
 4  5  6  7  8  9 10
11 12 13 14 15 16 17
18 19 20 21 22 23 24
25 26 27 28 29 30 31
                    
";

    fn week(grid: &Grid, at: u32) -> Vec<String> {
        match grid.weeks.get(console_core_number_conversion::index(at).unwrap()) {
            Some(week) => week.clone(),
            None => Vec::new(),
        }
    }

    #[test]
    fn the_month_after_december_is_january_of_the_next_year() {
        let Ok(after) = Month { year: 2026, month: 12 }.after();

        assert_eq!(after, Month { year: 2027, month: 1 });

        let Ok(before) = Month { year: 2027, month: 1 }.before();

        assert_eq!(before, Month { year: 2026, month: 12 });
    }

    #[test]
    fn a_year_of_steps_either_way_comes_back_to_the_month_it_started_in() {
        let from = Month { year: 2026, month: 9 };

        let Ok(on) = from.stepped(12);
        let Ok(back) = on.stepped(-12);

        assert_eq!(on, Month { year: 2027, month: 9 });
        assert_eq!(back, from);
    }

    #[test]
    fn standing_still_is_not_a_step() {
        let from = Month { year: 2026, month: 9 };

        let Ok(nowhere) = from.stepped(0);

        assert_eq!(nowhere, from);
    }

    #[test]
    fn a_week_that_starts_mid_week_keeps_the_days_under_their_own_names() {
        let Ok(grid) = read(OCTOBER);

        assert_eq!(grid.said, "October 2026");
        assert_eq!(grid.weekdays, ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]);
        assert_eq!(week(&grid, 0), ["", "", "", "", "1", "2", "3"]);

        for said in &grid.weeks {
            assert_eq!(u32::try_from(said.len()).unwrap(), WEEK, "a week is seven columns wide whatever is in it");
        }
    }

    #[test]
    fn a_month_is_read_without_the_blank_line_under_it() {
        let Ok(grid) = read(OCTOBER);

        assert_eq!(grid.weeks.len(), 5);
        assert_eq!(week(&grid, 4), ["25", "26", "27", "28", "29", "30", "31"]);
    }

    #[test]
    fn a_month_nothing_drew_is_a_month_with_nothing_in_it() {
        let Ok(grid) = read("");

        assert_eq!(grid, Grid::default());
    }

    #[test]
    fn the_month_a_day_is_in_is_the_only_one_that_holds_it() {
        let day = Day { year: 2026, month: 10, day: 9 };
        let Ok(at) = day.month();

        let Ok(holds) = at.holds(day);
        let Ok(next) = at.after();
        let Ok(elsewhere) = next.holds(day);

        assert_eq!(holds, Holds::Yes);
        assert_eq!(elsewhere, Holds::No);
    }
}
