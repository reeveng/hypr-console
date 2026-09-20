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

use console_test_stages::checking::{Check, Body, Done, failed};
use console_test_stages::device::Device;

pub const KEPT: Check = Check {
    name: "450-what-the-machine-is-spending",
    about: "Two readings taken on the device add up to a report naming what held the CPU.",
    feature: "resource-usage",
    since: "2026-09-19",
    bodies: &[Body::Device(kept_there)],
};

const STORE: &str = ".cache/console-resource-usage-check.jsonl";

const A_PROGRAM: &str = "% of a core";

fn kept_there(stage: &mut Device) -> Done {
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
