//! That every job this desktop has is on something that can reach it.
//!
//! This used to be a question about two profiles. A chooser wore one of its
//! own, so a button given a job on the desktop and forgotten in the chooser
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

use console_input_bindings::bound::Input;
use console_input_bindings::moved::Jobs;
use console_input_controller::means::{JOBS, Table, When};
use console_input_controller::mode::Mode;
use console_input_gamepad::routing::arrives;
use console_input_gamepad::vocabulary::button_name;

fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
    let Ok(value) = answer;

    value
}

fn table() -> Table {
    ok(Table::of(&ok(Jobs::none())))
}

fn mode_of(when: When) -> Mode {
    match when {
        When::WithAChooserUp => Mode::Tabs,
        When::OnTheHomeScreen => Mode::Home,
        When::StandingOnASquare => Mode::Standing,
        When::Anywhere | When::OnTheDesktop => Mode::Desktop,
    }
}

#[test]
fn every_job_is_on_a_button_that_reaches_the_daemon() {
    for job in JOBS {
        for (on, held, pressed) in job.bound {
            match on {
                Input::Keyboard => continue,
                Input::Pad => {},
            }

            for button in held.iter().chain([pressed]) {
                match ok(console_input_gamepad::vocabulary::is_trigger(button)) {
                    console_input_gamepad::vocabulary::Names::ATrigger => continue,
                    console_input_gamepad::vocabulary::Names::AButton => {},
                }

                let named = button_name(button).expect("a button this desktop has a word for");

                assert!(
                    ok(arrives(named)).is_some(),
                    "{} is on {button}, which arrives nowhere",
                    job.slug
                );
            }
        }
    }
}

#[test]
fn every_job_can_be_reached_by_pressing_what_it_is_bound_to() {
    let table = table();

    for job in JOBS {
        let mode = mode_of(job.when);

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
    let table = table();

    for job in JOBS {
        for (on, held, pressed) in job.bound {
            match on {
                Input::Keyboard => continue,
                Input::Pad => {},
            }

            assert!(
                ok(console_input_controller::buttons::job_for(
                    &table,
                    Mode::Keyboard,
                    held,
                    pressed
                ))
                .is_none(),
                "{} acts while the keyboard is up",
                job.slug
            );
        }
    }
}

#[test]
fn the_right_stick_pressed_is_the_same_answer_as_a() {
    let table = table();

    for mode in [
        Mode::Desktop,
        Mode::Tabs,
        Mode::Home,
        Mode::Standing,
        Mode::Keyboard,
        Mode::Asking,
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
