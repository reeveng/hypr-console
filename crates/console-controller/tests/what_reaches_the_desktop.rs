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

use console_controller::means::{JOBS, Table};
use console_controller::mode::Mode;
use console_pad::jobs::Jobs;
use console_pad::routing::arrives;
use console_pad::vocabulary::button_name;

/// Every job is on a button that arrives somewhere.
#[test]
fn every_job_is_on_a_button_that_reaches_the_daemon() {
    for job in JOBS {
        for (_, button) in job.bound {
            let named = button_name(button).expect("a button this desktop has a word for");
            assert!(arrives(named).is_some(), "{} is on {button}, which arrives nowhere", job.slug);
        }
    }
}

/// And every button a job is on can be pressed to reach that job, in the place
/// the job is for. The table is asked the way the daemon asks it, so a job
/// that is bound and unreachable is a failure here rather than a surprise on
/// the machine.
#[test]
fn every_job_can_be_reached_by_pressing_what_it_is_bound_to() {
    let table = Table::of(&Jobs::none());
    for job in JOBS {
        let mode = match job.when {
            console_controller::means::When::WithAChooserUp => Mode::Tabs,
            // The home screen's own are on buttons the desktop has jobs for,
            // and the desktop's are what a press reaches while the apps are
            // not drawn. Each is asked about where it is for, which is the
            // whole of what this test is: a job nothing can press is a job
            // nobody has.
            console_controller::means::When::OnTheHomeScreen => Mode::Home,
            console_controller::means::When::StandingOnASquare => Mode::Standing,
            _ => Mode::Desktop,
        };
        for (layer, button) in job.bound {
            let found = table.what(button, *layer, mode);
            assert_eq!(found.map(|found| found.slug), Some(job.slug), "{} is unreachable", job.slug);
        }
    }
}

/// The one thing the on-screen keyboard needs of all this: while it is up, the
/// pad is its own. It reads the pad itself, and a daemon acting on the same
/// presses would move the highlight twice.
#[test]
fn the_keyboard_keeps_the_pad_while_it_is_up() {
    let table = Table::of(&Jobs::none());
    for job in JOBS {
        for (layer, button) in job.bound {
            assert!(
                console_controller::buttons::job_for(&table, Mode::Keyboard, button, *layer)
                    .is_none(),
                "{} acts while the keyboard is up",
                job.slug
            );
        }
    }
}
