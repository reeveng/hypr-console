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


use console_number::{Float, toward_zero_usize};
use std::cmp::Reverse;
use std::time::Duration;

use crate::line::{Entry, ms};

/// How many openings it takes before the slow tenth is a tenth of anything.
pub const ENOUGH: usize = 10;

/// What a set of waits was like.
#[derive(Debug, Clone, PartialEq)]
pub struct Spread {
    pub many: usize,
    pub middle: Duration,
    /// The slow tenth, where there have been enough of them to have one.
    pub high: Option<Duration>,
    pub worst: Duration,
}

/// The load at which a machine was doing something else as well.
///
/// Below this, whatever else was running was not what made an opening slow.
/// Above it, the number is about the machine and not about the panel -- which
/// is worth saying rather than hiding, because the worst openings in this store
/// were made by a handheld compiling its own desktop.
pub const BUSY: f64 = 1.0;

/// One kind of waiting, and everything the store holds about it.
#[derive(Debug, Clone, PartialEq)]
pub struct About {
    pub who: String,
    pub what: String,
    pub waited: Spread,
    /// What else the machine was doing during the worst of them.
    pub worst_load: f64,
    /// Where the time went, worst first, so the line that reads slowest is the
    /// one to look at.
    pub marks: Vec<(String, Spread)>,
}

/// The one at that share of the way along, counting from the fastest.
///
/// Nearest-rank, which picks a measurement that actually happened rather than
/// inventing one between two that did. A tenth of a millisecond of interpolated
/// precision is not worth a number nobody can go and find in the file.
pub fn at_share(sorted: &[Duration], share: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }

    let rank = toward_zero_usize((share * sorted.len().float()).ceil().max(1.0));
    sorted[rank.min(sorted.len()) - 1]
}

/// The middle, the slow tenth and the worst of a set of waits.
pub fn spread(mut took: Vec<Duration>) -> Spread {
    took.sort_unstable();
    Spread {
        many: took.len(),
        middle: at_share(&took, 0.5),
        high: (took.len() >= ENOUGH).then(|| at_share(&took, 0.9)),
        worst: took.last().copied().unwrap_or_default(),
    }
}

/// Everything in the store, gathered by who waited for what.
///
/// Slowest first, by the middle rather than by the worst: the one bad opening
/// on the fastest surface should not put it at the top of a page about which
/// surface is slow.
pub fn about(entries: &[Entry]) -> Vec<About> {
    let mut kinds: Vec<(String, String)> = Vec::new();

    for entry in entries {
        let kind = (entry.who.clone(), entry.what.clone());

        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }

    let mut gathered: Vec<About> = kinds
        .into_iter()
        .map(|(who, what)| {
            let mine: Vec<&Entry> =
                entries.iter().filter(|entry| entry.who == who && entry.what == what).collect();
            let worst = mine.iter().max_by_key(|entry| entry.waited);
            About {
                who,
                what,
                waited: spread(mine.iter().map(|entry| entry.waited).collect()),
                worst_load: worst.map_or(0.0, |entry| entry.load),
                marks: stretches(&mine),
            }
        })
        .collect();
    gathered.sort_by_key(|about| Reverse(about.waited.middle));
    gathered
}

