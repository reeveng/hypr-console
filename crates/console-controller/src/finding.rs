//! Which device is which, decided by what each one says about itself.
//!
//! All three of these are asked of a list rather than of the machine, so the
//! rules can be held to a capture of the real devices without a device in the
//! room.

use evdev::{AbsoluteAxisCode, Device, KeyCode};

/// What a device says about itself, which is all these rules have to go on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Says {
    pub path: String,
    pub name: String,
    /// Where it is plugged in. A device made through uinput has none.
    pub phys: String,
    pub keys: Vec<u16>,
    pub axes: Vec<u16>,
}

impl Says {
    fn has_key(&self, key: KeyCode) -> bool {
        self.keys.contains(&key.0)
    }

    fn has_axis(&self, axis: AbsoluteAxisCode) -> bool {
        self.axes.contains(&axis.0)
    }

    /// Whether this is one of the devices InputPlumber publishes rather than
    /// one somebody is holding.
    fn made(&self) -> bool {
        self.phys.is_empty()
    }
}

/// What one device says about itself, in the words these rules are written in.
///
/// The one place a real device is turned into something the rules can be asked
/// about, so a program that wants to know which pad is which does not have to
/// know how to ask a device what it is. Everything below this line is asked of
/// a list and never of the machine.
pub fn says(path: &str, device: &Device) -> Says {
    Says {
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
    }
}

/// The virtual pad InputPlumber publishes, not the physical controller.
///
/// Both carry a right stick and both call themselves an Xbox pad, so the name
/// settles nothing. The physical one is grabbed by InputPlumber and would
/// report nothing for as long as we held it.
///
/// A device made through uinput has no physical location, and a real one does.
/// That is the difference, and unlike a name it cannot be shared.
pub fn gamepad(among: &[Says]) -> Option<&Says> {
    among.iter().find(|says| {
        says.has_axis(AbsoluteAxisCode::ABS_RX)
            && says.has_axis(AbsoluteAxisCode::ABS_RY)
            && says.made()
    })
}

/// The keyboard InputPlumber publishes, where the back buttons arrive.
pub fn keyboard(among: &[Says]) -> Option<&Says> {
    among.iter().find(|says| {
        says.has_key(KeyCode::KEY_F13) && says.has_key(KeyCode::KEY_ESC) && says.made()
    })
}

/// The controller's touchpad, which nothing else is reading.
///
/// This one has a physical location, because InputPlumber never touches it and
/// what is read is the real device.
pub fn touchpad(among: &[Says]) -> Option<&Says> {
    among.iter().find(|says| {
        says.has_key(KeyCode::BTN_TOUCH)
            && says.has_axis(AbsoluteAxisCode::ABS_X)
            && says.name.to_lowercase().contains("touchpad")
    })
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

    /// The one that matters. Both pads carry a right stick and both call
    /// themselves an Xbox pad; only one of them has nowhere to be plugged in.
    #[test]
    fn the_pad_that_is_read_is_the_one_nobody_is_holding() {
        let both = [pad("usb-0000:c2:00.3-3/input0"), pad("")];
        assert_eq!(gamepad(&both).map(|says| says.phys.as_str()), Some(""));
    }

    #[test]
    fn a_physical_pad_on_its_own_is_not_the_one() {
        assert_eq!(gamepad(&[pad("usb-0000:c2:00.3-3/input0")]), None);
    }

    #[test]
    fn the_keyboard_is_the_one_the_back_buttons_arrive_on() {
        let every = [pad(""), keys(), touch()];
        assert_eq!(keyboard(&every).map(|says| says.name.as_str()), Some("InputPlumber Keyboard"));
    }

    /// The touchpad is the one device here that is read as itself, so it has a
    /// physical location and is found by what it is rather than by having none.
    #[test]
    fn the_touchpad_is_found_although_somebody_is_holding_it() {
        let every = [pad(""), keys(), touch()];
        assert!(touchpad(&every).is_some());
    }

    #[test]
    fn a_touchscreen_is_not_the_touchpad() {
        let screen = Says { name: "Legion Controller Touchscreen".into(), ..touch() };
        assert_eq!(touchpad(&[screen]), None);
    }

    #[test]
    fn nothing_at_all_is_nothing_rather_than_a_guess() {
        assert_eq!(gamepad(&[]), None);
        assert_eq!(keyboard(&[]), None);
        assert_eq!(touchpad(&[]), None);
    }
}
