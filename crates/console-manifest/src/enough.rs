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
use console_never::Never;

pub const MARGIN: i32 = 15;

pub const FLAT: i32 = MARGIN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enough {
    Yes,
    No(String),
}

pub fn wanted(levels: Levels) -> Result<i32, Never> {
    let protect = levels.at(Step::Protect)?;

    Ok(match protect {
        NEVER => FLAT,
        protect => protect.saturating_add(MARGIN),
    })
}

pub fn enough(charge: Charge, levels: Levels) -> Result<Enough, Never> {
    match charge.filling == Filling::Yes {
        true => return Ok(Enough::Yes),
        false => {},
    }

    let Some(percent) = charge.percent else { return Ok(Enough::Yes) };

    let Ok(wanted) = wanted(levels);

    match percent >= wanted {
        true => return Ok(Enough::Yes),
        false => {},
    }

    let protect = levels.at(Step::Protect)?;

    Ok(Enough::No(format!(
        "the battery is at {percent}% and is not charging. An apply is minutes, and this machine \
         stops itself at {protect}%, so it wants {wanted}% to be sure of finishing. Plug it in."
    )))
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

    fn asking(charge: Charge, levels: Levels) -> Enough {
        let Ok(enough) = enough(charge, levels);

        enough
    }

    fn wants(levels: Levels) -> i32 {
        let Ok(wanted) = wanted(levels);

        wanted
    }

    #[test]
    fn an_apply_wants_room_above_the_level_the_machine_stops_at() {
        assert_eq!(asking(on_battery(5 + MARGIN), levels(5)), Enough::Yes);
        assert!(matches!(asking(on_battery(4 + MARGIN), levels(5)), Enough::No(_)));
    }

    #[test]
    fn a_charge_that_would_be_stopped_partway_through_is_refused_before_it_starts() {
        let charge = on_battery(8);
        let levels = levels(5);
        let Ok(protect) = levels.at(Step::Protect);

        assert!(charge.percent > Some(protect), "this test is about the gap");
        assert!(matches!(asking(charge, levels), Enough::No(_)));
    }

    #[test]
    fn a_machine_that_is_charging_is_never_refused() {
        let filling = Charge { percent: Some(1), filling: Filling::Yes };
        assert_eq!(asking(filling, levels(5)), Enough::Yes);
    }

    #[test]
    fn a_machine_with_no_battery_is_never_refused() {
        let none = Charge { percent: None, filling: Filling::No };
        assert_eq!(asking(none, levels(5)), Enough::Yes);
    }

    #[test]
    fn switching_the_step_off_leaves_a_floor_under_it() {
        assert_eq!(wants(levels(NEVER)), FLAT);
        assert_eq!(asking(on_battery(FLAT), levels(NEVER)), Enough::Yes);
        assert!(matches!(asking(on_battery(FLAT - 1), levels(NEVER)), Enough::No(_)));
    }

    #[test]
    fn the_refusal_says_what_is_wrong_and_what_would_fix_it() {
        let Enough::No(said) = asking(on_battery(8), levels(5)) else { panic!("it was allowed") };
        assert!(said.contains("8%"), "{said}");
        assert!(said.contains("5%"), "{said}");
        assert!(said.contains("Plug it in"), "{said}");
    }
}
