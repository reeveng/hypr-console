//! The control center, pulled down from the top edge by a finger and put away
//! by one.
//!
//! It is a strip a few points deep along the top of the screen that grows into
//! a sheet when a finger drawn from it has gone far enough down, and nothing
//! else on the device can bring it. So the only question worth asking is the
//! finger's: a swipe from the edge, and then the size the compositor says the
//! surface is. A long-running control center once stopped answering any finger
//! at all while still holding its strip on the screen, which looks exactly
//! like one that is fine to everything that is not a finger.

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, CheckResult, failed, happened};
use console_test_stages::device::{Device, PATIENCE, Ready};

pub const PULLED: Check = Check {
    name: "480-a-pull-from-the-top-edge-brings-the-control-center",
    about: "A finger drawn down from the top edge opens the control center, and a tap outside it puts it away.",
    feature: "control-center",
    since: "2026-09-24",
    bodies: &[Body::Device(pulled)],
};

const EDGE: u32 = 10;

fn height(seen: &mut Device) -> Result<Option<u32>, Never> {
    let Ok(layer) = seen.layer(console_onscreen::CONTROL_CENTER);

    Ok(layer.map(|(_, _, _, height)| height))
}

fn opened(seen: &mut Device) -> Result<Ready, Never> {
    let Ok(now) = height(seen);

    Ok(match now.is_some_and(|height| height > EDGE) {
        true => Ready::Yes,
        false => Ready::NotYet,
    })
}

fn put_away(seen: &mut Device) -> Result<Ready, Never> {
    let Ok(now) = height(seen);

    Ok(match now == Some(EDGE) {
        true => Ready::Yes,
        false => Ready::NotYet,
    })
}

fn pulled(stage: &mut Device) -> CheckResult {
    let Ok(before) = height(stage);

    match before {
        Some(EDGE) => {},
        Some(other) => return failed(format!("the control center was {other} tall before anything pulled it, not the {EDGE} of its edge")),
        None => return failed("the control center has no edge on the screen to pull".to_string()),
    }

    let middle = 512;

    stage.swipe((middle, 4), (middle, 300))?;

    let Ok(came) = stage.until::<Never>(opened, PATIENCE);
    let Ok(now) = height(stage);

    happened(came, || format!("a finger drawn down from the top edge left the control center {now:?} tall"))?;

    stage.touch((middle, 600))?;

    let Ok(went) = stage.until::<Never>(put_away, PATIENCE);
    let Ok(now) = height(stage);

    happened(went, || format!("a tap outside the sheet left the control center {now:?} tall"))
}
