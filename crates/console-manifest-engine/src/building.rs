//! How far into the build cargo has got, from what it says while it does it.
//!
//! Building is sixty of an apply's hundred and it is one stage, so the strip
//! under the bar stood at ten per cent for the whole of the longest thing an
//! apply does and then jumped to seventy. A bar that does not move for two
//! minutes is a bar that says nothing at all: someone standing over the device
//! cannot tell an apply that is compiling from an apply that has hung, which is
//! the one question the strip exists to answer.
//!
//! Cargo says what it is doing as it does it -- a line per crate it starts --
//! so the apply reads those as they go past and moves the strip on each one.
//! The line is read through `how_far::plain`, because cargo is asked for color
//! when a person is watching and the word it is being read for arrives wrapped
//! in escapes on exactly the runs where someone is looking at the bar.
//!
//! # Why it does not count toward a total
//!
//! Because there is no honest total to count toward. How many crates a build
//! compiles depends on what is already built, which depends on what changed,
//! and asking cargo in advance means running the whole resolver twice. What is
//! remembered from the last apply is no better: the apply that matters is the
//! one after someone edited one file, and the one after someone bumped a
//! dependency, and those two builds are twenty crates apart.
//!
//! So it moves and it does not pretend. Each crate carries the strip a share of
//! what is left of the stage, so it always moves forward, moves most at the
//! start where a person is deciding whether anything is happening, and never
//! reaches the end of the stage before the stage is over. The end is not a
//! guess: it arrives when the build does.

use console_core_never::Never;

use console_how_far as how_far;

pub const TICK: std::time::Duration = std::time::Duration::from_secs(2);

pub const A_TICK: f64 = 1.0 / 6.0;

pub const PACE: f64 = 20.0;

pub fn names_a_crate(line: &str) -> Result<Names, Never> {
    let Ok(plain) = how_far::plain(line);

    let rest = match plain.trim_start().strip_prefix("Compiling ") {
        Some(rest) => rest,
        None => return Ok(Names::SomethingElse),
    };

    let name = match rest.split_whitespace().next() {
        Some(name) => name,
        None => return Ok(Names::SomethingElse),
    };

    Ok(Names::ACrate(name.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Names {
    ACrate(String),
    SomethingElse,
}

pub fn far(steps: f64) -> Result<f64, Never> {
    let steps = steps.max(0.0);

    Ok(steps / (steps + PACE))
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn the_line_that_says_a_crate_has_been_started_is_the_one_that_is_counted() {
        assert_eq!(
            names_a_crate("   Compiling console-panel v0.1.0 (/etc/console/crates/console-panel)"),
            Ok(Names::ACrate("console-panel".to_string()))
        );
        assert_eq!(names_a_crate("Compiling serde v1.0.0"),
            Ok(Names::ACrate("serde".to_string())));
        assert_eq!(
            names_a_crate("    Finished `release` profile [optimized] target(s) in 4m 12s"),
            Ok(Names::SomethingElse)
        );
        assert_eq!(names_a_crate("   Compiling"),
            Ok(Names::SomethingElse), "the word alone names no crate");
        assert_eq!(names_a_crate("warning: unused import: `std::fmt`"),
            Ok(Names::SomethingElse));
        assert_eq!(names_a_crate(""),
            Ok(Names::SomethingElse));
    }

    #[test]
    fn a_crate_started_in_color_is_the_same_crate() {
        assert_eq!(
            names_a_crate("\u{1b}[0m\u{1b}[1m\u{1b}[32m   Compiling\u{1b}[0m console-panel v0.1.0"),
            Ok(Names::ACrate("console-panel".to_string())),
            "cargo colors its own output and the line stopped being read"
        );
    }

    #[test]
    fn every_crate_moves_it_forward() {
        let Ok(mut before) = far(0.0);

        assert_eq!(before, 0.0);

        for seen in 1..200 {
            let Ok(now) = far(f64::from(seen));

            assert!(now > before, "crate {seen} did not move it: {before} to {now}");
            before = now;
        }
    }

    #[test]
    fn silence_moves_it_too() {
        let Ok(quiet) = far(4.0 + A_TICK * 30.0);
        let Ok(four_crates) = far(4.0);
        let Ok(thirty_more) = far(4.0 + 30.0);

        assert!(quiet > four_crates, "a minute of silence left the strip where it was");
        assert!(
            quiet < thirty_more,
            "a minute of silence was counted as a minute of crates"
        );
    }

    #[test]
    fn it_never_reaches_the_end_of_the_stage_on_its_own() {
        let Ok(a_million) = far(1_000_000.0);
        let Ok(sixty) = far(60.0);

        assert!(a_million < 1.0);
        assert!(sixty < 0.8, "it spends its last quarter too early");
    }

    #[test]
    fn the_first_crates_move_it_visibly() {
        let Ok(five) = far(5.0);
        let Ok(twenty) = far(20.0);

        assert!(five > 0.15, "five crates in and the strip has hardly moved");
        assert!((twenty - 0.5).abs() < 0.01);
    }
}
