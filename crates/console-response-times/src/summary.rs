//! What the store says, added up.
//!
//! Not averages. A mean is what a handheld is like on a good day with fifty
//! quick openings hiding the three that made somebody put the device down, and
//! the three are the whole question. So: the middle one, the slow tenth, and
//! the worst there has ever been.
//!
//! And no ninety-ninth. With forty openings the ninety-ninth is the worst
//! sample wearing a percentile's name, which reads as a measurement and is a
//! coincidence. The slow tenth is not printed either until there are ten of
//! them to take it from, because a tenth of nine is the same lie one place
//! further along.


use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_usize};
use std::cmp::Reverse;
use std::time::Duration;

use crate::line::{Entry, ms};

pub const ENOUGH: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct Spread {
    pub many: usize,
    pub middle: Duration,
    pub high: Option<Duration>,
    pub worst: Duration,
}

pub const BUSY: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct About {
    pub who: String,
    pub what: String,
    pub waited: Spread,
    pub worst_load: f64,
    pub marks: Vec<(String, Spread)>,
}

pub fn at_share(sorted: &[Duration], share: f64) -> Result<Duration, Never> {
    match sorted.is_empty() {
        true => return Ok(Duration::ZERO),
        false => {}
    }

    let Ok(many) = sorted.len().float();
    let Ok(rank) = toward_zero_usize((share * many).ceil().max(1.0));

    let at = rank.min(sorted.len()).saturating_sub(1);

    Ok(sorted.get(at).copied().unwrap_or(Duration::ZERO))
}

pub fn spread(mut took: Vec<Duration>) -> Result<Spread, Never> {
    took.sort_unstable();

    let Ok(middle) = at_share(&took, 0.5);
    let high = match took.len() >= ENOUGH {
        true => {
            let Ok(high) = at_share(&took, 0.9);

            Some(high)
        }
        false => None,
    };

    Ok(Spread {
        many: took.len(),
        middle,
        high,
        worst: took.last().copied().unwrap_or_default(),
    })
}

pub fn about(entries: &[Entry]) -> Result<Vec<About>, Never> {
    let mut kinds: Vec<(String, String)> = Vec::new();

    for entry in entries {
        let kind = (entry.who.clone(), entry.what.clone());

        match kinds.contains(&kind) {
            true => {}
            false => kinds.push(kind),
        }
    }

    let mut gathered: Vec<About> = kinds
        .into_iter()
        .map(|(who, what)| {
            let mine: Vec<&Entry> =
                entries.iter().filter(|entry| entry.who == who && entry.what == what).collect();
            let worst = mine.iter().max_by_key(|entry| entry.waited);

            let Ok(waited) = spread(mine.iter().map(|entry| entry.waited).collect());
            let Ok(marks) = stretches(&mine);

            About {
                who,
                what,
                waited,
                worst_load: worst.map_or(0.0, |entry| entry.load),
                marks,
            }
        })
        .collect();
    gathered.sort_by_key(|about| Reverse(about.waited.middle));

    Ok(gathered)
}

fn stretches(entries: &[&Entry]) -> Result<Vec<(String, Spread)>, Never> {
    let mut named: Vec<String> = Vec::new();

    for entry in entries {
        for (name, _) in &entry.marks {
            match named.contains(name) {
                true => {}
                false => named.push(name.clone()),
            }
        }
    }

    let mut all: Vec<(String, Spread)> = named
        .into_iter()
        .map(|name| {
            let took: Vec<Duration> = entries
                .iter()
                .flat_map(|entry| entry.marks.iter())
                .filter(|(mark, _)| *mark == name)
                .map(|(_, took)| *took)
                .collect();

            let Ok(held) = spread(took);

            (name, held)
        })
        .collect();
    all.sort_by_key(|(_, spread)| Reverse(spread.middle));

    Ok(all)
}

