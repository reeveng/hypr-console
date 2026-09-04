//! How warm the screen is, and when.
//!
//! `hyprsunset` does the work. It is a daemon that hands the compositor a
//! colour transform, which is why it is preferred to a shader: what it changes
//! is not in a screenshot or a recording, so the screenshot the top right
//! paddle takes at eleven at night looks like the one taken at noon.
//!
//! ## The clock decides, not a thumb
//!
//! It used to be a switch and one temperature: press it and the screen went
//! warm, press it again and it went back. That is a decision somebody has to
//! remember to make twice a day, and the evening it is wanted is the evening
//! nobody thinks of it.
//!
//! So the screen follows the clock. It cools nothing all day, slides from
//! daylight down to lamplight across the two hours of dusk, holds there through
//! the night, and climbs back over the half hour before morning. The slide is
//! what makes it invisible: a screen that changed colour in one step at half
//! past seven would be a thing that happened to you, and this is a thing you
//! never catch happening.
//!
//! `hyprsunset` does the following itself, out of `hyprsunset.conf`: a list of
//! profiles, each a time and what to wear from then on, and it holds each one
//! until the next. So the whole of the curve is a file, and nothing here has to
//! be running to keep the screen honest at three in the morning.
//!
//! The steps are spaced evenly **in mireds**, not in kelvin. Kelvin is not
//! perceptually even -- the same thousand degrees is an enormous change down at
//! the warm end and barely visible up at the cold one -- so a curve stepped
//! evenly in kelvin crawls all evening and then lurches at the end. A mired is
//! a million over the kelvin, and even steps in it are even steps to an eye.
//!
//! ## And the switch is now the daemon
//!
//! There is still a way to say no, and it had to change shape. `hyprsunset`
//! re-applies its profile at every step, so telling it `identity` is undone by
//! the clock -- three minutes later during dusk, and not until morning at
//! midnight, which is a switch that behaves differently depending on when it is
//! pressed. There is no way to ask the daemon to stop following its own
//! profiles.
//!
//! So off means the daemon is not running. `console-warm` writes the answer
//! down and restarts the unit; the unit asks this before it starts anything, in
//! `ExecCondition=`, and a compositor with no colour transform on it is a screen
//! showing its own colours -- which is the one state that is true whatever the
//! hour and survives a reboot without anybody re-asserting it.
//!
//! What is here is the curve, the answer to "is it wanted", and the writing of
//! the config out of the first. `console-warm curve` prints it, which is how
//! the copy in `files/` is made; a test holds the two together so the file on
//! the machine cannot drift from the curve this says.


use console_number::whole_u32;
use std::fmt::Write;
use std::path::PathBuf;

/// Daylight, in kelvin: the top of the curve and what the screen is all day.
///
/// 6500 is the daylight every panel is measured against, and it is what the
/// curve leaves rather than a colour it ever sits at for long: the day itself
/// is `identity`, the daemon wearing nothing at all.
pub const DAYLIGHT: u32 = 6500;

/// Warm, in kelvin: the bottom of the curve and what the night is.
///
/// The colour of a lamp rather than of daylight: warm enough that a screen read
/// in the dark stops looking like a window, and not so far that the wallpapers
/// go orange.
pub const WARM: u32 = 3000;

/// Minutes between one profile and the next.
///
/// Close enough together that the slide reads as continuous rather than as a
/// series of jumps, and far enough apart that the file is a page and not a
/// book. What decides it is the eye: at this spacing no single step is a
/// change anybody notices, which is the whole point of the curve.
pub const STEP: u32 = 3;

/// When dusk begins and ends, in minutes past midnight.
///
/// Two hours, which is long enough that no part of it is an event. The same
/// hours the laptop this desktop is written on uses, so that moving between the
/// two machines in an evening is not moving between two different times of day.
pub const DUSK: (u32, u32) = (19 * 60 + 30, 21 * 60 + 30);

