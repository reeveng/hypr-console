//! The machine writes down what it is spending, and can say it back.
//!
//! The two readings are taken a second apart, and the second is the only
//! duration in this crate that is the thing being checked rather than a guess
//! at how long something takes. A reading here is what changed between two
//! moments -- how much uptime went by, what the battery spent over it, which
//! programs held the CPU while it did -- so two readings taken in the same
//! instant are not a measurement and `console-resource-usage` says exactly
//! that. Both `note` calls were in one command with a semicolon between them,
//! which is two process starts and often the same hundredth of a second: the
//! check asked for a report over no time at all and then failed because it did
//! not get one.

use console_test_stages::checking::{Check, Body, CheckResult, failed};
use console_test_stages::device::Device;

pub const KEPT: Check = Check {
    name: "450-what-the-machine-is-spending",
    about: "Two readings taken on the device add up to a report naming what held the CPU.",
    feature: "resource-usage",
    since: "2026-09-19",
    bodies: &[Body::Device(kept_there)],
};

pub const ON_ITS_OWN: Check = Check {
    name: "451-the-timer-reading-is-taken-whole",
    about: "The reading the timer takes, inside its unit's sandbox, is taken without a complaint about anything it could not ask.",
    feature: "resource-usage",
    since: "2026-09-25",
    bodies: &[Body::Device(taken_on_its_own)],
};

const UNIT: &str = "console-resource-usage.service";

const COMPLAINT: &str = "console-resource-usage:";

const STORE: &str = ".cache/console-resource-usage-check.jsonl";

const A_PROGRAM: &str = "% of a core";

fn kept_there(stage: &mut Device) -> CheckResult {
    let Ok(home) = stage.home();
    let store = format!("{home}/{STORE}");
    let Ok(_) = stage.user(&format!("rm -f {store}"));
    let Ok(_) = stage.user(&format!(
        "console-resource-usage note --file {store}; sleep 1; \
         console-resource-usage note --file {store}"
    ));
    let Ok(said) = stage.user(&format!("console-resource-usage --file {store}"));
    let Ok(_) = stage.user(&format!("rm -f {store}"));

    match said.contains(A_PROGRAM) {
        true => Ok(()),
        false => failed(format!("two readings came to {said:?}")),
    }
}

fn taken_on_its_own(stage: &mut Device) -> CheckResult {
    let Ok(_) = stage.user(&format!("systemctl --user start {UNIT}"));
    let Ok(said) = stage.user(&format!("journalctl --user -u {UNIT} -o cat --since=-1min --no-pager"));

    match said.contains(COMPLAINT) {
        false => Ok(()),
        true => failed(format!("the timer's reading said {said:?}")),
    }
}
