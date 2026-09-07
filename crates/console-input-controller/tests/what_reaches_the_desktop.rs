//! That every job this desktop has is on a button that can reach it.
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

use console_input_controller::means::{JOBS, Table};
use console_input_controller::mode::Mode;
use console_input_gamepad::jobs::{ALONE, Jobs};
use console_input_gamepad::routing::arrives;
use console_input_gamepad::vocabulary::button_name;

fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
    let Ok(value) = answer;

    value
}

#[test]
fn every_job_is_on_a_button_that_reaches_the_daemon() {
    for job in JOBS {
        for (_, button) in job.bound {
            let named = button_name(button).expect("a button this desktop has a word for");
            assert!(
                ok(arrives(named)).is_some(),
                "{} is on {button}, which arrives nowhere",
                job.slug
            );
        }
    }
}

#[test]
fn every_job_can_be_reached_by_pressing_what_it_is_bound_to() {
    let table = ok(Table::of(&ok(Jobs::none())));
    for job in JOBS {
        let mode = match job.when {
            console_input_controller::means::When::WithAChooserUp => Mode::Tabs,
            console_input_controller::means::When::OnTheHomeScreen => Mode::Home,
            console_input_controller::means::When::StandingOnASquare => Mode::Standing,
            _ => Mode::Desktop,
        };
        for (layer, button) in job.bound {
            let Ok(found) = table.what(button, *layer, mode);

            assert_eq!(found.map(|found| found.slug), Some(job.slug), "{} is unreachable", job.slug);
        }
    }
}

#[test]
fn the_keyboard_keeps_the_pad_while_it_is_up() {
    let table = ok(Table::of(&ok(Jobs::none())));
    for job in JOBS {
        for (layer, button) in job.bound {
            assert!(
                ok(console_input_controller::buttons::job_for(&table, Mode::Keyboard, button, *layer))
                    .is_none(),
                "{} acts while the keyboard is up",
                job.slug
            );
        }
    }
}

#[test]
fn the_right_stick_pressed_is_the_same_answer_as_a() {
    let table = ok(Table::of(&ok(Jobs::none())));

    for mode in [
        Mode::Desktop,
        Mode::Tabs,
        Mode::Home,
        Mode::Standing,
        Mode::Keyboard,
        Mode::Asking,
    ] {
        let Ok(accepts) = table.what("a", ALONE, mode);
        let Ok(stick) = table.what("r3", ALONE, mode);

        assert_eq!(
            accepts.map(|job| job.slug),
            stick.map(|job| job.slug),
            "the right stick pressed and A part company in {mode:?}, \
             so the thumb already on the stick has to move to accept"
        );
    }
}
