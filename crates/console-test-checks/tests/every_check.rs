//! Every check, run here, as part of the ordinary suite.
//!
//! The checks exist to be replayed against the device at the end. That only
//! means anything if they still run at all, and a check nobody has run since the
//! feature changed is a check that will fail on the device for a reason that has
//! nothing to do with the device. So they are also the fast suite: every one of
//! them that can run without a machine runs on every `cargo test`.
//!
//! What is not here is a count of them. There was a test that wanted more than
//! half to answer here, and it went red the day a check arrived that only a
//! machine can answer -- which is a fact about that feature rather than about
//! the suite, and a number that goes red for a reason that is not a feature is
//! the thing `docs/checks.md` argues against. Where a check runs is decided by
//! what it has to press. Arithmetic is a unit test beside the code that does
//! it, and what is left over is what nothing here can answer.

use console_test_checks::CHECKS;
use console_test_stages::checking::{How, here};
use console_test_stages::here::Here;

#[test]
fn every_check_written_for_here_passes_here() {
    let failed: Vec<String> = CHECKS
        .into_iter()
        .filter_map(|check| {
            let mut stage = Here::new().expect("a stage");
            let Ok(how) = here(check, &mut stage);

            match how {
                How::Failed(why) => Some(format!("{}: {why}", check.name)),
                _ => None,
            }
        })
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn a_check_is_a_number_and_what_it_is_about() {
    for check in CHECKS {
        let Ok(rest) = check.rest();

        assert!(!rest.is_empty(), "{} is a number and nothing else", check.name);
    }
}