pub fn told(about: &About) -> Result<String, Never> {
    let Ok(middle) = ms(about.waited.middle);
    let Ok(worst) = ms(about.waited.worst);
    let slow = match about.waited.high {
        Some(high) => {
            let Ok(high) = ms(high);

            format!(", slow tenth {high:.0}ms")
        }
        None => String::new(),
    };

    let mut said = format!(
        "{} {} \u{2014} {} of them, middle {middle:.0}ms{slow}, worst {worst:.0}ms\n",
        about.who, about.what, about.waited.many,
    );

    match about.worst_load >= BUSY {
        true => {
            said.pop();
            said.push_str(&format!(", on a machine at {:.1}\n", about.worst_load));
        }
        false => {}
    }

    for (name, spread) in &about.marks {
        let Ok(middle) = ms(spread.middle);
        let over = match spread.worst.checked_sub(spread.middle) {
            Some(over) => {
                let Ok(over) = ms(over);

                match over >= 1.0 {
                    true => {
                        let Ok(worst) = ms(spread.worst);

                        format!("   worst {worst:.0}ms")
                    }
                    false => String::new(),
                }
            }
            None => String::new(),
        };

        said.push_str(&format!("    {name:16} {middle:>8.1}ms{over}\n"));
    }

    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waits(ms: &[u64]) -> Vec<Duration> {
        ms.iter().map(|each| Duration::from_millis(*each)).collect()
    }

    fn opening(who: &str, waited: u64, marks: &[(&str, u64)]) -> Entry {
        Entry {
            at: 0,
            up: 0.0,
            load: 0.0,
            who: who.to_string(),
            what: "opening".to_string(),
            waited: Duration::from_millis(waited),
            marks: marks
                .iter()
                .map(|(name, took)| ((*name).to_string(), Duration::from_millis(*took)))
                .collect(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn the_share_is_one_of_the_waits_and_not_a_number_between_two() {
        let sorted = waits(&[10, 20, 30, 40]);
        assert_eq!(at_share(&sorted, 0.5), Ok(Duration::from_millis(20)));
        assert_eq!(at_share(&sorted, 0.9), Ok(Duration::from_millis(40)));
        assert_eq!(at_share(&sorted, 0.0), Ok(Duration::from_millis(10)));
    }

    #[test]
    fn nothing_waited_for_is_no_time_at_all() {
        assert_eq!(at_share(&[], 0.5), Ok(Duration::ZERO));

        let Ok(nothing) = spread(Vec::new());

        assert_eq!(nothing.many, 0);
    }

    #[test]
    fn the_slow_tenth_is_withheld_until_there_are_ten_to_take_it_from() {
        let Ok(nine) = spread(waits(&[1, 2, 3, 4, 5, 6, 7, 8, 9]));
        let Ok(ten) = spread(waits(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 100]));

        assert_eq!(nine.high, None);
        assert_eq!(ten.high, Some(Duration::from_millis(9)));
        assert_eq!(ten.middle, Duration::from_millis(5));
        assert_eq!(ten.worst, Duration::from_millis(100));
    }

    #[test]
    fn the_surfaces_are_ordered_by_the_middle_and_not_by_the_worst() {
        let entries = vec![
            opening("launcher", 400, &[]),
            opening("launcher", 420, &[]),
            opening("notices-panel", 90, &[]),
            opening("notices-panel", 3000, &[]),
        ];
        let Ok(gathered) = about(&entries);

        assert_eq!(gathered[0].who, "launcher");
        assert_eq!(gathered[1].who, "notices-panel");
    }

    #[test]
    fn the_stretches_are_ordered_by_which_of_them_is_the_slow_one() {
        let entries = vec![
            opening("launcher", 400, &[("press", 10), ("gtk", 130), ("placed", 240)]),
            opening("launcher", 380, &[("press", 12), ("gtk", 120), ("placed", 230)]),
        ];
        let Ok(gathered) = about(&entries);

        let named: Vec<&str> = gathered[0].marks.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(named, ["placed", "gtk", "press"]);
    }

    #[test]
    fn a_stretch_that_only_some_openings_had_is_counted_over_those() {
        let entries = vec![
            opening("launcher", 400, &[("gtk", 100)]),
            opening("launcher", 700, &[("gtk", 100), ("screen", 300)]),
        ];
        let Ok(gathered) = about(&entries);

        let screen = gathered[0]
            .marks
            .iter()
            .find(|(name, _)| name == "screen")
            .expect("the wait for the screen");
        assert_eq!(screen.1.many, 1);
        assert_eq!(screen.1.middle, Duration::from_millis(300));
    }

    #[test]
    fn a_worst_that_happened_while_the_machine_was_busy_says_so() {
        let mut busy = opening("launcher", 1007, &[]);
        busy.load = 5.2;
        let quiet = opening("launcher", 148, &[]);

        let Ok(gathered) = about(&[quiet, busy]);
        let Ok(said) = told(&gathered[0]);

        assert!(said.contains("worst 1007ms, on a machine at 5.2"), "{said}");
    }

    #[test]
    fn a_worst_on_a_quiet_machine_is_told_without_a_word_about_the_machine() {
        let Ok(gathered) = about(&[opening("launcher", 148, &[])]);
        let Ok(said) = told(&gathered[0]);

        assert!(!said.contains("on a machine"), "{said}");
    }

    #[test]
    fn who_waited_for_what_is_what_makes_two_lines_the_same_kind() {
        let entries = vec![
            opening("launcher", 400, &[]),
            {
                let mut list = opening("launcher", 900, &[]);
                list.what = "list".to_string();
                list
            },
        ];
        let Ok(gathered) = about(&entries);

        assert_eq!(gathered.len(), 2);
    }
}