/// Each stretch these waits were made of, worst of them first.
///
/// A stretch that is missing from some of the lines is summarised over the ones
/// that have it, because a panel that only sometimes waits for the screen is
/// still a panel that sometimes waits for the screen, and averaging in the
/// openings that did not would hide exactly that.
fn stretches(entries: &[&Entry]) -> Vec<(String, Spread)> {
    let mut named: Vec<String> = Vec::new();

    for entry in entries {
        for (name, _) in &entry.marks {
            if !named.contains(name) {
                named.push(name.clone());
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
            (name, spread(took))
        })
        .collect();
    all.sort_by_key(|(_, spread)| Reverse(spread.middle));
    all
}

/// One kind of waiting, written for reading.
pub fn told(about: &About) -> String {
    let mut said = format!(
        "{} {} \u{2014} {} of them, middle {:.0}ms{}, worst {:.0}ms\n",
        about.who,
        about.what,
        about.waited.many,
        ms(about.waited.middle),
        about
            .waited
            .high
            .map(|high| format!(", slow tenth {:.0}ms", ms(high)))
            .unwrap_or_default(),
        ms(about.waited.worst),
    );

    if about.worst_load >= BUSY {
        // Said rather than left out. An opening that happened while the machine
        // was busy is a real opening somebody had, and it is also not what the
        // panel is like.
        said.pop();
        said.push_str(&format!(", on a machine at {:.1}\n", about.worst_load));
    }

    for (name, spread) in &about.marks {
        said.push_str(&format!(
            "    {name:16} {:>8.1}ms{}\n",
            ms(spread.middle),
            spread.worst.checked_sub(spread.middle).map_or(String::new(), |over| {
                match ms(over) >= 1.0 {
                    true => format!("   worst {:.0}ms", ms(spread.worst)),
                    false => String::new(),
                }
            })
        ));
    }

    said
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

    /// The share is taken off a measurement that happened, so every number on
    /// the page can be gone and found in the file.
    #[test]
    fn the_share_is_one_of_the_waits_and_not_a_number_between_two() {
        let sorted = waits(&[10, 20, 30, 40]);
        assert_eq!(at_share(&sorted, 0.5), Duration::from_millis(20));
        assert_eq!(at_share(&sorted, 0.9), Duration::from_millis(40));
        assert_eq!(at_share(&sorted, 0.0), Duration::from_millis(10));
    }

    #[test]
    fn nothing_waited_for_is_no_time_at_all() {
        assert_eq!(at_share(&[], 0.5), Duration::ZERO);
        assert_eq!(spread(Vec::new()).many, 0);
    }

    /// The slow tenth of nine openings is the slowest of nine wearing a
    /// percentile's name, which reads as a measurement and is not one.
    #[test]
    fn the_slow_tenth_is_withheld_until_there_are_ten_to_take_it_from() {
        assert_eq!(spread(waits(&[1, 2, 3, 4, 5, 6, 7, 8, 9])).high, None);
        // Ten of them, and the slow tenth is the ninth: the worst is the
        // tenth, and it is printed as the worst rather than twice.
        let ten = spread(waits(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 100]));
        assert_eq!(ten.high, Some(Duration::from_millis(9)));
        assert_eq!(ten.middle, Duration::from_millis(5));
        assert_eq!(ten.worst, Duration::from_millis(100));
    }

    /// One bad opening on a quick surface should not put it at the top of a
    /// page about which surface is slow.
    #[test]
    fn the_surfaces_are_ordered_by_the_middle_and_not_by_the_worst() {
        let entries = vec![
            opening("launcher", 400, &[]),
            opening("launcher", 420, &[]),
            opening("notices-panel", 90, &[]),
            opening("notices-panel", 3000, &[]),
        ];
        let gathered = about(&entries);
        assert_eq!(gathered[0].who, "launcher");
        assert_eq!(gathered[1].who, "notices-panel");
    }

    /// Where the time went, worst first, because the first line under a slow
    /// surface should be the reason it is slow.
    #[test]
    fn the_stretches_are_ordered_by_which_of_them_is_the_slow_one() {
        let entries = vec![
            opening("launcher", 400, &[("press", 10), ("gtk", 130), ("placed", 240)]),
            opening("launcher", 380, &[("press", 12), ("gtk", 120), ("placed", 230)]),
        ];
        let gathered = about(&entries);
        let named: Vec<&str> = gathered[0].marks.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(named, ["placed", "gtk", "press"]);
    }

    /// A stretch only some of the openings had is summarised over those, so a
    /// panel that sometimes waits for the screen shows a wait for the screen.
    #[test]
    fn a_stretch_that_only_some_openings_had_is_counted_over_those() {
        let entries = vec![
            opening("launcher", 400, &[("gtk", 100)]),
            opening("launcher", 700, &[("gtk", 100), ("screen", 300)]),
        ];
        let gathered = about(&entries);
        let screen = gathered[0]
            .marks
            .iter()
            .find(|(name, _)| name == "screen")
            .expect("the wait for the screen");
        assert_eq!(screen.1.many, 1);
        assert_eq!(screen.1.middle, Duration::from_millis(300));
    }

    /// The worst opening in this store was made by a handheld compiling its own
    /// desktop, and read as what the menu is like. It says so now.
    #[test]
    fn a_worst_that_happened_while_the_machine_was_busy_says_so() {
        let mut busy = opening("launcher", 1007, &[]);
        busy.load = 5.2;
        let quiet = opening("launcher", 148, &[]);
        let said = told(&about(&[quiet, busy])[0]);
        assert!(said.contains("worst 1007ms, on a machine at 5.2"), "{said}");
    }

    /// And an ordinary one says nothing, because a line about a machine that
    /// was doing nothing else is a line about nothing.
    #[test]
    fn a_worst_on_a_quiet_machine_is_told_without_a_word_about_the_machine() {
        let said = told(&about(&[opening("launcher", 148, &[])])[0]);
        assert!(!said.contains("on a machine"), "{said}");
    }

    /// Two surfaces are two kinds of waiting even when one program draws both.
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
        assert_eq!(about(&entries).len(), 2);
    }
}
