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
//!
//! `standing` is the reading of that answer, and it is here rather than at each
//! caller because there are two of them and they were not reading it the same
//! way. A file nobody has written yet means following the clock, which is what
//! a machine that was never asked should do. A file that is there and will not
//! be read means nothing of the sort, and a reader that turns it into
//! `Following` reports a setting the person may have turned off. So the two are
//! separate answers, and it is the caller that decides how loudly to say the
//! second.


use console_core_atomic_writes::Held;
use console_core_never::Never;
use console_core_number_conversion::whole_u32;
use std::fmt::Write;
use std::path::{Path, PathBuf};

pub const DAYLIGHT: u32 = 6500;

pub const WARM: u32 = 3000;

pub const STEP: u32 = 3;

pub const DUSK: (u32, u32) = (19 * 60 + 30, 21 * 60 + 30);

pub const DAY: u32 = 7 * 60;

pub const DAWN: u32 = 30;

pub const NAMED: &str = "warm";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Says {
    Warmth(u32),
    Daylight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Step {
    pub at: u32,
    pub says: Says,
}

pub fn curve() -> Result<Vec<Step>, Never> {
    let mut steps = Vec::new();

    let falling = DUSK.1.saturating_sub(DUSK.0).saturating_div(STEP);

    for part in 0..=falling {
        let Ok(warmth) = between(DAYLIGHT, WARM, part, falling);

        steps.push(Step {
            at: DUSK.0.saturating_add(part.saturating_mul(STEP)),
            says: Says::Warmth(warmth),
        });
    }

    let climbing = DAWN.saturating_div(STEP);

    for part in 1..climbing {
        let Ok(warmth) = between(WARM, DAYLIGHT, part, climbing);

        steps.push(Step {
            at: DAY.saturating_sub(DAWN).saturating_add(part.saturating_mul(STEP)),
            says: Says::Warmth(warmth),
        });
    }

    steps.push(Step { at: DAY, says: Says::Daylight });

    Ok(steps)
}

pub fn config() -> Result<String, Never> {
    let mut said = String::from(HEAD);
    let Ok(curve) = curve();

    for step in curve {
        let Ok(clock) = clock(step.at);
        let _ = write!(said, "\nprofile {{\n    time = {clock}\n");
        let _ = match step.says {
            Says::Warmth(kelvin) => write!(said, "    temperature = {kelvin}\n}}\n"),
            Says::Daylight => write!(said, "    identity = true\n}}\n"),
        };
    }

    Ok(said)
}

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

fn clock(minutes: u32) -> Result<String, Never> {
    Ok(format!("{:02}:{:02}", minutes.saturating_div(60), minutes.wrapping_rem(60)))
}

fn between(from: u32, to: u32, part: u32, whole: u32) -> Result<u32, Never> {
    let Ok(from) = mired(from);
    let Ok(to) = mired(to);
    let at = from + (to - from) * f64::from(part) / f64::from(whole);
    let kelvin = 1_000_000.0 / at;
    let Ok(tens) = whole_u32(kelvin / 10.0);

    Ok(tens.saturating_mul(10))
}

fn mired(kelvin: u32) -> Result<f64, Never> {
    Ok(1_000_000.0 / f64::from(kelvin))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Warmth {
    Following,
    Ordinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    Running,
    Off,
}

impl Warmth {
    pub fn read(held: &str) -> Result<Self, Never> {
        Ok(match held.trim() {
            "ordinary" => Warmth::Ordinary,
            _ => Warmth::Following,
        })
    }

    pub fn other(self) -> Result<Self, Never> {
        Ok(match self {
            Warmth::Following => Warmth::Ordinary,
            Warmth::Ordinary => Warmth::Following,
        })
    }

    pub fn written(self) -> Result<&'static str, Never> {
        Ok(match self {
            Warmth::Following => "clock\n",
            Warmth::Ordinary => "ordinary\n",
        })
    }

    pub fn wanted(self) -> Result<Wanted, Never> {
        Ok(match self == Warmth::Following {
            true => Wanted::Running,
            false => Wanted::Off,
        })
    }
}

pub fn at(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = console_core_places::Base::Config.ours_under(home);

    Ok(ours.join(NAMED))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    Saying(Warmth),
    Unreadable(String),
}

pub fn standing(home: &Path) -> Result<Standing, Never> {
    let Ok(at) = at(home);
    let Ok(held) = console_core_atomic_writes::read(&at);

    Ok(match held {
        Held::Said(said) => {
            let Ok(warmth) = Warmth::read(&said);

            Standing::Saying(warmth)
        }
        Held::Nothing => Standing::Saying(Warmth::Following),
        Held::Unreadable(fault) => Standing::Unreadable(fault),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(held: &str) -> Warmth {
        let Ok(warmth) = Warmth::read(held);

        warmth
    }

    fn other(warmth: Warmth) -> Warmth {
        let Ok(other) = warmth.other();

        other
    }

    fn written(warmth: Warmth) -> &'static str {
        let Ok(written) = warmth.written();

        written
    }

    fn wanted(warmth: Warmth) -> Wanted {
        let Ok(wanted) = warmth.wanted();

        wanted
    }

    fn curve() -> Vec<Step> {
        let Ok(steps) = super::curve();

        steps
    }

    fn config() -> String {
        let Ok(said) = super::config();

        said
    }

    #[test]
    fn a_device_that_was_never_asked_follows_the_clock() {
        assert_eq!(read(""), Warmth::Following);
        assert_eq!(read("what?\n"), Warmth::Following);
        assert_eq!(wanted(read("")), Wanted::Running);
    }

    #[test]
    fn only_the_refusal_is_remembered() {
        assert_eq!(read("ordinary\n"), Warmth::Ordinary);
        assert_eq!(wanted(Warmth::Ordinary), Wanted::Off);
    }

    #[test]
    fn what_was_written_is_what_is_read_back() {
        for way in [Warmth::Following, Warmth::Ordinary] {
            assert_eq!(read(written(way)), way);
        }
    }

    #[test]
    fn the_switch_has_two_sides_and_they_are_each_other() {
        assert_eq!(other(Warmth::Following), Warmth::Ordinary);
        assert_eq!(other(Warmth::Ordinary), Warmth::Following);
    }

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

    #[test]
    fn dusk_falls_and_dawn_climbs() {
        let steps = curve();
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

    #[test]
    fn no_step_is_big_enough_to_notice() {
        let steps = curve();
        let biggest = steps
            .windows(2)
            .filter_map(|two| match (two[0].says, two[1].says) {
                (Says::Warmth(before), Says::Warmth(after)) => {
                    let Ok(after) = mired(after);
                    let Ok(before) = mired(before);

                    Some((after - before).abs())
                }
                _ => None,
            })
            .fold(0.0_f64, f64::max);
        assert!(biggest < 20.0, "one step moves {biggest} mireds, which is a jump");
    }

    #[test]
    fn the_config_is_written_the_way_the_daemon_reads_it() {
        let said = config();
        assert!(said.starts_with('#'), "the file says what wrote it");
        assert!(said.contains("profile {\n    time = 19:30\n    temperature = 6500\n}\n"));
        assert!(said.contains("profile {\n    time = 21:30\n    temperature = 3000\n}\n"));
        assert!(said.contains("profile {\n    time = 07:00\n    identity = true\n}\n"));
        assert_eq!(said.matches("profile {").count(), curve().len());
    }

    #[test]
    fn warm_is_a_lamp_rather_than_daylight_or_a_fire() {
        const { assert!(WARM < 4500, "warm is not far enough from daylight to see") };
        const { assert!(WARM > 2000, "that is orange rather than warm") };
    }

    #[test]
    fn the_answer_is_kept_under_the_home_it_belongs_to() {
        let Ok(at) = at(Path::new("/home/somebody"));

        assert_eq!(at, PathBuf::from("/home/somebody/.config/console/warm"));
    }

    #[test]
    fn a_home_with_nothing_written_in_it_follows_the_clock() {
        let at = std::env::temp_dir().join("console-warm-never-asked");

        let _ = std::fs::remove_dir_all(&at);

        let Ok(standing) = super::standing(&at);

        assert_eq!(standing, Standing::Saying(Warmth::Following));
    }

    #[test]
    fn an_answer_that_will_not_be_read_is_not_an_answer() {
        let home = std::env::temp_dir().join("console-warm-unreadable");
        let Ok(at) = at(&home);

        let _ = std::fs::remove_dir_all(&home);

        std::fs::create_dir_all(&at).expect("somewhere to work");

        let Ok(standing) = super::standing(&home);

        match standing {
            Standing::Unreadable(_) => {}
            Standing::Saying(warmth) => {
                panic!("a setting that would not be read came back as {warmth:?}")
            }
        }

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_refusal_that_was_written_down_is_read_back() {
        let home = std::env::temp_dir().join("console-warm-ordinary");
        let Ok(at) = at(&home);

        let _ = std::fs::remove_dir_all(&home);

        std::fs::create_dir_all(at.parent().expect("a place to put it")).expect("somewhere to work");
        std::fs::write(&at, "ordinary\n").expect("something to read back");

        let Ok(standing) = super::standing(&home);

        assert_eq!(standing, Standing::Saying(Warmth::Ordinary));

        let _ = std::fs::remove_dir_all(&home);
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
