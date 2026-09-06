//! How far into the build cargo has got, from what it says while it does it.
//!
//! Building is sixty of an apply's hundred and it is one stretch, so the strip
//! under the bar stood at ten per cent for the whole of the longest thing an
//! apply does and then jumped to seventy. A bar that does not move for two
//! minutes is a bar that says nothing at all: somebody standing over the device
//! cannot tell an apply that is compiling from an apply that has hung, which is
//! the one question the strip exists to answer.
//!
//! Cargo says what it is doing as it does it -- a line per crate it starts --
//! so the apply reads those as they go past and moves the strip on each one.
//!
//! # Why it does not count towards a total
//!
//! Because there is no honest total to count towards. How many crates a build
//! compiles depends on what is already built, which depends on what changed,
//! and asking cargo in advance means running the whole resolver twice. What is
//! remembered from the last apply is no better: the apply that matters is the
//! one after somebody edited one file, and the one after somebody bumped a
//! dependency, and those two builds are twenty crates apart.
//!
//! So it moves and it does not pretend. Each crate carries the strip a share of
//! what is left of the stretch, so it always moves forward, moves most at the
//! start where a person is deciding whether anything is happening, and never
//! reaches the end of the stretch before the stretch is over. The end is not a
//! guess: it arrives when the build does.

use console_never::Never;

pub const TICK: std::time::Duration = std::time::Duration::from_secs(2);

pub const A_TICK: f64 = 1.0 / 6.0;

pub const PACE: f64 = 20.0;

pub fn names_a_crate(line: &str) -> Result<Names, Never> {
    Ok(match line.trim_start().starts_with("Compiling ") {
        true => Names::ACrate,
        false => Names::SomethingElse,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    ACrate,
    SomethingElse,
}

pub fn far(steps: f64) -> Result<f64, Never> {
    let steps = steps.max(0.0);

    Ok(steps / (steps + PACE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(line: &str) -> Names {
        let Ok(names) = names_a_crate(line);

        names
    }

    fn along(steps: f64) -> f64 {
        let Ok(far) = far(steps);

        far
    }

    #[test]
    fn the_line_that_says_a_crate_has_been_started_is_the_one_that_is_counted() {
        assert_eq!(
            names("   Compiling console-panel v0.1.0 (/etc/console/crates/console-panel)"),
            Names::ACrate
        );
        assert_eq!(names("Compiling serde v1.0.0"), Names::ACrate);
        assert_eq!(
            names("    Finished `release` profile [optimized] target(s) in 4m 12s"),
            Names::SomethingElse
        );
        assert_eq!(names("   Compiling"), Names::SomethingElse, "the word alone names no crate");
        assert_eq!(names("warning: unused import: `std::fmt`"), Names::SomethingElse);
        assert_eq!(names(""), Names::SomethingElse);
    }

    #[test]
    fn every_crate_moves_it_forward() {
        let mut before = along(0.0);
        assert_eq!(before, 0.0);
        for seen in 1..200 {
            let now = along(f64::from(seen));
            assert!(now > before, "crate {seen} did not move it: {before} to {now}");
            before = now;
        }
    }

    #[test]
    fn silence_moves_it_too() {
        let quiet = along(4.0 + A_TICK * 30.0);
        assert!(quiet > along(4.0), "a minute of silence left the strip where it was");
        assert!(
            quiet < along(4.0 + 30.0),
            "a minute of silence was counted as a minute of crates"
        );
    }

    #[test]
    fn it_never_reaches_the_end_of_the_stretch_on_its_own() {
        assert!(along(1_000_000.0) < 1.0);
        assert!(along(60.0) < 0.8, "it spends its last quarter too early");
    }

    #[test]
    fn the_first_crates_move_it_visibly() {
        assert!(along(5.0) > 0.15, "five crates in and the strip has hardly moved");
        assert!((along(20.0) - 0.5).abs() < 0.01);
    }
}
