//! How warm the screen is, and what warm means on this one.
//!
//! `hyprsunset` does the work. It is a daemon that hands the compositor a
//! colour transform, which is why it is preferred to a shader: what it changes
//! is not in a screenshot or a recording, so the screenshot the top right
//! paddle takes at eleven at night looks like the one taken at noon.
//!
//! What is here is the one decision -- which temperature counts as warm -- and
//! the answer to "is it on", which nothing else can give. The daemon takes a
//! temperature and does not report one back, so a panel that wants to draw a
//! switch has to have been told, and this is where it was written down.

use std::path::PathBuf;

/// Warm, in kelvin.
///
/// 6000 is the daemon's own idea of neutral and is what it wears at rest, so
/// the number here has to be far enough from it to be worth pressing a switch
/// for. 3400 is the colour of a lamp rather than of daylight: warm enough that
/// a screen read in the dark stops looking like a window, and not so far that
/// the wallpapers go orange.
pub const WARM: u32 = 3400;

/// Where the answer is kept, under the home of whoever this desktop belongs to.
///
/// Not in the manifest, for the reason the button table is not: it is true of
/// one machine on one evening and wrong for every other, and a manifest file
/// somebody is invited to change is a file `console check` reports as drift for
/// ever after.
pub const UNDER: &str = ".config/console/warm";

/// Which way the switch is standing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Warmth {
    Warm,
    Ordinary,
}

impl Warmth {
    /// What was written down, or ordinary where nothing was.
    ///
    /// Anything unreadable is ordinary as well. The failure of this file should
    /// be a screen the colour it has always been, never a screen somebody
    /// cannot explain.
    pub fn read(held: &str) -> Self {
        match held.trim() {
            "warm" => Warmth::Warm,
            _ => Warmth::Ordinary,
        }
    }

    /// The other one.
    pub fn other(self) -> Self {
        match self {
            Warmth::Warm => Warmth::Ordinary,
            Warmth::Ordinary => Warmth::Warm,
        }
    }

    /// What goes in the file.
    pub fn written(self) -> &'static str {
        match self {
            Warmth::Warm => "warm\n",
            Warmth::Ordinary => "ordinary\n",
        }
    }

    /// What the daemon is told, as the words `hyprctl` takes after it.
    ///
    /// `identity` rather than 6000: the daemon's word for changing nothing at
    /// all, which is a state it can be in rather than a temperature that
    /// happens to look like one.
    pub fn told(self) -> Vec<String> {
        let said = match self {
            Warmth::Warm => vec!["temperature".to_string(), WARM.to_string()],
            Warmth::Ordinary => vec!["identity".to_string()],
        };
        [vec!["hyprctl".to_string(), "hyprsunset".to_string()], said].concat()
    }

    pub fn is_warm(self) -> bool {
        self == Warmth::Warm
    }
}

/// Where the answer is on this machine.
pub fn at(home: &str) -> PathBuf {
    PathBuf::from(home).join(UNDER)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine nobody has asked is a machine the colour it came out of the
    /// box, which is the only state that needs no explaining.
    #[test]
    fn a_device_that_was_never_asked_is_ordinary() {
        assert_eq!(Warmth::read(""), Warmth::Ordinary);
        assert_eq!(Warmth::read("what?\n"), Warmth::Ordinary);
    }

    #[test]
    fn what_was_written_is_what_is_read_back() {
        for way in [Warmth::Warm, Warmth::Ordinary] {
            assert_eq!(Warmth::read(way.written()), way);
        }
    }

    #[test]
    fn the_switch_has_two_sides_and_they_are_each_other() {
        assert_eq!(Warmth::Warm.other(), Warmth::Ordinary);
        assert_eq!(Warmth::Ordinary.other(), Warmth::Warm);
    }

    /// The daemon is told in its own words, and ordinary is a word rather than
    /// a number that means nothing changed.
    #[test]
    fn the_daemon_is_told_a_temperature_or_told_to_stop() {
        assert_eq!(Warmth::Warm.told(), ["hyprctl", "hyprsunset", "temperature", "3400"]);
        assert_eq!(Warmth::Ordinary.told(), ["hyprctl", "hyprsunset", "identity"]);
    }

    /// Warm has to be worth pressing a switch for. The daemon sits at 6000 when
    /// it is doing nothing, so a warm that was near it would be a switch with
    /// no visible sides.
    #[test]
    fn warm_is_far_enough_from_the_daylight_it_replaces() {
        const { assert!(WARM < 4500, "warm is not far enough from daylight to see") };
        const { assert!(WARM > 2000, "that is orange rather than warm") };
    }

    #[test]
    fn the_answer_is_kept_under_the_home_it_belongs_to() {
        assert_eq!(
            at("/home/somebody"),
            PathBuf::from("/home/somebody/.config/console/warm")
        );
    }
}
