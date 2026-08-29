//! Every check, run here, as part of the ordinary suite.
//!
//! The checks exist to be replayed against the device at the end. That only
//! means anything if they still run at all, and a check nobody has run since the
//! feature changed is a check that will fail on the device for a reason that has
//! nothing to do with the device. So they are also the fast suite: every one of
//! them that can run without a machine runs on every `cargo test`.

use console_checks::CHECKS;
use console_stage::checking::{How, here};
use console_stage::here::Here;

#[test]
fn every_check_written_for_here_passes_here() {
    let failed: Vec<String> = CHECKS
        .into_iter()
        .filter_map(|check| {
            let mut stage = Here::new().expect("a stage");
            match here(check, &mut stage) {
                How::Failed(why) => Some(format!("{}: {why}", check.name)),
                _ => None,
            }
        })
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A feature split across checks is split on purpose, into parts that fail
/// separately. Two checks claiming the whole of one feature is the thing this is
/// meant not to become.
#[test]
fn a_check_is_a_number_and_what_it_is_about() {
    for check in CHECKS {
        assert!(!check.rest().is_empty(), "{} is a number and nothing else", check.name);
    }
}

/// The point of the emulator is that most of them do.
#[test]
fn most_of_the_checks_can_be_answered_without_a_machine() {
    let answered = CHECKS
        .into_iter()
        .filter(|check| {
            let mut stage = Here::new().expect("a stage");
            here(check, &mut stage) == How::Ok
        })
        .count();
    assert!(answered * 2 > CHECKS.len(), "only {answered} of {} run here", CHECKS.len());
}
