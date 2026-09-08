//! Which device is which, decided by what each one says about itself.
//!
//! All three of these are asked of a list rather than of the machine, so the
//! rules can be held to a capture of the real devices without a device in the
//! room.


use crate::devices::Has;
use console_core_never::Never;
use evdev::{AbsoluteAxisCode, Device, KeyCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Made {
    ByInputPlumber,
    ByHand,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Says {
    pub path: String,
    pub name: String,
    pub phys: String,
    pub keys: Vec<u16>,
    pub axes: Vec<u16>,
}

impl Says {
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
            true => Made::ByInputPlumber,
            false => Made::ByHand,
        })
    }
}

pub fn says(path: &str, device: &Device) -> Result<Says, Never> {
    Ok(Says {
        path: path.to_string(),
        name: device.name().unwrap_or_default().to_string(),
        phys: device.physical_path().unwrap_or_default().to_string(),
        keys: device
            .supported_keys()
            .map(|keys| keys.iter().map(|key| key.0).collect())
            .unwrap_or_default(),
        axes: device
            .supported_absolute_axes()
            .map(|axes| axes.iter().map(|axis| axis.0).collect())
            .unwrap_or_default(),
    })
}

pub fn gamepad(among: &[Says]) -> Result<Option<&Says>, Never> {
    for says in among {
        let across = says.has_axis(AbsoluteAxisCode::ABS_RX)?;
        let down = says.has_axis(AbsoluteAxisCode::ABS_RY)?;
        let sticks = across == Has::Yes && down == Has::Yes;

        let made = says.made()?;

        match sticks && made == Made::ByInputPlumber {
            true => return Ok(Some(says)),
            false => {},
        }
    }

    Ok(None)
}

pub fn keyboard(among: &[Says]) -> Result<Option<&Says>, Never> {
    for says in among {
        let paddle = says.has_key(KeyCode::KEY_F13)?;
        let escape = says.has_key(KeyCode::KEY_ESC)?;
        let keys = paddle == Has::Yes && escape == Has::Yes;

        let made = says.made()?;

        match keys && made == Made::ByInputPlumber {
            true => return Ok(Some(says)),
            false => {},
        }
    }

    Ok(None)
}

pub fn typing(among: &[Says]) -> Result<Vec<&Says>, Never> {
    let mut found = Vec::new();

    for says in among {
        let first = says.has_key(KeyCode::KEY_A)?;
        let last = says.has_key(KeyCode::KEY_Z)?;
        let escape = says.has_key(KeyCode::KEY_ESC)?;
        let letters = first == Has::Yes && last == Has::Yes && escape == Has::Yes;

        let made = says.made()?;

        match letters && made == Made::ByHand {
            true => found.push(says),
            false => {},
        }
    }

    Ok(found)
}

pub fn touchpad(among: &[Says]) -> Result<Option<&Says>, Never> {
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

    fn pad(phys: &str) -> Says {
        Says {
            path: "/dev/input/event0".into(),
            name: "Microsoft X-Box One Elite 2 pad".into(),
            phys: phys.into(),
            keys: vec![KeyCode::BTN_SOUTH.0],
            axes: vec![AbsoluteAxisCode::ABS_RX.0, AbsoluteAxisCode::ABS_RY.0],
        }
    }

    fn keys() -> Says {
        Says {
            path: "/dev/input/event1".into(),
            name: "InputPlumber Keyboard".into(),
            phys: String::new(),
            keys: vec![KeyCode::KEY_F13.0, KeyCode::KEY_ESC.0],
            axes: vec![],
        }
    }

    fn touch() -> Says {
        Says {
            path: "/dev/input/event2".into(),
            name: "  Legion Controller  Touchpad".into(),
            phys: "usb-0000:c2:00.3-3/input1".into(),
            keys: vec![KeyCode::BTN_TOUCH.0],
            axes: vec![AbsoluteAxisCode::ABS_X.0, AbsoluteAxisCode::ABS_Y.0],
        }
    }

    #[test]
    fn the_pad_that_is_read_is_the_one_nobody_is_holding() {
        let both = [pad("usb-0000:c2:00.3-3/input0"), pad("")];
        let Ok(found) = gamepad(&both);

        assert_eq!(found.map(|says| says.phys.as_str()), Some(""));
    }

    #[test]
    fn a_physical_pad_on_its_own_is_not_the_one() {
        assert_eq!(gamepad(&[pad("usb-0000:c2:00.3-3/input0")]), Ok(None));
    }

    #[test]
    fn the_keyboard_is_the_one_the_back_buttons_arrive_on() {
        let every = [pad(""), keys(), touch()];
        let Ok(found) = keyboard(&every);

        assert_eq!(found.map(|says| says.name.as_str()), Some("InputPlumber Keyboard"));
    }

    #[test]
    fn the_touchpad_is_found_although_somebody_is_holding_it() {
        let every = [pad(""), keys(), touch()];
        let Ok(found) = touchpad(&every);

        assert!(found.is_some());
    }

    #[test]
    fn a_touchscreen_is_not_the_touchpad() {
        let screen = Says { name: "Legion Controller Touchscreen".into(), ..touch() };
        assert_eq!(touchpad(&[screen]), Ok(None));
    }

    fn typed(name: &str) -> Says {
        Says {
            path: "/dev/input/event3".into(),
            name: name.into(),
            phys: "usb-0000:c2:00.3-4/input0".into(),
            keys: vec![KeyCode::KEY_A.0, KeyCode::KEY_Z.0, KeyCode::KEY_ESC.0],
            axes: vec![],
        }
    }

    #[test]
    fn a_keyboard_somebody_plugged_in_is_the_one_with_letters_on_it() {
        let every = [pad(""), keys(), touch(), typed("Logitech K380")];
        let Ok(found) = typing(&every);

        assert_eq!(
            found.iter().map(|says| says.name.as_str()).collect::<Vec<&str>>(),
            vec!["Logitech K380"]
        );
    }

    #[test]
    fn every_keyboard_somebody_plugged_in_is_one_of_them() {
        let every = [typed("Logitech K380"), typed("Some Other Board")];
        let Ok(found) = typing(&every);

        assert_eq!(found.len(), 2, "a second keyboard is a second keyboard");
    }

    #[test]
    fn the_rocker_on_the_edge_of_the_machine_is_not_a_keyboard_to_type_on() {
        let rocker = Says {
            path: "/dev/input/event4".into(),
            name: "Legion Go Volume".into(),
            phys: "isa0060/serio0/input0".into(),
            keys: vec![KeyCode::KEY_VOLUMEUP.0, KeyCode::KEY_VOLUMEDOWN.0],
            axes: vec![],
        };

        assert_eq!(typing(&[rocker]), Ok(Vec::new()));
    }

    #[test]
    fn what_inputplumber_publishes_is_not_something_somebody_types_on() {
        let ours = Says {
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
