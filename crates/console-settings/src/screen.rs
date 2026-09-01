//! How bright the screen is, and what full means on this panel.
//!
//! The backlight takes a number up to 65535 and anything near the top of that
//! range comes back out as nothing: setting it to full turns the light off. So
//! what counts as full is a decision rather than a reading, and it is made
//! once, here. A second opinion about this screen is two numbers that part
//! company the day either of them moves -- which is why the settings panel
//! asked `console-brightness get` rather than reading the file itself, and why
//! it can now ask this instead.

use std::path::PathBuf;

/// The panel this device has.
pub const DEVICE: &str = "/sys/class/backlight/amdgpu_bl1";

/// The highest value that still lights the screen.
pub const CEILING: i64 = 64000;

/// The floor is well above nothing for the same reason in reverse: a screen
/// nobody can read is not a brightness setting.
pub const FLOOR: i64 = 3200;

/// One press of the rocker, or of left and right under L2.
pub const STEP: i64 = 6000;

/// Which way a press goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Way {
    Up,
    Down,
}

impl Way {
    pub fn named(word: &str) -> Option<Self> {
        match word {
            "up" => Some(Way::Up),
            "down" => Some(Way::Down),
            _ => None,
        }
    }
}

/// Where a press lands, from where the screen is now.
///
/// Clamped at both ends, so a press at the top is no press at all rather than a
/// screen that goes dark. `090-brighter` and `091-dimmer` both rest on that:
/// they turn the screen the other way first because a machine already at the
/// top has nothing to show for a press.
pub fn stepped(now: i64, way: Way) -> i64 {
    let next = match way {
        Way::Up => now + STEP,
        Way::Down => now - STEP,
    };
    next.clamp(FLOOR, CEILING)
}

/// The same range read the other way round, in points of a hundred, so the
/// panel can draw a bar of it.
pub fn as_points(now: i64) -> i64 {
    ((now - FLOOR) * 100 / (CEILING - FLOOR)).clamp(0, 100)
}

/// What the notice says, at a level read in points of a hundred.
///
/// The same sentence as the rocker's, because it is the same kind of thing: a
/// press that has moved something, said where somebody is already looking. The
/// number is in it as well as on the bar, since a bar is a length and the
/// figure is what tells two adjacent presses apart.
pub fn said(points: i64) -> String {
    format!("Brightness {points}%")
}

pub fn at() -> PathBuf {
    PathBuf::from(DEVICE).join("brightness")
}

// ------------------------------------------------- the screen, left alone

/// Where the screen goes when nothing has happened for a while.
///
/// The floor, which is the dimmest this desktop will set by hand, and dim is
/// not off: what comes next is the screen going out altogether, and a step
/// that was already black would make the two indistinguishable to somebody
/// watching to see whether the machine is about to sleep.
pub const DIMMED: i64 = FLOOR;

/// Where the level it was at is kept while it is dimmed.
///
/// In the runtime directory and deliberately not in the home: a machine that
/// lost power while dim has nothing to restore and should not think it has.
/// The panel comes back at whatever the kernel gives it, which is a screen
/// somebody can see rather than one that stayed dark for a reason nobody can
/// find.
pub fn remembered() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var("XDG_RUNTIME_DIR").ok()?).join("console-dim"))
}

/// What to restore, given where the screen is now and what was remembered.
///
/// Nothing, if the screen is not where the dimming left it. Somebody who
/// reached for the rocker while it was dim has said what they want it at, and
/// putting the old level back the moment they touch a button would take it
/// away again -- the same press being both the wake and the change.
pub fn undimming(now: i64, was: i64) -> Option<i64> {
    match now == DIMMED {
        true => Some(was),
        false => None,
    }
}

/// What the screen is now, or nothing on a machine with another panel in it.
pub fn now() -> Option<i64> {
    std::fs::read_to_string(at()).ok()?.trim().parse().ok()
}

/// Move it, and say whether the machine let us.
pub fn set(to: i64) -> bool {
    std::fs::write(at(), format!("{to}\n")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_moves_it_one_step() {
        assert_eq!(stepped(20000, Way::Up), 26000);
        assert_eq!(stepped(20000, Way::Down), 14000);
    }

    /// Setting the panel to full turns the light off, so full is this number
    /// and never the one the file would take.
    #[test]
    fn it_never_goes_past_the_brightest_that_still_lights() {
        assert_eq!(stepped(CEILING, Way::Up), CEILING);
        assert_eq!(stepped(CEILING - 1, Way::Up), CEILING);
    }

    /// A screen nobody can read is not a brightness setting.
    #[test]
    fn it_never_goes_down_to_a_screen_nobody_can_read() {
        assert_eq!(stepped(FLOOR, Way::Down), FLOOR);
        assert_eq!(stepped(FLOOR + 1, Way::Down), FLOOR);
    }

    /// The two ends of the range are nought and a hundred, so the bar the panel
    /// draws is full at the brightest this screen goes rather than at a number
    /// that would black it out.
    #[test]
    fn the_bar_is_full_at_the_brightest_this_screen_goes() {
        assert_eq!(as_points(CEILING), 100);
        assert_eq!(as_points(FLOOR), 0);
        assert_eq!(as_points((CEILING + FLOOR) / 2), 50);
    }

    /// A machine whose backlight is somewhere else answers outside the range,
    /// and a bar cannot be drawn past its own ends.
    #[test]
    fn a_reading_from_outside_the_range_is_still_a_bar_that_can_be_drawn() {
        assert_eq!(as_points(65535), 100);
        assert_eq!(as_points(0), 0);
    }

    /// The round trip, which is the whole of what dimming promises.
    #[test]
    fn a_screen_that_was_dimmed_comes_back_where_it_was() {
        assert_eq!(undimming(DIMMED, 40000), Some(40000));
    }

    /// A hand on the rocker outranks the memory. The alternative is a press
    /// that wakes the screen and undoes itself in the same instant.
    #[test]
    fn a_screen_somebody_moved_while_it_was_dim_is_left_where_they_put_it() {
        assert_eq!(undimming(40000, 20000), None);
        assert_eq!(undimming(DIMMED + 1, 20000), None);
    }

    /// Dim is not off, and the difference is what tells somebody watching that
    /// the machine is going rather than gone.
    #[test]
    fn dimmed_is_still_a_screen_that_can_be_read() {
        assert_eq!(DIMMED, FLOOR);
        const { assert!(DIMMED > 0, "dimmed is off, and off is the step after") };
    }

    /// The figure is on the card as well as under it: mako draws the value
    /// hint as a fill, and a fill alone cannot be told from the one before it.
    #[test]
    fn the_notice_says_the_level_it_has_reached() {
        assert_eq!(said(as_points(CEILING)), "Brightness 100%");
        assert_eq!(said(as_points(FLOOR)), "Brightness 0%");
    }

    #[test]
    fn nothing_but_the_two_words_is_a_way() {
        assert_eq!(Way::named("up"), Some(Way::Up));
        assert_eq!(Way::named("Up"), None);
        assert_eq!(Way::named("get"), None);
    }
}
