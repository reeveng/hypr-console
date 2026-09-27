//! That every job this desktop has is on something that can reach it.
//!
//! This used to be a question about two profiles. A picker wore one of its
//! own, so a button given a job on the desktop and forgotten in the picker
//! reached whatever was underneath -- and whether an unmapped button passes
//! through is not written down anywhere and was never worth resting on.
//!
//! There is one profile now and it names every button, so that half of the
//! question is answered by the routing table. What is left is the other half,
//! and it is the half that can still go wrong: a job bound to a button the
//! profile does not route is a job nothing can ever reach, and the table of
//! jobs and the table of routes are two files that have to agree.
//!
//! A keyboard binding is asked the same question and answered by a different
//! half of the machine: the compositor carries it, so what is checked here is
//! that the table can find it again, and `the_binds_the_compositor_is_given`
//! is where the other end is held to it.

use console_core_never::Never;
use console_input_bindings::bound::Input;
use console_input_gamepad::vocabulary::{Names, is_trigger};
use console_input_bindings::moved::Tasks;
use console_input_controller::actions::{JOBS, Table, Context};
use console_input_controller::mode::Mode;
use console_input_gamepad::routing::arrives;
use console_input_gamepad::vocabulary::button_name;

type Failure = Box<dyn std::error::Error>;

fn unmoved() -> Result<Table, Never> {
    let Ok(none) = Tasks::none();

    Table::of(&none)
}

fn mode_of(when: Context) -> Result<Mode, Never> {
    Ok(match when {
        Context::WithAPickerUp => Mode::Tabs,
        Context::OnTheHomeScreen => Mode::HomeScreen,
        Context::StandingOnASquare => Mode::Standing,
        Context::Anywhere | Context::OnTheDesktop => Mode::Desktop,
    })
}

#[test]
fn every_job_is_on_a_button_that_reaches_the_daemon() -> Result<(), Failure> {
    for job in JOBS {
        for (_, held, pressed) in job.bound.iter().filter(|(on, _, _)| *on == Input::Pad) {
            let buttons = held.iter().chain([pressed]).filter(|button| is_trigger(button) == Ok(Names::AButton));

            for button in buttons {
                let named = button_name(button)?;

                assert_ne!(arrives(named), Ok(None), "{} is on {button}, which arrives nowhere", job.slug);
            }
        }
    }

    Ok(())
}

#[test]
fn every_job_can_be_reached_by_pressing_what_it_is_bound_to() {
    let Ok(table) = unmoved();

    for job in JOBS {
        let Ok(mode) = mode_of(job.context);

        for (on, held, pressed) in job.bound {
            let Ok(found) = table.what(*on, held, pressed, mode);

            assert_eq!(
                found.map(|found| found.slug),
                Some(job.slug),
                "{} is unreachable on {pressed}",
                job.slug
            );
        }
    }
}

#[test]
fn the_keyboard_keeps_the_pad_while_it_is_up() {
    let Ok(table) = unmoved();

    for job in JOBS {
        for (_, held, pressed) in job.bound.iter().filter(|(on, _, _)| *on == Input::Pad) {
            assert_eq!(
                console_input_controller::buttons::job_for(&table, Mode::Keyboard, held, pressed),
                Ok(None),
                "{} acts while the keyboard is up",
                job.slug
            );
        }
    }
}

#[test]
fn the_right_stick_pressed_is_the_same_answer_as_a() {
    let Ok(table) = unmoved();

    for mode in [
        Mode::Desktop,
        Mode::Tabs,
        Mode::HomeScreen,
        Mode::Standing,
        Mode::Keyboard,
        Mode::Prompt,
    ] {
        let Ok(accepts) = table.what(Input::Pad, &[], "a", mode);
        let Ok(stick) = table.what(Input::Pad, &[], "r3", mode);

        assert_eq!(
            accepts.map(|job| job.slug),
            stick.map(|job| job.slug),
            "the right stick pressed and A part company in {mode:?}, \
             so the thumb already on the stick has to move to accept"
        );
    }
}
