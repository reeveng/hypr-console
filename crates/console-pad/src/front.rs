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


use crate::devices::Has;
use std::collections::BTreeSet;


/// Where the composite device is, and what it is asked.
pub const BUS: &str = "org.shadowblip.InputPlumber";
pub const OBJECT: &str = "/org/shadowblip/InputPlumber/CompositeDevice0";
pub const INTERFACE: &str = "org.shadowblip.Input.CompositeDevice";

/// The whole question, as a command.
///
/// Read as anybody: this is asked during a check, which is not root, as well
/// as during an apply, which is.
pub fn asking() -> Vec<&'static str> {
    vec!["busctl", "--system", "get-property", BUS, OBJECT, INTERFACE, "Capabilities"]
}

/// The file the pad is wearing, as a command.
pub fn wearing() -> Vec<&'static str> {
    vec!["busctl", "--system", "get-property", BUS, OBJECT, INTERFACE, "ProfilePath"]
}

/// Ask for a profile to be read again, as a command.
///
/// By the path it is at rather than by a word, because this is asked when the
/// file has changed underneath a profile that is already loaded, and the
/// daemon that watches which profile is on would see nothing to do: the name
/// is the same name.
pub fn loading(path: &str) -> Vec<String> {
    ["busctl", "--system", "call", BUS, OBJECT, INTERFACE, "LoadProfilePath", "s", path]
        .iter()
        .map(|word| (*word).to_string())
        .collect()
}

/// The one string out of a `busctl get-property` that answered with one.
pub fn one_said(said: &str) -> Option<String> {
    said.split('"').nth(1).filter(|said| !said.is_empty()).map(str::to_string)
}

/// Where the kernel lists what is plugged in, in the one format that says
/// whether a touch device is a screen or a pad.
pub const DEVICES: &str = "/proc/bus/input/devices";

/// What a machine says about the front of itself.
///
/// Both halves are an `Option` for the same reason: not knowing is a third
/// answer, and it is the honest one when InputPlumber is not running or has
/// not finished enumerating. Reported as unknown rather than as missing,
/// because a check that cries about every button on a machine it could not ask
/// is a check somebody turns off.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Front {
    pub capabilities: Option<BTreeSet<String>>,
    /// Whether anything here is a screen you can touch.
    pub touchscreen: Option<bool>,
}

impl Front {
    /// Out of what the machine said: the bus property, and the kernel's list.
    pub fn of(said: &str, devices: &str) -> Self {
        Front { capabilities: capabilities(said), touchscreen: touchscreen(devices) }
    }

    /// Whether this machine can send that button, said the way the machine
    /// says it.
    ///
    /// A machine that could not be asked can send anything. Nothing at all is
    /// not an empty machine, and reading silence as "it has no buttons" is a
    /// desktop that tells somebody holding a working handheld that none of
    /// their buttons exist.
    pub fn can_send(&self, button: &str) -> Has {
        match &self.capabilities {
            Some(has) => match has.contains(&crate::vocabulary::capability_of(button)) {
                true => Has::Yes,
                false => Has::No,
            },
            None => Has::Yes,
        }
    }

    /// What this desktop asks for that this machine cannot send.
    ///
    /// Given in the profiles' own names for buttons. Nothing at all when the
    /// machine could not be asked: the caller says that separately, because a
    /// list of everything is not the same claim as a list.
    pub fn missing<'a>(&self, buttons: &[&'a str]) -> Vec<&'a str> {
        match self.capabilities.is_some() {
            true => buttons.iter().copied().filter(|button| self.can_send(button) == Has::No).collect(),
            false => Vec::new(),
        }
    }

    /// What this machine can send that nothing is bound to.
    ///
    /// The other direction, and the one the setup screen is made of: a device
    /// with a button going spare is a device something can be moved onto.
    pub fn spare(&self, bound: &[&str]) -> Vec<String> {
        let Some(has) = &self.capabilities else { return Vec::new() };

        let taken: BTreeSet<String> =
            bound.iter().map(|button| crate::vocabulary::capability_of(button)).collect();
        has.iter()
            .filter(|said| said.starts_with(crate::vocabulary::BUTTON))
            .filter(|said| !taken.contains(said.as_str()))
            .cloned()
            .collect()
    }
}

/// The strings out of `busctl get-property`, which are the only quoted things
/// in what it prints.
///
/// Nothing at all is not an empty machine: it is a machine that did not
/// answer, and `busctl` prints nothing when the property, the object or the
/// daemon is missing.
pub fn capabilities(said: &str) -> Option<BTreeSet<String>> {
    let found: BTreeSet<String> = said
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|said| !said.trim().is_empty())
        .map(str::to_string)
        .collect();

    match found.is_empty() {
        true => None,
        false => Some(found),
    }
}

/// Whether the kernel's list holds a screen somebody can touch.
///
/// The bit is the whole answer. `INPUT_PROP_DIRECT` means the thing you touch
/// is the thing you are looking at, which is what tells this device's
/// touchscreen from the touchpad on the back of its own controller: the screen
/// says `PROP=2` and the pad says `PROP=0`, and both otherwise look alike --
/// same `BTN_TOUCH`, same absolute axes, and a name that promises nothing.
pub fn touchscreen(devices: &str) -> Option<bool> {
    if devices.trim().is_empty() {
        return None;
    }

    let mut unreadable = false;

    for line in devices.lines() {
        match properties(line) {
            Properties::Bits(bits) if bits & DIRECT != 0 => return Some(true),
            Properties::Unreadable => unreadable = true,
            Properties::Bits(_) | Properties::Elsewhere => {},
        }
    }

    // A line this could not read may have been the screen's. Unknown is the
    // honest answer to that, and it is not the same answer as no screen: a
    // kernel that writes something other than hex there would otherwise turn
    // the touchscreen into a touchpad, silently, which is the one mistake
    // this whole module exists to avoid.
    match unreadable {
        true => None,
        false => Some(false),
    }
}

