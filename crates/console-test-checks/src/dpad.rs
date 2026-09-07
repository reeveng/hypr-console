//! The d-pad on its own moves between things and does nothing else.

use console_test_stages::checking::{Body, Check, Done, empty, same};
use console_test_stages::device::{A_MOMENT, Device};
use console_test_stages::here::{Here, TURNS};

const WAYS: [&str; 4] = ["dpad-down", "dpad-left", "dpad-right", "dpad-up"];

pub const DPAD: Check = Check {
    name: "100-the-dpad-does-not-act",
    about: "The d-pad on its own moves between things and does nothing else.",
    feature: "dpad",
    since: "2026-08-26",
    bodies: &[Body::Here(here), Body::Device(there)],
};

fn here(stage: &mut Here) -> Done {
    for way in WAYS {
        stage.press(way)?;
    }

    let Ok(()) = stage.settle(TURNS);
    let Ok(ran) = stage.names();

    empty(&ran, || format!("the d-pad ran {ran:?}"))
}

fn there(stage: &mut Device) -> Done {
    let Ok(where_) = stage.workspace();
    let Ok(windows) = stage.windows();

    for way in WAYS {
        let Ok(()) = stage.press(way);
    }

    #[cfg_attr(
        dylint_lib = "explicit022_no_settling",
        allow(
            explicit022_no_settling,
            reason = "what is being checked is that nothing happened, and nothing happening has no question to ask: the number is how long the desktop is given to do the wrong thing before the check believes it did not"
        )
    )]
    let Ok(()) = stage.settle(A_MOMENT);

    let Ok(now) = stage.workspace();

    same(&now, &where_, || format!("the d-pad moved the desktop to {now}"))?;

    let Ok(after) = stage.windows();

    same(&after, &windows, || "the d-pad opened or closed something".to_string())
}
