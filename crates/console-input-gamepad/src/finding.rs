//! Which device is which, decided by what each one says about itself.
//!
//! All three of these are asked of a list rather than of the machine, so the
//! rules can be held to a capture of the real devices without a device in the
//! room.
//!
//! What a device InputPlumber made says about itself is in `targets.rs`, and it
//! is two numbers rather than the absence of a physical path: an empty path is
//! every uinput device on the machine, someone else's virtual pad included.


use crate::devices::Has;
use crate::targets::{Identity, Target};
use console_core_never::Never;
use console_input_event_devices::{AbsoluteAxisCode, Device, KeyCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Made {
    Virtual,
    Plugged,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub path: String,
    pub name: String,
    pub phys: String,
    pub vendor: u16,
    pub product: u16,
    pub keys: Vec<u16>,
    pub axes: Vec<u16>,
}

impl DeviceInfo {
    fn has_key(&self, key: KeyCode) -> Result<Has, Never> {
        Ok(match self.keys.contains(&key.0) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    fn has_axis(&self, axis: AbsoluteAxisCode) -> Result<Has, Never> {
        Ok(match self.axes.contains(&axis.0) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    fn made(&self) -> Result<Made, Never> {
        Ok(match self.phys.is_empty() {
            true => Made::Virtual,
            false => Made::Plugged,
        })
    }

    fn wearing(&self, identity: Identity) -> Result<Has, Never> {
        Ok(match self.vendor == identity.vendor && self.product == identity.product {
            true => Has::Yes,
            false => Has::No,
        })
    }
}

pub const UNNAMED: &str = "";

pub fn named(device: &Device) -> Result<String, Never> {
    Ok(match &device.name {
        Some(name) => name.clone(),
        None => UNNAMED.to_string(),
    })
}

pub fn wired(device: &Device) -> Result<String, Never> {
    Ok(match &device.physical_path {
        Some(phys) => phys.clone(),
        None => UNNAMED.to_string(),
    })
}

pub fn describe(path: &str, device: &Device) -> Result<DeviceInfo, Never> {
    let Ok(name) = named(device);
    let Ok(phys) = wired(device);

    let id = device.id;
    let keys = device.keys.iter().map(|key| key.0).collect();
    let axes = device.absolute_axes.iter().map(|axis| axis.0).collect();

    Ok(DeviceInfo {
        path: path.to_string(),
        name,
        phys,
        vendor: id.vendor,
        product: id.product,
        keys,
        axes,
    })
}

pub fn gamepad(among: &[DeviceInfo]) -> Result<Option<&DeviceInfo>, Never> {
    let Ok(identity) = Target::Pad.identity();

    for says in among {
        let across = says.has_axis(AbsoluteAxisCode::ABS_RX)?;
        let down = says.has_axis(AbsoluteAxisCode::ABS_RY)?;
        let sticks = across == Has::Yes && down == Has::Yes;

        let made = says.made()?;
        let wearing = says.wearing(identity)?;

        match sticks && made == Made::Virtual && wearing == Has::Yes {
            true => return Ok(Some(says)),
            false => {},
        }
    }

    Ok(None)
}

pub fn keyboard(among: &[DeviceInfo]) -> Result<Option<&DeviceInfo>, Never> {
    let Ok(identity) = Target::Keyboard.identity();

    for says in among {
        let paddle = says.has_key(KeyCode::KEY_F13)?;
        let escape = says.has_key(KeyCode::KEY_ESC)?;
        let keys = paddle == Has::Yes && escape == Has::Yes;

        let made = says.made()?;
        let wearing = says.wearing(identity)?;

        match keys && made == Made::Virtual && wearing == Has::Yes {
            true => return Ok(Some(says)),
            false => {},
        }
    }

    Ok(None)
}

pub fn typing(among: &[DeviceInfo]) -> Result<Vec<&DeviceInfo>, Never> {
    let mut found = Vec::new();

    for says in among {
        let first = says.has_key(KeyCode::KEY_A)?;
        let last = says.has_key(KeyCode::KEY_Z)?;
        let escape = says.has_key(KeyCode::KEY_ESC)?;
        let letters = first == Has::Yes && last == Has::Yes && escape == Has::Yes;

        let made = says.made()?;

        match letters && made == Made::Plugged {
            true => found.push(says),
            false => {},
        }
    }

    Ok(found)
}

pub fn touchpad(among: &[DeviceInfo]) -> Result<Option<&DeviceInfo>, Never> {
    for says in among {
        let touched = says.has_key(KeyCode::BTN_TOUCH)?;
        let across = says.has_axis(AbsoluteAxisCode::ABS_X)?;
        let touch = touched == Has::Yes && across == Has::Yes;

        match touch && says.name.to_lowercase().contains("touchpad") {
            true => return Ok(Some(says)),
            false => {},
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(phys: &str) -> DeviceInfo {
        let Ok(identity) = Target::Pad.identity();

        DeviceInfo {
            path: "/dev/input/event0".into(),
            name: "Microsoft X-Box One Elite 2 pad".into(),
            phys: phys.into(),
            vendor: identity.vendor,
            product: identity.product,
            keys: vec![KeyCode::BTN_SOUTH.0],
            axes: vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0],
        }
    }

    fn steams_pad() -> DeviceInfo {
        DeviceInfo {
            path: "/dev/input/event17".into(),
            name: "Microsoft X-Box 360 pad 0".into(),
            phys: String::new(),
            vendor: 0x28de,
            product: 0x11ff,
            keys: vec![KeyCode::BTN_SOUTH.0],
            axes: vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0],
        }
    }

    fn keys() -> DeviceInfo {
        let Ok(identity) = Target::Keyboard.identity();

        DeviceInfo {
            path: "/dev/input/event1".into(),
            name: "InputPlumber Keyboard".into(),
            phys: String::new(),
            vendor: identity.vendor,
            product: identity.product,
            keys: vec![KeyCode::KEY_F13.0, KeyCode::KEY_ESC.0],
            axes: vec![],
        }
    }

    fn touch() -> DeviceInfo {
        DeviceInfo {
            path: "/dev/input/event2".into(),
            name: "  Legion Controller  Touchpad".into(),
            phys: "usb-0000:c2:00.3-3/input1".into(),
            vendor: 0x17ef,
            product: 0x61eb,
            keys: vec![KeyCode::BTN_TOUCH.0],
            axes: vec![AbsoluteAxisCode::ABS_X.0, AbsoluteAxisCode::ABS_Y.0],
        }
    }

    #[test]
    fn the_pad_that_is_read_is_the_one_no_one_is_holding() {
        let both = [pad("usb-0000:c2:00.3-3/input0"), pad("")];
        let Ok(found) = gamepad(&both);

        assert_eq!(found.map(|says| says.phys.as_str()), Some(""));
    }

    #[test]
    fn a_physical_pad_on_its_own_is_not_the_one() {
        assert_eq!(gamepad(&[pad("usb-0000:c2:00.3-3/input0")]), Ok(None));
    }

    #[test]
    fn a_pad_steam_published_is_not_the_one_the_profile_asked_for() {
        let both = [steams_pad(), pad("")];
        let Ok(found) = gamepad(&both);

        assert_eq!(
            found.map(|says| says.path.as_str()),
            Some("/dev/input/event0"),
            "the pad InputPlumber made is the one this desktop reads, whatever else \
             is publishing a gamepad beside it"
        );
    }

    #[test]
    fn steams_pad_on_its_own_is_nothing_to_read() {
        assert_eq!(
            gamepad(&[steams_pad()]),
            Ok(None),
            "a machine whose only virtual pad is someone else's has no pad of ours on it, \
             and saying so is what stops the daemon reading a device nothing emits on"
        );
    }

    #[test]
    fn the_keyboard_is_the_one_the_back_buttons_arrive_on() {
        let every = [pad(""), keys(), touch()];
        let Ok(found) = keyboard(&every);

        assert_eq!(found.map(|says| says.name.as_str()), Some("InputPlumber Keyboard"));
    }

    #[test]
    fn the_touchpad_is_found_although_someone_is_holding_it() {
        let every = [pad(""), keys(), touch()];
        let Ok(found) = touchpad(&every);

        assert!(found.is_some());
    }

    #[test]
    fn a_touchscreen_is_not_the_touchpad() {
        let screen = DeviceInfo { name: "Legion Controller Touchscreen".into(), ..touch() };
        assert_eq!(touchpad(&[screen]), Ok(None));
    }

    fn typed(name: &str) -> DeviceInfo {
        DeviceInfo {
            path: "/dev/input/event3".into(),
            name: name.into(),
            phys: "usb-0000:c2:00.3-4/input0".into(),
            vendor: 0x046d,
            product: 0xb342,
            keys: vec![KeyCode::KEY_A.0, KeyCode::KEY_Z.0, KeyCode::KEY_ESC.0],
            axes: vec![],
        }
    }

    #[test]
    fn a_keyboard_someone_plugged_in_is_the_one_with_letters_on_it() {
        let every = [pad(""), keys(), touch(), typed("Logitech K380")];
        let Ok(found) = typing(&every);

        assert_eq!(
            found.iter().map(|says| says.name.as_str()).collect::<Vec<&str>>(),
            vec!["Logitech K380"]
        );
    }

    #[test]
    fn every_keyboard_someone_plugged_in_is_one_of_them() {
        let every = [typed("Logitech K380"), typed("Some Other Board")];
        let Ok(found) = typing(&every);

        assert_eq!(found.len(), 2, "a second keyboard is a second keyboard");
    }

    #[test]
    fn the_rocker_on_the_edge_of_the_machine_is_not_a_keyboard_to_type_on() {
        let rocker = DeviceInfo {
            path: "/dev/input/event4".into(),
            name: "Legion Go Volume".into(),
            phys: "isa0060/serio0/input0".into(),
            vendor: 0x17ef,
            product: 0x6182,
            keys: vec![KeyCode::KEY_VOLUMEUP.0, KeyCode::KEY_VOLUMEDOWN.0],
            axes: vec![],
        };

        assert_eq!(typing(&[rocker]), Ok(Vec::new()));
    }

    #[test]
    fn what_inputplumber_publishes_is_not_something_someone_types_on() {
        let ours = DeviceInfo {
            keys: vec![KeyCode::KEY_A.0, KeyCode::KEY_Z.0, KeyCode::KEY_ESC.0],
            ..keys()
        };

        assert_eq!(typing(&[ours]), Ok(Vec::new()));
    }

    #[test]
    fn nothing_at_all_is_nothing_rather_than_a_guess() {
        assert_eq!(gamepad(&[]), Ok(None));
        assert_eq!(keyboard(&[]), Ok(None));
        assert_eq!(touchpad(&[]), Ok(None));
        assert_eq!(typing(&[]), Ok(Vec::new()));
    }
}
