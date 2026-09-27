//! Whether there is enough battery left to start an apply.
//!
//! An apply is minutes, and most of them are the build. Across those minutes
//! the one reading on this device that moves without anyone pressing anything
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
//! someone chose, the way everything else in this crate that decides something
//! is handed what it decides about.

use console_battery::{Cable, Charge, Levels, NEVER, Step};
use console_core_never::Never;

pub const MARGIN: i32 = 15;

pub const FLAT: i32 = MARGIN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enough {
    Yes,
    No(String),
}

pub fn required_charge(levels: Levels) -> Result<i32, Never> {
    let protect = levels.at(Step::Protect)?;

    Ok(match protect {
        NEVER => FLAT,
        protect => protect.saturating_add(MARGIN),
    })
}

pub fn enough(charge: Charge, levels: Levels) -> Result<Enough, Never> {
    let Ok(cable) = charge.filling.cable();

    match cable {
        Cable::Connected => return Ok(Enough::Yes),
        Cable::Disconnected => {},
    }

    let percent = match charge.percent {
        Some(percent) => percent,
        None => return Ok(Enough::Yes),
    };

    let Ok(wanted) = required_charge(levels);

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
    use console_battery::Filling;

    type Failure = Box<dyn std::error::Error>;

    const PROTECT: i32 = 5;

    const JUST_ENOUGH: i32 = PROTECT + MARGIN;

    const ONE_SHORT: i32 = PROTECT - 1 + MARGIN;

    const UNDER_THE_FLOOR: i32 = FLAT - 1;

    fn on_battery(percent: i32) -> Result<Charge, Never> {
        Ok(Charge { percent: Some(percent), filling: Filling::No })
    }

    fn levels(protect: i32) -> Result<Levels, Never> {
        Ok(Levels { low: 25, lower: 15, protect })
    }

    #[test]
    fn an_apply_wants_room_above_the_level_the_machine_stops_at() {
        let Ok(just_enough) = on_battery(JUST_ENOUGH);
        let Ok(one_short) = on_battery(ONE_SHORT);
        let Ok(levels) = levels(PROTECT);

        assert_eq!(enough(just_enough, levels), Ok(Enough::Yes));
        assert!(matches!(enough(one_short, levels), Ok(Enough::No(_))));
    }

    #[test]
    fn a_charge_that_would_be_stopped_partway_through_is_refused_before_it_starts() {
        let Ok(charge) = on_battery(8);
        let Ok(levels) = levels(PROTECT);
        let Ok(protect) = levels.at(Step::Protect);

        assert!(charge.percent > Some(protect), "this test is about the gap");
        assert!(matches!(enough(charge, levels), Ok(Enough::No(_))));
    }

    #[test]
    fn a_machine_that_is_charging_is_never_refused() {
        let filling = Charge { percent: Some(1), filling: Filling::Yes };
        let Ok(levels) = levels(PROTECT);

        assert_eq!(enough(filling, levels), Ok(Enough::Yes));
    }

    #[test]
    fn a_machine_on_the_cable_at_its_charge_limit_is_never_refused() {
        let held = Charge { percent: Some(1), filling: Filling::Charged };
        let Ok(levels) = levels(PROTECT);

        assert_eq!(
            enough(held, levels),
            Ok(Enough::Yes),
            "an apply was refused on a device sitting on its charger"
        );
    }

    #[test]
    fn a_machine_with_no_battery_is_never_refused() {
        let none = Charge { percent: None, filling: Filling::No };
        let Ok(levels) = levels(PROTECT);

        assert_eq!(enough(none, levels), Ok(Enough::Yes));
    }

    #[test]
    fn switching_the_step_off_leaves_a_floor_under_it() {
        let Ok(off) = levels(NEVER);
        let Ok(on_the_floor) = on_battery(FLAT);
        let Ok(under_it) = on_battery(UNDER_THE_FLOOR);

        assert_eq!(required_charge(off), Ok(FLAT));
        assert_eq!(enough(on_the_floor, off), Ok(Enough::Yes));
        assert!(matches!(enough(under_it, off), Ok(Enough::No(_))));
    }

    #[test]
    fn the_refusal_says_what_is_wrong_and_what_would_fix_it() -> Result<(), Failure> {
        let Ok(charge) = on_battery(8);
        let Ok(levels) = levels(PROTECT);
        let Ok(asked) = enough(charge, levels);
        let said = match asked {
            Enough::No(said) => said,
            Enough::Yes => return Err(Failure::from("it was allowed")),
        };

        assert!(said.contains("8%"), "{said}");
        assert!(said.contains("5%"), "{said}");
        assert!(said.contains("Plug it in"), "{said}");

        Ok(())
    }
}
