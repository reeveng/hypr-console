//! What the front of this machine actually is, asked of the machine.
//!
//! `wanted` is what the desktop binds. This is the other half: what the thing
//! in somebody's hands can actually send. The two are compared at install
//! time, and where they differ the answer is a notice rather than a failure --
//! a desktop that refuses to install on a device missing one paddle is worse
//! than one that installs and says which promise it cannot keep.
//!
//! InputPlumber is the only thing that can answer. Half of what this device
//! sends never appears in `/dev/input` at all: the paddles, the Legion buttons
//! and the button with a keyboard on it are read off hidraw by the driver
//! `50-legion_go.yaml` selects, and it selects it by the DMI of this machine.
//! So enumerating input devices would say a Legion Go has no paddles, which is
//! both wrong and the exact mistake this module exists to avoid. The composite
//! device is asked instead, and it answers in the same words the profiles are
//! written in:
//!
//! ```text
//! as 41 "Gamepad:Button:South" "Gamepad:Button:LeftPaddle1" "Gyroscope:Center" ...
//! ```
//!
//! Nothing here opens a bus or reads a file. What was said is handed in, so
//! every rule can be asked of a machine that is not in the room -- including
//! the machine this desktop has never run on, which is the one that matters.


use console_core_external_programs::Program;
use console_core_never::Never;

use crate::devices::Has;
use std::collections::BTreeSet;


pub const BUS: &str = "org.shadowblip.InputPlumber";
pub const OBJECT: &str = "/org/shadowblip/InputPlumber/CompositeDevice0";
pub const INTERFACE: &str = "org.shadowblip.Input.CompositeDevice";

pub fn asking() -> Result<Vec<&'static str>, Never> {
    let Ok(busctl) = Program::Busctl.name();

    Ok(vec![
        busctl,
        "--system",
        "get-property",
        BUS,
        OBJECT,
        INTERFACE,
        "Capabilities",
    ])
}

pub fn wearing() -> Result<Vec<&'static str>, Never> {
    let Ok(busctl) = Program::Busctl.name();

    Ok(vec![
        busctl,
        "--system",
        "get-property",
        BUS,
        OBJECT,
        INTERFACE,
        "ProfilePath",
    ])
}

pub fn loading(path: &str) -> Result<Vec<String>, Never> {
    Program::Busctl.argv(&[
        "--system",
        "call",
        BUS,
        OBJECT,
        INTERFACE,
        "LoadProfilePath",
        "s",
        path,
    ])
}

pub fn one_said(said: &str) -> Result<Option<String>, Never> {
    Ok(said.split('"').nth(1).filter(|said| !said.is_empty()).map(str::to_string))
}

pub const DEVICES: &str = "/proc/bus/input/devices";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Front {
    pub capabilities: Option<BTreeSet<String>>,
    pub touchscreen: Option<bool>,
}

impl Front {
    pub fn of(said: &str, devices: &str) -> Result<Self, Never> {
        let capabilities = capabilities(said)?;

        let touchscreen = touchscreen(devices)?;

        Ok(Front { capabilities, touchscreen })
    }

    pub fn can_send(&self, button: &str) -> Result<Has, Never> {
        let has = match &self.capabilities {
            Some(has) => has,
            None => return Ok(Has::Yes),
        };

        let capability = crate::vocabulary::capability_of(button)?;

        Ok(match has.contains(&capability) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    pub fn missing<'a>(&self, buttons: &[&'a str]) -> Result<Vec<&'a str>, Never> {
        match self.capabilities.is_some() {
            true => {},
            false => return Ok(Vec::new()),
        }

        let mut missing = Vec::new();

        for button in buttons.iter().copied() {
            let has = self.can_send(button)?;

            match has {
                Has::No => missing.push(button),
                Has::Yes => {},
            }
        }

        Ok(missing)
    }

    pub fn spare(&self, bound: &[&str]) -> Result<Vec<String>, Never> {
        let has = match &self.capabilities {
            Some(has) => has,
            None => return Ok(Vec::new()),
        };

        let mut taken: BTreeSet<String> = BTreeSet::new();

        for button in bound {
            let capability = crate::vocabulary::capability_of(button)?;

            taken.insert(capability);
        }

        Ok(has
            .iter()
            .filter(|said| said.starts_with(crate::vocabulary::BUTTON))
            .filter(|said| !taken.contains(said.as_str()))
            .cloned()
            .collect())
    }
}

pub fn capabilities(said: &str) -> Result<Option<BTreeSet<String>>, Never> {
    let found: BTreeSet<String> = said
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|said| !said.trim().is_empty())
        .map(str::to_string)
        .collect();

    Ok(match found.is_empty() {
        true => None,
        false => Some(found),
    })
}

pub fn touchscreen(devices: &str) -> Result<Option<bool>, Never> {
    match devices.trim().is_empty() {
        true => return Ok(None),
        false => {},
    }

    let mut unreadable = false;

    for line in devices.lines() {
        let properties = properties(line)?;

        match properties {
            Properties::Bits(bits) if bits & DIRECT != 0 => return Ok(Some(true)),
            Properties::Unreadable => unreadable = true,
            Properties::Bits(_) | Properties::Elsewhere => {},
        }
    }

    Ok(match unreadable {
        true => None,
        false => Some(false),
    })
}