/// When the screen is daylight again, in minutes past midnight.
pub const DAY: u32 = 7 * 60;

/// How long the climb back to daylight takes, in minutes.
///
/// Shorter than the fall. Dusk is slow because it is happening while somebody
/// is looking at the screen; this mostly happens while nobody is.
pub const DAWN: u32 = 30;

/// Where the answer is kept, under the home of whoever this desktop belongs to.
///
/// Not in the manifest, for the reason the button table is not: it is true of
/// one machine on one evening and wrong for every other, and a manifest file
/// somebody is invited to change is a file `console check` reports as drift for
/// ever after.
pub const UNDER: &str = ".config/console/warm";

/// What a profile in the config says to wear.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Says {
    /// A temperature, in kelvin.
    Warmth(u32),
    /// Nothing at all: the screen's own colours.
    ///
    /// `identity` rather than 6500, which is a temperature that looks like
    /// daylight and is still a transform sitting on the monitor. The panel is
    /// left alone instead.
    Daylight,
}

/// One profile: a time, and what the screen wears from then until the next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Step {
    /// Minutes past midnight.
    pub at: u32,
    pub says: Says,
}

/// The whole curve, in the order the clock meets it.
///
/// Dusk first because that is where the day's colour starts to change, then
/// the climb back and the daylight it lands on. `hyprsunset` holds the last
/// profile it passed, so the bottom of dusk carries through the small hours
/// without anything being written for them: the night is not in this list
/// because the night is what happens when nothing else is said.
pub fn curve() -> Vec<Step> {
    let mut steps = Vec::new();

    let falling = (DUSK.1 - DUSK.0) / STEP;

    for part in 0..=falling {
        steps.push(Step {
            at: DUSK.0 + part * STEP,
            says: Says::Warmth(between(DAYLIGHT, WARM, part, falling)),
        });
    }

    // The last of the climb would be daylight itself, and daylight is written
    // as wearing nothing rather than as the number that looks like it. So the
    // climb stops one short and the day is the profile after it.
    let climbing = DAWN / STEP;

    for part in 1..climbing {
        steps.push(Step {
            at: DAY - DAWN + part * STEP,
            says: Says::Warmth(between(WARM, DAYLIGHT, part, climbing)),
        });
    }

    steps.push(Step { at: DAY, says: Says::Daylight });

    steps
}

/// The config `hyprsunset` reads, as the whole file.
///
/// Written from `curve` rather than kept beside it, because a curve written
/// twice is a curve that goes out of step, and out of step here is a screen
/// that changes colour at a time nothing in this repository mentions.
pub fn config() -> String {
    let mut said = String::from(HEAD);

    for step in curve() {
        let _ = write!(said, "\nprofile {{\n    time = {}\n", clock(step.at));
        let _ = match step.says {
            Says::Warmth(kelvin) => write!(said, "    temperature = {kelvin}\n}}\n"),
            Says::Daylight => write!(said, "    identity = true\n}}\n"),
        };
    }

    said
}

/// What stands above the profiles in the file it is written into.
const HEAD: &str = "\
# The colour of the screen, on a clock. Written by `console-warm curve` out of
# `console_settings::warm`, which is where the hours and the two temperatures
# are decided; a test holds this file to what that says, so editing it here is
# an edit that comes back.
#
# hyprsunset holds each profile until the next one, so the last of the evening
# carries through the small hours and there is nothing written for the night.
# The steps are spaced evenly in mireds rather than in kelvin, because kelvin is
# not perceptually even.
#
# The unit that starts hyprsunset asks `console-warm wanted` first, so the
# switch on the Screen tab is this daemon running or not running rather than a
# temperature sent to it: a temperature would be undone by the next profile
# below.
";

