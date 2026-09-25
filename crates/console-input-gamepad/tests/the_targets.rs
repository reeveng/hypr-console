//! What InputPlumber actually makes, held against what this desktop says it
//! will be.
//!
//! `targets.rs` names a target in the profile and, on the same line, the two
//! numbers the device arrives wearing, because those are what tell it from
//! another program's virtual pad. The numbers are InputPlumber's to choose and
//! not this tree's, so the one thing that can go wrong is that it changes them
//! and nothing here notices until a handheld comes up with no buttons.
//!
//! The capture is the device's own answer -- `just capture` writes it off the
//! machine -- so asking it here is the last moment that drift is cheap.

use console_input_gamepad::capture::captured;
use console_input_gamepad::targets::ASKED;

#[test]
fn every_target_this_desktop_asks_for_is_on_the_device_wearing_what_it_says() {
    let every = captured().expect("the capture");

    for target in ASKED {
        let Ok(identity) = target.identity();
        let Ok(asked) = target.asked();

        let found = every
            .values()
            .find(|told| told.vendor == identity.vendor && told.product == identity.product);

        assert!(
            found.is_some(),
            "the profile asks InputPlumber for {asked} and this desktop finds it by \
             {:#06x}:{:#06x}, which nothing in the capture is wearing. Either InputPlumber \
             names it differently now or it makes it differently, and the daemon will open \
             someone else's device or none at all",
            identity.vendor,
            identity.product
        );
    }
}

#[test]
fn nothing_else_on_the_device_is_wearing_a_targets_numbers() {
    let every = captured().expect("the capture");

    for target in ASKED {
        let Ok(identity) = target.identity();
        let Ok(asked) = target.asked();

        let wearing: Vec<&str> = every
            .values()
            .filter(|told| told.vendor == identity.vendor && told.product == identity.product)
            .map(|told| told.name.as_str())
            .collect();

        assert_eq!(
            wearing.len(),
            1,
            "{asked} is found by two numbers and {wearing:?} answer to them, so which device \
             the daemon opens is whichever the kernel lists first"
        );
    }
}
