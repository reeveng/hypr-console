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

/// How often the strip is moved on while cargo is saying nothing.
///
/// Cargo names a crate when it *starts* it, and a build with sixteen cores
/// starts a dozen at once and then says nothing for a minute while they
/// finish. Measured on the device: the strip went from twelve to thirty in two
/// seconds and then stood still for seventy-seven. Standing still is the thing
/// this was written to end, so time moves it too.
pub const TICK: std::time::Duration = std::time::Duration::from_secs(2);

/// What one of those ticks is worth against one crate.
///
/// A sixth, so twelve seconds of silence carries the strip as far as one crate
/// does. Less than a crate because a crate is news and a tick is only the
/// absence of it, and more than nothing because a bar that has not moved in a
/// minute is a bar nobody believes.
pub const A_TICK: f64 = 1.0 / 6.0;

/// How many crates it takes to carry the strip half way through the stretch.
///
/// Chosen against this workspace: a build that touches one crate names a
/// handful, a build after a dependency changes names dozens. Half way at
/// twenty means the first few crates move it visibly -- which is what somebody
/// looking at the screen is asking about -- without the last twenty crawling.
pub const PACE: f64 = 20.0;

/// What cargo says when it starts on a crate.
///
/// Indented by cargo and coloured only when it is talking to a terminal. The
/// apply reads it off a pipe, so this is the plain word; matched on the front
/// after trimming rather than on the whole line, because what follows is a
/// crate name, a version and sometimes a path.
pub fn names_a_crate(line: &str) -> Names {
    match line.trim_start().starts_with("Compiling ") {
        true => Names::ACrate,
        false => Names::SomethingElse,
    }
}

/// Whether a line off cargo's output names a crate it has started on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    /// It does, which is one more step through the build.
    ACrate,
    /// It is anything else cargo says, which is most of what it says.
    SomethingElse,
}

/// How far through the build to draw it, from what has happened so far.
///
/// Counted in steps rather than in crates, because two things move it: a crate
/// cargo has named, worth one, and a tick of silence, worth `A_TICK`.
///
/// Approaches the end of the stretch and never arrives: what ends the stretch
/// is cargo ending, not this. Nothing yet is nothing, which is where the
/// stretch starts anyway.
pub fn far(steps: f64) -> f64 {
    let steps = steps.max(0.0);
    steps / (steps + PACE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines cargo actually writes while it builds this workspace.
    #[test]
    fn the_line_that_says_a_crate_has_been_started_is_the_one_that_is_counted() {
        assert_eq!(
            names_a_crate("   Compiling console-panel v0.1.0 (/etc/console/crates/console-panel)"),
            Names::ACrate
        );
        assert_eq!(names_a_crate("Compiling serde v1.0.0"), Names::ACrate);
        assert_eq!(
            names_a_crate("    Finished `release` profile [optimized] target(s) in 4m 12s"),
            Names::SomethingElse
        );
        assert_eq!(
            names_a_crate("   Compiling"),
            Names::SomethingElse,
            "the word alone names no crate"
        );
        assert_eq!(names_a_crate("warning: unused import: `std::fmt`"), Names::SomethingElse);
        assert_eq!(names_a_crate(""), Names::SomethingElse);
    }

    /// It only ever goes forwards. A strip that went back would be read as an
    /// apply that had started again.
    #[test]
    fn every_crate_moves_it_forward() {
        let mut before = far(0.0);
        assert_eq!(before, 0.0);
        for seen in 1..200 {
            let now = far(f64::from(seen));
            assert!(now > before, "crate {seen} did not move it: {before} to {now}");
            before = now;
        }
    }

    /// And so does a minute in which cargo said nothing at all, which is most
    /// of the end of a build.
    #[test]
    fn silence_moves_it_too() {
        // Four crates, and then a minute in which cargo said nothing.
        let quiet = far(4.0 + A_TICK * 30.0);
        assert!(quiet > far(4.0), "a minute of silence left the strip where it was");
        assert!(
            quiet < far(4.0 + 30.0),
            "a minute of silence was counted as a minute of crates"
        );
    }

    /// And never arrives, because what ends the stretch is cargo ending. A
    /// strip sitting at the end of the building stretch while the build is
    /// still running is the fault this was written for, one stretch along.
    #[test]
    fn it_never_reaches_the_end_of_the_stretch_on_its_own() {
        assert!(far(1_000_000.0) < 1.0);
        assert!(far(60.0) < 0.8, "it spends its last quarter too early");
    }

    /// The first few are what somebody standing over the device sees, and they
    /// are the ones that have to move.
    #[test]
    fn the_first_crates_move_it_visibly() {
        assert!(far(5.0) > 0.15, "five crates in and the strip has hardly moved");
        assert!((far(20.0) - 0.5).abs() < 0.01);
    }
}
