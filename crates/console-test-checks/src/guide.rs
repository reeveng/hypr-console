//! The Menu button opens the guide to what every button does, which is the
//! Buttons panel: what each one does, changed where it is read.

use console_test_stages::checking::{Body, Check, CheckResult};
use console_test_stages::device::Device;
use console_test_stages::here::Here;

use crate::picker::{Expected, opens};

pub const GUIDE: Check = Check {
    name: "050-the-guide",
    about: "The Menu button opens the guide to what every button does.",
    feature: "guide",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> CheckResult {
    stage.press("menu")?;
    stage.ran(&[&["/usr/local/bin/mapping-panel"]])
}

fn there(stage: &mut Device) -> CheckResult {
    opens(stage, "menu", Expected("guide"))
}
