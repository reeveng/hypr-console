//! Whether there is enough battery left to start an apply.
//!
//! An apply is minutes, and most of them are the build. Across those minutes
//! the one reading on this device that moves without anybody pressing anything
//! goes on moving, and `console-battery` is watching it: at the protect step it
//! stops the machine, on purpose, before the battery stops it for them. That is
//! the right thing for it to do and it is aimed squarely at the one operation
//! here that must not be interrupted.
//!
//! Nothing used to stand between those two. An apply started at the wrong
//! moment on a battery low enough would be powered off partway through --
//! somewhere in the build if it was lucky, somewhere in the swap if it was not
//! -- by a piece of this desktop doing exactly its job.
//!
//! So an apply asks first. What it asks is not "is the battery low", which is a
//! question about now; it is "will this machine still be running when this
//! finishes", which is a question about the next several minutes and is why the
//! answer is a level rather than a reading.
//!
//! Nothing here reads the machine. It is handed a charge and the levels
//! somebody chose, the way everything else in this crate that decides something
//! is handed what it decides about.

use console_defaults::battery::{Charge, Filling, Levels, NEVER, Step};

/// How much room an apply wants above the level the machine stops itself at.
///
/// An apply is minutes and a battery on this device moves several points in
/// that time, so a charge exactly at the protect level is a machine that stops
/// during the build rather than one that stops before it. Fifteen is more than
/// the drop an apply has been watched to make and less than the room somebody
/// would call being refused for no reason.
pub const MARGIN: i32 = 15;

/// The floor under an apply on a machine that has switched the protect step
/// off.
///
/// The step being set to never means nobody wants to be stopped early. It does
/// not mean the battery lasts for ever, and a build begun at four per cent ends
/// the same way whatever the setting says. So there is still a floor and it is
/// the flatness the hardware imposes rather than the one a person chose.
pub const FLAT: i32 = MARGIN;

/// Whether to start an apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enough {
    /// Start it.
    Yes,
    /// Do not, and this is what to tell whoever asked.
    No(String),
}

/// The charge an apply wants before it begins.
///
/// The protect level plus room to finish, or the flat floor where the protect
/// step is switched off.
pub fn wanted(levels: Levels) -> i32 {
    match levels.at(Step::Protect) {
        NEVER => FLAT,
        protect => protect + MARGIN,
    }
}

/// Whether there is enough of it.
///
/// A machine on the mains is always enough: the reading may be low and it is
/// going the other way, and refusing there would refuse the exact case somebody
/// plugs the device in for.
///
/// A machine with no battery is enough as well, and for a plainer reason than
/// it looks. `percent` is nothing on a device whose battery will not answer,
/// which is every machine that has none -- and a machine with no battery cannot
/// run out of one. The alternative is a desktop that refuses to install itself
/// because it cannot find a battery to worry about.
pub fn enough(charge: Charge, levels: Levels) -> Enough {
    if charge.filling == Filling::Yes {
        return Enough::Yes;
    }

    let Some(percent) = charge.percent else { return Enough::Yes };

    let wanted = wanted(levels);

    if percent >= wanted {
        return Enough::Yes;
    }

    Enough::No(format!(
        "the battery is at {percent}% and is not charging. An apply is minutes, and this machine \
         stops itself at {}%, so it wants {wanted}% to be sure of finishing. Plug it in.",
        levels.at(Step::Protect)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_battery(percent: i32) -> Charge {
        Charge { percent: Some(percent), filling: Filling::No }
    }

    fn levels(protect: i32) -> Levels {
        Levels { low: 25, lower: 15, protect }
    }

    /// The whole of it: a charge that would not survive the apply is refused,
    /// and one that would is not.
    #[test]
    fn an_apply_wants_room_above_the_level_the_machine_stops_at() {
        assert_eq!(enough(on_battery(5 + MARGIN), levels(5)), Enough::Yes);
        assert!(matches!(enough(on_battery(4 + MARGIN), levels(5)), Enough::No(_)));
    }

    /// The fault this exists for, written as the thing that used to happen: a
    /// charge above the protect level, so nothing warns, and not far enough
    /// above it to reach the end of a build.
    #[test]
    fn a_charge_that_would_be_stopped_partway_through_is_refused_before_it_starts() {
        let charge = on_battery(8);
        let levels = levels(5);
        assert!(charge.percent > Some(levels.at(Step::Protect)), "this test is about the gap");
        assert!(matches!(enough(charge, levels), Enough::No(_)));
    }

    /// On the mains, at any reading. Refusing here would refuse the one thing
    /// somebody does about a low battery.
    #[test]
    fn a_machine_that_is_charging_is_never_refused() {
        let filling = Charge { percent: Some(1), filling: Filling::Yes };
        assert_eq!(enough(filling, levels(5)), Enough::Yes);
    }

    /// A machine with no battery cannot run out of one. Otherwise a desktop
    /// refuses to install itself for want of something to worry about.
    #[test]
    fn a_machine_with_no_battery_is_never_refused() {
        let none = Charge { percent: None, filling: Filling::No };
        assert_eq!(enough(none, levels(5)), Enough::Yes);
    }

    /// The protect step switched off does not switch off the hardware. There is
    /// still a floor and it is what flat means.
    #[test]
    fn switching_the_step_off_leaves_a_floor_under_it() {
        assert_eq!(wanted(levels(NEVER)), FLAT);
        assert_eq!(enough(on_battery(FLAT), levels(NEVER)), Enough::Yes);
        assert!(matches!(enough(on_battery(FLAT - 1), levels(NEVER)), Enough::No(_)));
    }

    /// The refusal says the reading, the level and what to do, because whoever
    /// reads it is holding a handheld and has just been stopped.
    #[test]
    fn the_refusal_says_what_is_wrong_and_what_would_fix_it() {
        let Enough::No(said) = enough(on_battery(8), levels(5)) else { panic!("it was allowed") };
        assert!(said.contains("8%"), "{said}");
        assert!(said.contains("5%"), "{said}");
        assert!(said.contains("Plug it in"), "{said}");
    }
}