/// A time of day, as the config writes it.
fn clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// One step of the way from one temperature to another, evenly in mireds.
///
/// Rounded to ten kelvin, which is far below anything an eye can tell apart and
/// keeps the file readable.
fn between(from: u32, to: u32, part: u32, whole: u32) -> u32 {
    let (from, to) = (mired(from), mired(to));
    let at = from + (to - from) * f64::from(part) / f64::from(whole);
    let kelvin = 1_000_000.0 / at;
    whole_u32(kelvin / 10.0) * 10
}

/// A temperature in the units an eye steps evenly through.
fn mired(kelvin: u32) -> f64 {
    1_000_000.0 / f64::from(kelvin)
}

/// Which way the switch is standing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Warmth {
    /// The screen follows the clock.
    Following,
    /// The screen is its own colours, whatever the hour.
    Ordinary,
}

/// Whether the daemon that wears the curve should be running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    /// It should: the screen follows the sun.
    Running,
    /// It should not, and the curve is whatever it was set to by hand.
    Off,
}

impl Warmth {
    /// What was written down, or the clock where nothing was.
    ///
    /// A machine nobody has asked follows the clock, because that is what this
    /// desktop does and a person should not have to find the setting to get it.
    /// Anything unreadable is the same: the failure of this file should be the
    /// desktop behaving as it is meant to, never a screen somebody cannot
    /// explain.
    pub fn read(held: &str) -> Self {
        match held.trim() {
            "ordinary" => Warmth::Ordinary,
            _ => Warmth::Following,
        }
    }

    /// The other one.
    pub fn other(self) -> Self {
        match self {
            Warmth::Following => Warmth::Ordinary,
            Warmth::Ordinary => Warmth::Following,
        }
    }

    /// What goes in the file.
    pub fn written(self) -> &'static str {
        match self {
            Warmth::Following => "clock\n",
            Warmth::Ordinary => "ordinary\n",
        }
    }

    /// Whether the daemon that wears the curve should be running at all.
    pub fn wanted(self) -> Wanted {
        match self == Warmth::Following {
            true => Wanted::Running,
            false => Wanted::Off,
        }
    }
}

