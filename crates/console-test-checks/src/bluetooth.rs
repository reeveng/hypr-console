//! The Bluetooth tab looks while it is the tab in front.
//!
//! bluez keeps what it found only while it is looking. Discovery stops and
//! everything that was never paired is dropped within seconds, so a tab that
//! scanned for eight seconds and then asked what was there read the leavings of
//! a scan rather than the scan -- which is why a room's worth of untitled
//! addresses arrived and the thing someone was holding did not. The looking is
//! the tab being in front now, and whether the radio agreed is a question only
//! a machine with a radio can be asked.
//!
//! The radio is asked through `busctl`, which every systemd machine has, rather
//! than through what the tab runs: a check that read bluez the way the tab does
//! would agree with the tab about whatever the tab had wrong. It used to be
//! `bluetoothctl show`, which left with bluez-utils when nothing on the desktop
//! ran it any more.

use console_core_never::Never;
use console_test_stages::checking::{Body, Check, CheckResult, cannot, happened};
use console_test_stages::device::{Device, PATIENCE, Ready};

pub const LOOKS: Check = Check {
  name: "430-the-bluetooth-tab-looks",
  about: "The Bluetooth tab has the radio looking while it is up, and stops when it goes.",
  feature: "bluetooth",
  since: "2026-09-08",
  bodies: &[Body::Device(there)],
};

const SEARCHING: &str =
  "busctl --system get-property org.bluez /org/bluez/hci0 org.bluez.Adapter1 Discovering";

const POWERED: &str =
  "busctl --system get-property org.bluez /org/bluez/hci0 org.bluez.Adapter1 Powered";

const YES: &str = "b true";

const TAB: &str = "settings-panel Bluetooth";

fn looking(stage: &mut Device) -> Result<Ready, Never> {
  let Ok(said) = stage.user(SEARCHING);

  Ok(match ::core::primitive::bool::then_some(said.lines().any(|line| line.trim() == YES), ()) {
    ::core::option::Option::Some(()) => Ready::Yes,
    ::core::option::Option::None => Ready::NotYet,
  })
}

fn there(stage: &mut Device) -> CheckResult {
  let Ok(said) = stage.user(POWERED);

  match ::core::primitive::bool::then_some(said.lines().any(|line| line.trim() == YES), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return cannot("the radio is off, so there is nothing here that can look"),
  }

  let Ok(_) = stage.exec_cmd(TAB);
  let Ok(drawn) = stage.wait_for_menu(PATIENCE);

  happened(drawn, || "the settings panel did not draw".to_string())?;

  let Ok(looked) = stage.until(looking, PATIENCE);
  let Ok(()) = stage.press("b");
  let Ok(gone) = stage.wait_for_close(PATIENCE);
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
    "the panel went and the radio is still looking, which is a scan no one asked for".to_string()
  })
}