const DIRECT: u64 = 1 << 1;

enum Properties {
    Elsewhere,
    Unreadable,
    Bits(u64),
}

fn properties(line: &str) -> Result<Properties, Never> {
    let hex = match line.strip_prefix("B: PROP=") {
        Some(hex) => hex,
        None => return Ok(Properties::Elsewhere),
    };

    Ok(match u64::from_str_radix(hex.trim(), 16) {
        Ok(bits) => Properties::Bits(bits),
        Err(_) => Properties::Unreadable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    const SAID: &str = r#"as 41 "Gamepad:Trigger:RightTouchpadForce" "Gyroscope:Center" "Gamepad:Button:DPadLeft" "Gamepad:Button:LeftPaddle1" "Gamepad:Button:QuickAccess" "Gamepad:Button:South" "Gamepad:Axis:LeftStick" "Gamepad:Button:RightPaddle3""#;

    const LISTED: &str = "\
N: Name=\"NVTK0603:00 0603:F001\"
P: Phys=i2c-NVTK0603:00
H: Handlers=event8 mouse2
B: PROP=2
B: EV=1b
B: ABS=673800001000003

N: Name=\"  Legion Controller  Touchpad\"
P: Phys=usb-0000:c2:00.3-3/input1
H: Handlers=event7 mouse1
B: PROP=0
B: EV=1b
B: ABS=10000000003
";

    #[test]
    fn what_the_machine_said_is_read_as_what_it_has() {
        let front = ok(Front::of(SAID, LISTED));
        let has = front.capabilities.expect("it answered");
        assert!(has.contains("Gamepad:Button:LeftPaddle1"));
        assert!(has.contains("Gamepad:Axis:LeftStick"));
        assert_eq!(has.len(), 8);
    }

    #[test]
    fn a_button_this_machine_cannot_send_is_the_one_that_comes_back() {
        let front = ok(Front::of(SAID, LISTED));
        assert_eq!(ok(front.missing(&["South", "RightPaddle1"])), ["RightPaddle1"]);
        assert_eq!(front.can_send("South"), Ok(Has::Yes));
        assert_eq!(front.can_send("RightPaddle1"), Ok(Has::No));
    }

    #[test]
    fn a_machine_that_could_not_be_asked_is_missing_nothing() {
        let quiet = ok(Front::of("", ""));
        assert_eq!(quiet.capabilities, None);
        assert_eq!(quiet.touchscreen, None);
        assert!(ok(quiet.missing(&["South"])).is_empty());
        assert!(ok(quiet.spare(&[])).is_empty());
        assert_eq!(
            quiet.can_send("RightPaddle1"),
            Ok(Has::Yes),
            "a machine that said nothing has every button"
        );
    }

    #[test]
    fn a_button_nothing_is_bound_to_is_one_the_setup_screen_can_offer() {
        let front = ok(Front::of(SAID, LISTED));
        let spare = ok(front.spare(&["South"]));
        assert!(spare.contains(&"Gamepad:Button:RightPaddle3".to_string()), "{spare:?}");
        assert!(!spare.contains(&"Gamepad:Button:South".to_string()), "{spare:?}");
        assert!(spare.iter().all(|said| said.starts_with("Gamepad:Button:")), "{spare:?}");
    }

    #[test]
    fn a_screen_you_can_touch_is_told_from_a_touchpad_by_the_one_bit() {
        assert_eq!(touchscreen(LISTED), Ok(Some(true)));
        let no_screen: String =
            LISTED.lines().map(|line| line.replace("PROP=2", "PROP=0")).collect::<Vec<_>>().join("\n");
        assert_eq!(touchscreen(&no_screen), Ok(Some(false)));
    }

    #[test]
    fn a_kernel_that_said_nothing_is_not_a_machine_without_a_screen() {
        assert_eq!(touchscreen(""), Ok(None));
    }

    #[test]
    fn the_profile_the_pad_is_wearing_is_read_out_of_what_the_bus_said() {
        assert_eq!(
            one_said("s \"/etc/inputplumber/profiles/desktop.yaml\""),
            Ok(Some("/etc/inputplumber/profiles/desktop.yaml".to_string()))
        );
        assert_eq!(one_said(""), Ok(None));
        assert_eq!(one_said("s \"\""), Ok(None));
    }

    #[test]
    fn a_profile_is_read_again_by_the_path_it_is_at() {
        let asked = ok(loading("/etc/inputplumber/profiles/tabs.yaml"));
        assert_eq!(asked.first().map(String::as_str), Some("busctl"));
        assert!(asked.contains(&"LoadProfilePath".to_string()));
        assert_eq!(asked.last().map(String::as_str), Some("/etc/inputplumber/profiles/tabs.yaml"));
    }

    #[test]
    fn the_question_is_the_one_the_daemon_answers() {
        let asking = ok(asking());
        assert_eq!(asking[0], "busctl");
        assert!(asking.contains(&"--system"));
        assert!(asking.contains(&"Capabilities"));
    }
}
