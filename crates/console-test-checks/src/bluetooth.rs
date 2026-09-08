//! The Bluetooth tab looks while it is the tab in front.
//!
//! bluez keeps what it found only while it is looking. Discovery stops and
//! everything that was never paired is dropped within seconds, so a tab that
//! scanned for eight seconds and then asked what was there read the leavings of
//! a scan rather than the scan -- which is why a room's worth of nameless
//! addresses arrived and the thing somebody was holding did not. The looking is
//! the tab being in front now, and whether the radio agreed is a question only
//! a machine with a radio can be asked.

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, Done, cannot, happened};
use console_test_stages::device::{Device, PATIENCE, Seen};

pub const LOOKS: Check = Check {
    name: "430-the-bluetooth-tab-looks",
    about: "The Bluetooth tab has the radio looking while it is up, and stops when it goes.",
    feature: "bluetooth",
    since: "2026-09-08",
    bodies: &[Body::Device(there)],
};

const SHOW: &str = "bluetoothctl show";

const TAB: &str = "settings-panel Bluetooth";

fn looking(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(said) = stage.user(SHOW);

    Ok(match said.contains("Discovering: yes") {
        true => Seen::Yes,
        false => Seen::NotYet,
    })
}

fn there(stage: &mut Device) -> Done {
    let Ok(said) = stage.user(SHOW);

    match said.contains("Powered: yes") {
        true => {},
        false => return cannot("the radio is off, so there is nothing here that can look"),
    }

    let Ok(_) = stage.exec_cmd(TAB);
    let Ok(drawn) = stage.drawn(PATIENCE);

    happened(drawn, || "the settings panel did not draw".to_string())?;

    let Ok(looked) = stage.until(looking, PATIENCE);
    let Ok(()) = stage.press("b");
    let Ok(gone) = stage.gone(PATIENCE);
    let Ok(stopped) = stage.until(
        |seen| {
            let Ok(up) = looking(seen);

            up.flipped()
        },
        PATIENCE,
    );

    happened(looked, || {
        "the Bluetooth tab is in front and the radio is not looking for anything".to_string()
    })?;

    happened(gone, || "B did not put the panel away".to_string())?;

    happened(stopped, || {
        "the panel went and the radio is still looking, which is a scan nobody asked for"
            .to_string()
    })
}