/// `INPUT_PROP_DIRECT`, as the kernel writes it in that file.
const DIRECT: u64 = 1 << 1;

/// What one line of the kernel's list says about a device's properties.
///
/// Three answers rather than two. A line that is not the properties line is
/// not an answer at all, and a properties line written in something other
/// than hex is an answer that cannot be read -- which is a different fact
/// from a device having no properties, and has to stay different.
enum Properties {
    /// Some other line of the block.
    Elsewhere,
    /// The properties line, in something this cannot read.
    Unreadable,
    /// The bits the kernel wrote.
    Bits(u64),
}

fn properties(line: &str) -> Properties {
    let Some(hex) = line.strip_prefix("B: PROP=") else { return Properties::Elsewhere };

    match u64::from_str_radix(hex.trim(), 16) {
        Ok(bits) => Properties::Bits(bits),
        Err(_) => Properties::Unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// What this machine really said, cut down to the buttons the rules are
    /// about. Written from a `busctl get-property` on the device rather than
    /// invented, so the parsing is held to the format that exists.
    const SAID: &str = r#"as 41 "Gamepad:Trigger:RightTouchpadForce" "Gyroscope:Center" "Gamepad:Button:DPadLeft" "Gamepad:Button:LeftPaddle1" "Gamepad:Button:QuickAccess" "Gamepad:Button:South" "Gamepad:Axis:LeftStick" "Gamepad:Button:RightPaddle3""#;

    /// Two devices as the kernel lists them, off this machine: the screen, and
    /// the touchpad on the back of the controller.
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
        let front = Front::of(SAID, LISTED);
        let has = front.capabilities.expect("it answered");
        assert!(has.contains("Gamepad:Button:LeftPaddle1"));
        assert!(has.contains("Gamepad:Axis:LeftStick"));
        assert_eq!(has.len(), 8);
    }

    /// The one the whole module is for.
    #[test]
    fn a_button_this_machine_cannot_send_is_the_one_that_comes_back() {
        let front = Front::of(SAID, LISTED);
        assert_eq!(front.missing(&["South", "RightPaddle1"]), ["RightPaddle1"]);
        assert_eq!(front.can_send("South"), Has::Yes);
        assert_eq!(front.can_send("RightPaddle1"), Has::No);
    }

    /// A machine that did not answer is not a machine with nothing on it.
    /// Saying every button is missing because InputPlumber was still starting
    /// is the notice that teaches somebody to ignore notices.
    #[test]
    fn a_machine_that_could_not_be_asked_is_missing_nothing() {
        let quiet = Front::of("", "");
        assert_eq!(quiet.capabilities, None);
        assert_eq!(quiet.touchscreen, None);
        assert!(quiet.missing(&["South"]).is_empty());
        assert!(quiet.spare(&[]).is_empty());
        assert_eq!(
            quiet.can_send("RightPaddle1"),
            Has::Yes,
            "a machine that said nothing has every button"
        );
    }

    #[test]
    fn a_button_nothing_is_bound_to_is_one_the_setup_screen_can_offer() {
        let front = Front::of(SAID, LISTED);
        let spare = front.spare(&["South"]);
        assert!(spare.contains(&"Gamepad:Button:RightPaddle3".to_string()), "{spare:?}");
        assert!(!spare.contains(&"Gamepad:Button:South".to_string()), "{spare:?}");
        // Only buttons. A stick is not a thing a role can be moved onto by
        // pressing it.
        assert!(spare.iter().all(|said| said.starts_with("Gamepad:Button:")), "{spare:?}");
    }

    /// The bit that tells the screen from the pad on the back of the pad.
    #[test]
    fn a_screen_you_can_touch_is_told_from_a_touchpad_by_the_one_bit() {
        assert_eq!(touchscreen(LISTED), Some(true));
        let no_screen: String =
            LISTED.lines().map(|line| line.replace("PROP=2", "PROP=0")).collect::<Vec<_>>().join("\n");
        assert_eq!(touchscreen(&no_screen), Some(false));
    }

    #[test]
    fn a_kernel_that_said_nothing_is_not_a_machine_without_a_screen() {
        assert_eq!(touchscreen(""), None);
    }

    #[test]
    fn the_profile_the_pad_is_wearing_is_read_out_of_what_the_bus_said() {
        assert_eq!(
            one_said("s \"/etc/inputplumber/profiles/desktop.yaml\""),
            Some("/etc/inputplumber/profiles/desktop.yaml".to_string())
        );
        assert_eq!(one_said(""), None);
        assert_eq!(one_said("s \"\""), None);
    }

    /// A file that has changed under a profile already loaded is asked for by
    /// its path: the name has not changed, so nothing watching names would see
    /// anything to do.
    #[test]
    fn a_profile_is_read_again_by_the_path_it_is_at() {
        let asked = loading("/etc/inputplumber/profiles/tabs.yaml");
        assert_eq!(asked.first().map(String::as_str), Some("busctl"));
        assert!(asked.contains(&"LoadProfilePath".to_string()));
        assert_eq!(asked.last().map(String::as_str), Some("/etc/inputplumber/profiles/tabs.yaml"));
    }

    /// The question is asked of the system bus, which is where InputPlumber
    /// is, and read as whoever is asking.
    #[test]
    fn the_question_is_the_one_the_daemon_answers() {
        let asking = asking();
        assert_eq!(asking[0], "busctl");
        assert!(asking.contains(&"--system"));
        assert!(asking.contains(&"Capabilities"));
    }
}