/// Where the answer is on this machine.
pub fn at(home: &str) -> PathBuf {
    PathBuf::from(home).join(UNDER)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one the user asked for, and the one that costs nothing to get
    /// wrong: a device out of the box follows the clock.
    #[test]
    fn a_device_that_was_never_asked_follows_the_clock() {
        assert_eq!(Warmth::read(""), Warmth::Following);
        assert_eq!(Warmth::read("what?\n"), Warmth::Following);
        assert_eq!(Warmth::read("").wanted(), Wanted::Running);
    }

    /// Saying no is the only thing that has to be written down, so it is the
    /// only word this file can hold that means anything.
    #[test]
    fn only_the_refusal_is_remembered() {
        assert_eq!(Warmth::read("ordinary\n"), Warmth::Ordinary);
        assert_eq!(Warmth::Ordinary.wanted(), Wanted::Off);
    }

    #[test]
    fn what_was_written_is_what_is_read_back() {
        for way in [Warmth::Following, Warmth::Ordinary] {
            assert_eq!(Warmth::read(way.written()), way);
        }
    }

    #[test]
    fn the_switch_has_two_sides_and_they_are_each_other() {
        assert_eq!(Warmth::Following.other(), Warmth::Ordinary);
        assert_eq!(Warmth::Ordinary.other(), Warmth::Following);
    }

    /// The whole shape of it, said as a sentence: it leaves daylight when dusk
    /// begins, it is at its warmest when dusk ends, it stays there because
    /// nothing is written for the night, and the day is the daemon wearing
    /// nothing.
    #[test]
    fn the_curve_leaves_daylight_at_dusk_and_comes_back_at_seven() {
        let steps = curve();
        let first = steps.first().expect("a curve");
        assert_eq!(first.at, DUSK.0);
        assert_eq!(first.says, Says::Warmth(DAYLIGHT));

        let bottom = steps.iter().find(|step| step.at == DUSK.1).expect("the end of dusk");
        assert_eq!(bottom.says, Says::Warmth(WARM));

        assert!(
            !steps.iter().any(|step| step.at > DUSK.1 && step.at < DAY - DAWN),
            "the night is what happens when nothing is said, so nothing is said for it"
        );

        let last = steps.last().expect("a curve");
        assert_eq!(last.at, DAY);
        assert_eq!(last.says, Says::Daylight);
    }

    /// The times only ever go forwards, which is what makes the file readable
    /// and is the thing that was wrong with the one this was written from: its
    /// dawn had the right temperatures against the wrong times, so the morning
    /// got warmer instead of colder.
    #[test]
    fn dusk_falls_and_dawn_climbs() {
        let steps = curve();
        // The list starts at dusk and ends in the morning, so it goes forwards
        // throughout and turns over the midnight in the middle of it exactly
        // once. Two turns would mean a profile written out of its place.
        let midnights = steps.windows(2).filter(|pair| pair[0].at >= pair[1].at).count();
        assert_eq!(midnights, 1, "the curve crosses midnight {midnights} times");

        let dusk: Vec<u32> = warmths(&steps, DUSK.0, DUSK.1);
        assert!(dusk.windows(2).all(|two| two[0] > two[1]), "dusk does not fall: {dusk:?}");

        let dawn: Vec<u32> = warmths(&steps, DAY - DAWN, DAY);
        assert!(dawn.windows(2).all(|two| two[0] < two[1]), "dawn does not climb: {dawn:?}");
        assert!(
            dawn.first().is_some_and(|first| *first > WARM),
            "the climb starts above the night it is leaving"
        );
        assert!(
            dawn.last().is_some_and(|last| *last < DAYLIGHT),
            "the climb stops one short, because daylight is written as identity"
        );
    }

    /// No single step is a change anybody could catch. Said as the largest gap
    /// in mireds between one profile and the next, because that is the unit the
    /// eye steps in and the whole reason the curve is spaced this way.
    #[test]
    fn no_step_is_big_enough_to_notice() {
        let steps = curve();
        let biggest = steps
            .windows(2)
            .filter_map(|two| match (two[0].says, two[1].says) {
                (Says::Warmth(before), Says::Warmth(after)) => {
                    Some((mired(after) - mired(before)).abs())
                }
                _ => None,
            })
            .fold(0.0_f64, f64::max);
        assert!(biggest < 20.0, "one step moves {biggest} mireds, which is a jump");
    }

    /// Every profile is one hyprsunset will parse, and the day is the one that
    /// wears nothing.
    #[test]
    fn the_config_is_written_the_way_the_daemon_reads_it() {
        let said = config();
        assert!(said.starts_with('#'), "the file says what wrote it");
        assert!(said.contains("profile {\n    time = 19:30\n    temperature = 6500\n}\n"));
        assert!(said.contains("profile {\n    time = 21:30\n    temperature = 3000\n}\n"));
        assert!(said.contains("profile {\n    time = 07:00\n    identity = true\n}\n"));
        assert_eq!(said.matches("profile {").count(), curve().len());
    }

    /// Warm has to be far enough from daylight to be worth having, and near
    /// enough that the wallpapers are still their own colours.
    #[test]
    fn warm_is_a_lamp_rather_than_daylight_or_a_fire() {
        const { assert!(WARM < 4500, "warm is not far enough from daylight to see") };
        const { assert!(WARM > 2000, "that is orange rather than warm") };
    }

    #[test]
    fn the_answer_is_kept_under_the_home_it_belongs_to() {
        assert_eq!(at("/home/somebody"), PathBuf::from("/home/somebody/.config/console/warm"));
    }

    fn warmths(steps: &[Step], from: u32, to: u32) -> Vec<u32> {
        steps
            .iter()
            .filter(|step| step.at >= from && step.at <= to)
            .filter_map(|step| match step.says {
                Says::Warmth(kelvin) => Some(kelvin),
                Says::Daylight => None,
            })
            .collect()
    }
}
