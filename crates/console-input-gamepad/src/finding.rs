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

    fn origin(&self) -> Result<Made, Never> {
        Ok(match self.phys.is_empty() {
            true => Made::Virtual,
            false => Made::Plugged,
        })
    }

    fn matches(&self, identity: Identity) -> Result<Has, Never> {
        Ok(match self.vendor == identity.vendor && self.product == identity.product {
            true => Has::Yes,
            false => Has::No,
        })
    }
}

pub const UNNAMED: &str = "";

pub fn device_name(device: &Device) -> Result<String, Never> {
    Ok(match &device.name {
        Some(name) => name.clone(),
        None => UNNAMED.to_string(),
    })
}

pub fn physical_path(device: &Device) -> Result<String, Never> {
    Ok(match &device.physical_path {
        Some(phys) => phys.clone(),
        None => UNNAMED.to_string(),
    })
}

pub fn describe(path: &str, device: &Device) -> Result<DeviceInfo, Never> {
    let Ok(name) = device_name(device);
    let Ok(phys) = physical_path(device);

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

        let made = says.origin()?;
        let wearing = says.matches(identity)?;

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

        let made = says.origin()?;
        let wearing = says.matches(identity)?;

        match keys && made == Made::Virtual && wearing == Has::Yes {
            true => return Ok(Some(says)),
            false => {},
        }
    }

    Ok(None)
}

pub fn keyboards(among: &[DeviceInfo]) -> Result<Vec<&DeviceInfo>, Never> {
    let mut found = Vec::new();

    for says in among {
        let first = says.has_key(KeyCode::KEY_A)?;
        let last = says.has_key(KeyCode::KEY_Z)?;
        let escape = says.has_key(KeyCode::KEY_ESC)?;
        let letters = first == Has::Yes && last == Has::Yes && escape == Has::Yes;

        let made = says.origin()?;

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

    fn pad(phys: &str) -> Result<DeviceInfo, Never> {
        let Ok(identity) = Target::Pad.identity();

        Ok(DeviceInfo {
            path: "/dev/input/event0".to_string(),
            name: "Microsoft X-Box One Elite 2 pad".to_string(),
            phys: phys.to_string(),
            vendor: identity.vendor,
            product: identity.product,
            keys: vec![KeyCode::BTN_SOUTH.0],
            axes: vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0],
        })
    }

    fn steams_pad() -> Result<DeviceInfo, Never> {
        Ok(DeviceInfo {
            path: "/dev/input/event17".to_string(),
            name: "Microsoft X-Box 360 pad 0".to_string(),
            phys: String::new(),
            vendor: 0x28de,
            product: 0x11ff,
            keys: vec![KeyCode::BTN_SOUTH.0],
            axes: vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0],
        })
    }

    fn keys() -> Result<DeviceInfo, Never> {
        let Ok(identity) = Target::Keyboard.identity();

        Ok(DeviceInfo {
            path: "/dev/input/event1".to_string(),
            name: "InputPlumber Keyboard".to_string(),
            phys: String::new(),
            vendor: identity.vendor,
            product: identity.product,
            keys: vec![KeyCode::KEY_F13.0, KeyCode::KEY_ESC.0],
            axes: vec![],
        })
    }

    fn touch() -> Result<DeviceInfo, Never> {
        Ok(DeviceInfo {
            path: "/dev/input/event2".to_string(),
            name: "  Legion Controller  Touchpad".to_string(),
            phys: "usb-0000:c2:00.3-3/input1".to_string(),
            vendor: 0x17ef,
            product: 0x61eb,
            keys: vec![KeyCode::BTN_TOUCH.0],
            axes: vec![AbsoluteAxisCode::ABS_X.0, AbsoluteAxisCode::ABS_Y.0],
        })
    }

    fn every() -> Result<Vec<DeviceInfo>, Never> {
        let Ok(pad) = pad("");
        let Ok(keys) = keys();
        let Ok(touch) = touch();

        Ok(vec![pad, keys, touch])
    }

    #[test]
    fn the_pad_that_is_read_is_the_one_no_one_is_holding() {
        let Ok(held) = pad("usb-0000:c2:00.3-3/input0");
        let Ok(free) = pad("");
        let both = [held, free];
        let Ok(found) = gamepad(&both);

        assert_eq!(found.map(|says| says.phys.as_str()), Some(""));
    }

    #[test]
    fn a_physical_pad_on_its_own_is_not_the_one() {
        let Ok(held) = pad("usb-0000:c2:00.3-3/input0");

        assert_eq!(gamepad(&[held]), Ok(None));
    }

    #[test]
    fn a_pad_steam_published_is_not_the_one_the_profile_asked_for() {
        let Ok(steam) = steams_pad();
        let Ok(free) = pad("");
        let both = [steam, free];
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
        let Ok(steam) = steams_pad();

        assert_eq!(
            gamepad(&[steam]),
            Ok(None),
            "a machine whose only virtual pad is someone else's has no pad of ours on it, \
             and saying so is what stops the daemon reading a device nothing emits on"
        );
    }

    #[test]
    fn the_keyboard_is_the_one_the_back_buttons_arrive_on() {
        let Ok(every) = every();
        let Ok(found) = keyboard(&every);

        assert_eq!(found.map(|says| says.name.as_str()), Some("InputPlumber Keyboard"));
    }

    #[test]
    fn the_touchpad_is_found_although_someone_is_holding_it() {
        let Ok(every) = every();
        let Ok(found) = touchpad(&every);

        assert!(found.is_some());
    }

    #[test]
    fn a_touchscreen_is_not_the_touchpad() {
        let Ok(touch) = touch();
        let screen = DeviceInfo { name: "Legion Controller Touchscreen".to_string(), ..touch };

        assert_eq!(touchpad(&[screen]), Ok(None));
    }

    fn typed(name: &str) -> Result<DeviceInfo, Never> {
        Ok(DeviceInfo {
            path: "/dev/input/event3".to_string(),
            name: name.to_string(),
            phys: "usb-0000:c2:00.3-4/input0".to_string(),
            vendor: 0x046d,
            product: 0xb342,
            keys: vec![KeyCode::KEY_A.0, KeyCode::KEY_Z.0, KeyCode::KEY_ESC.0],
            axes: vec![],
        })
    }

    #[test]
    fn a_keyboard_someone_plugged_in_is_the_one_with_letters_on_it() {
        let Ok(mut every) = every();
        let Ok(plugged_in) = typed("Logitech K380");

        every.push(plugged_in);

        let Ok(found) = keyboards(&every);

        assert_eq!(
            found.iter().map(|says| says.name.as_str()).collect::<Vec<&str>>(),
            vec!["Logitech K380"]
        );
    }

    #[test]
    fn every_keyboard_someone_plugged_in_is_one_of_them() {
        let Ok(one) = typed("Logitech K380");
        let Ok(another) = typed("Some Other Board");
        let every = [one, another];
        let Ok(found) = keyboards(&every);

        assert_eq!(found.len(), 2, "a second keyboard is a second keyboard");
    }

    #[test]
    fn the_rocker_on_the_edge_of_the_machine_is_not_a_keyboard_to_type_on() {
        let rocker = DeviceInfo {
            path: "/dev/input/event4".to_string(),
            name: "Legion Go Volume".to_string(),
            phys: "isa0060/serio0/input0".to_string(),
            vendor: 0x17ef,
            product: 0x6182,
            keys: vec![KeyCode::KEY_VOLUMEUP.0, KeyCode::KEY_VOLUMEDOWN.0],
            axes: vec![],
        };

        assert_eq!(keyboards(&[rocker]), Ok(Vec::new()));
    }

    #[test]
    fn what_inputplumber_publishes_is_not_something_someone_types_on() {
        let Ok(keys) = keys();
        let published = DeviceInfo {
            keys: vec![KeyCode::KEY_A.0, KeyCode::KEY_Z.0, KeyCode::KEY_ESC.0],
            ..keys
        };

        assert_eq!(keyboards(&[published]), Ok(Vec::new()));
    }

    #[test]
    fn nothing_at_all_is_nothing_rather_than_a_guess() {
        assert_eq!(gamepad(&[]), Ok(None));
        assert_eq!(keyboard(&[]), Ok(None));
        assert_eq!(touchpad(&[]), Ok(None));
        assert_eq!(keyboards(&[]), Ok(Vec::new()));
    }
}
