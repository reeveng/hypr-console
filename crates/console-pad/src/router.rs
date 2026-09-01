//! The profile the desktop wears, made out of what this device says it has.
//!
//! It says what every button *is* and nothing about what any of it means. A
//! press arrives at the controller daemon as a thing that can be told apart
//! from every other press, and the daemon -- which can see the screen, the
//! triggers and the person's own answers -- decides what it comes to.
//!
//! There were two of these once, written by hand: one for the desktop and one
//! for while a chooser was up, three hundred lines each and nearly identical.
//! A button meant one thing in one and another in the other, so opening a menu
//! swapped them, and every swap destroyed the pad and built a new one --
//! taking the on-screen keyboard's device and the daemon's with it. Half the
//! comments in this crate are about that fault. There is one profile now, it
//! is worn from login to shutdown, and the difference between the desktop and
//! a chooser is a column in the daemon's own table.
//!
//! Made rather than kept in the tree, for the reason the asking profile is:
//! what it holds is one device's buttons, and the tree is what every machine
//! running this desktop has in common. A handheld with no paddles gets a
//! profile with no paddles in it, and the jobs that were on them say so on the
//! setup screen instead of being bound to something nobody can press.

use std::collections::BTreeSet;

use crate::routing;
use crate::vocabulary::BUTTON;

/// What this desktop calls the profile, and the file it is written to.
pub const NAME: &str = "router";
pub const FILE: &str = "router.yaml";

/// Where the profiles are on the machine, which is where a made one goes.
pub const PROFILES: &str = "/etc/inputplumber/profiles/";

/// How fast the left stick moves the pointer, in pixels a second.
///
/// InputPlumber's own, because the pointer is the one thing here that is
/// better done before the daemon sees it: a pointer moved by a daemon that
/// polls is a pointer that moves in steps, and this one is smooth because
/// nothing in this repository is in its way.
pub const POINTER_PPS: u32 = 900;

/// The sticks and the triggers, which are not buttons and are not routed.
///
/// The right stick is passed through untouched and the daemon turns how far it
/// is pushed into how fast the wheel turns -- a wheel notch does not repeat
/// while held, and arrow keys mean command history in a terminal, so neither
/// of those could do the job. The triggers are passed through because they are
/// the two layers, and how far each is pulled is what makes a chord a chord.
pub const PASSED: [&str; 2] = ["LeftTrigger", "RightTrigger"];

/// The profile, and what it could not route.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Router {
    /// Every button this device has that this desktop has a word for, in the
    /// order the routing table names them.
    pub buttons: Vec<&'static str>,
    /// Buttons the device has that nothing here can route.
    ///
    /// A button no word in `vocabulary` names. It is left out of the profile
    /// and so out of everything: nothing can be bound to it, because nothing
    /// could tell it from any other button it was left out with. Said out loud
    /// rather than dropped, because a device that has one is a device this
    /// repository has not been taught about yet.
    pub without: Vec<String>,
}

impl Router {
    /// What to write, given what the machine said it has.
    pub fn of(capabilities: &BTreeSet<String>) -> Self {
        let mut router = Router::default();
        for capability in capabilities {
            let Some(button) = capability.strip_prefix(BUTTON) else { continue };
            // A trigger reported as a button is still a trigger. It is one of
            // the layers, it is passed through as an axis below, and binding
            // anything to it would be binding a job to holding the machine.
            if button.ends_with("Trigger") || button.ends_with("StickTouch") {
                continue;
            }
            match routing::ROUTE.iter().find(|(named, _)| *named == button) {
                Some((named, _)) => router.buttons.push(named),
                None => router.without.push(button.to_string()),
            }
        }
        router.buttons.sort_unstable_by_key(|button| {
            routing::ROUTE.iter().position(|(named, _)| named == button)
        });
        router
    }

    /// Whether this device can send that button at all.
    pub fn has(&self, button: &str) -> bool {
        self.buttons.contains(&button)
    }

    /// The profile itself.
    ///
    /// All three targets are published, as every profile here does. A profile
    /// that named only the keyboard would destroy the pad and the mouse to say
    /// so, and the pad going out from under the on-screen keyboard is what
    /// crashed the controller daemon once already. `docs/button-contract.md`
    /// is where that rule is kept.
    pub fn yaml(&self) -> String {
        let mut said = String::from(
            "# Written by `console apply`, out of what this device says it can send.\n\
             #\n\
             # Nothing here says what a button means. Every button goes to something the\n\
             # controller daemon can tell from every other button, and what a press comes\n\
             # to -- on the desktop, with a chooser up, with a trigger held -- is decided\n\
             # in one table there and in one file of this machine owner's own. See\n\
             # crates/console-controller/src/means.rs.\n\
             #\n\
             # Not a file to edit: it is made again out of the machine on every apply.\n\
             version: 1\n\
             kind: DeviceProfile\n\
             name: Router\n\
             description: Every button, said as itself, for the daemon to read.\n\
             target_devices:\n  - mouse\n  - keyboard\n  - xbox-elite\n\
             \nmapping:\n",
        );
        // The pointer, which is the one thing left that a profile does rather
        // than says.
        said.push_str(&format!(
            "  - name: Left stick - move the pointer\n\
             \x20   source_event:\n      gamepad:\n        axis:\n          name: LeftStick\n\
             \x20   target_events:\n      - mouse:\n          motion:\n            speed_pps: {POINTER_PPS}\n\n"
        ));
        said.push_str(
            "  - name: Right stick - the wheel, turned by the daemon\n\
             \x20   source_event:\n      gamepad:\n        axis:\n          name: RightStick\n\
             \x20   target_events:\n      - gamepad:\n          axis:\n            name: RightStick\n\n",
        );
        for trigger in PASSED {
            said.push_str(&format!(
                "  - name: {trigger} - a layer, held\n\
                 \x20   source_event:\n      gamepad:\n        trigger:\n\
                 \x20         name: {trigger}\n          deadzone: 0.3\n\
                 \x20   target_events:\n      - gamepad:\n          trigger:\n\
                 \x20         name: {trigger}\n\n"
            ));
        }
        for button in &self.buttons {
            if let Some(mapping) = routing::mapping(button) {
                said.push_str(&mapping);
            }
        }
        said
    }
}

/// The front of the machine this desktop grew on, said the way the machine
/// says it.
///
/// Every button this repository has a word for, which is not a coincidence:
/// the words were written down off this device. It stands in for a real answer
/// wherever there is no machine to ask -- the emulator, the tests, and a
/// checkout on a laptop -- the way the captured devices stand in for the
/// kernel's own description of them.
pub fn legion_go() -> BTreeSet<String> {
    crate::vocabulary::BUTTONS
        .iter()
        .map(|(_, named)| format!("{BUTTON}{named}"))
        .chain(crate::vocabulary::AXES.iter().map(|(_, named)| format!("Gamepad:Axis:{named}")))
        .chain(
            crate::vocabulary::TRIGGERS
                .iter()
                .map(|(_, named)| format!("Gamepad:Trigger:{named}")),
        )
        .collect()
}

impl Router {
    /// The same, read back as a profile.
    ///
    /// Read back rather than built, so that what a test drives is the file the
    /// machine gets and not a second idea of it.
    pub fn profile(&self) -> Result<crate::profile::Profile, String> {
        crate::profile::Profile::read(std::path::Path::new(FILE), &self.yaml())
    }
}

/// Every profile a machine is driven by: the ones the tree carries, and the
/// one that is made rather than kept.
///
/// The router is made out of a device, so anything that wants the whole set
/// has to make it. What stands in for the device here is the machine this
/// desktop grew on -- see `legion_go` -- which is what the emulator and the
/// tests are asking for when they ask for the profiles at all. The device
/// itself has the real one in `/etc`, written by an apply.
pub fn every_profile(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, crate::profile::Profile>, String> {
    let mut profiles = crate::profile::load_all(root)?;
    profiles.insert(NAME.to_string(), Router::of(&legion_go()).profile()?);
    Ok(profiles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Profile, Source};
    use std::path::Path;

    /// A handheld with paddles, and one without.
    fn legion() -> BTreeSet<String> {
        [
            "Gamepad:Button:South",
            "Gamepad:Button:North",
            "Gamepad:Button:DPadUp",
            "Gamepad:Button:LeftPaddle1",
            "Gamepad:Button:QuickAccess",
            "Gamepad:Button:LeftTrigger",
            "Gamepad:Button:LeftStickTouch",
            "Gamepad:Axis:LeftStick",
            "Gamepad:Trigger:LeftTrigger",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    #[test]
    fn every_button_the_device_has_is_routed() {
        let router = Router::of(&legion());
        assert_eq!(router.buttons, ["South", "DPadUp", "LeftPaddle1", "QuickAccess", "North"]);
        assert!(router.without.is_empty());
    }

    /// A trigger is a layer, not a button. Bound to a job it would be a job
    /// that goes off while somebody holds the machine.
    #[test]
    fn a_trigger_is_not_one_of_the_buttons() {
        let router = Router::of(&legion());
        assert!(!router.has("LeftTrigger"));
        assert!(router.yaml().contains("LeftTrigger - a layer, held"));
    }

    /// A button this repository has no word for is left out and said out loud.
    /// Left in, it would be a button nothing could tell from the others left
    /// in with it.
    #[test]
    fn a_button_nothing_here_can_name_is_said_rather_than_dropped() {
        let mut odd = legion();
        odd.insert("Gamepad:Button:ThirdShoulder".to_string());
        let router = Router::of(&odd);
        assert_eq!(router.without, ["ThirdShoulder"]);
        assert!(!router.yaml().contains("ThirdShoulder"));
    }

    /// What it writes is a profile, by the same reader every other one goes
    /// through, and it publishes all three devices.
    #[test]
    fn what_it_writes_is_a_profile_that_reads_back() {
        let router = Router::of(&legion());
        let profile = Profile::read(Path::new(FILE), &router.yaml()).expect("it is a profile");
        assert_eq!(profile.name, "Router");
        assert!(profile.publishes("xbox-elite") && profile.publishes("keyboard"));
        assert!(profile.publishes("mouse"));
        // One for each button, one for each stick, one for each trigger.
        assert_eq!(profile.mappings.len(), router.buttons.len() + 4);
        assert!(
            profile
                .mappings
                .iter()
                .any(|mapping| mapping.source == Source::Button("LeftPaddle1".into()))
        );
    }

    /// The pointer is still the profile's. A pointer moved by a daemon that
    /// polls is a pointer that moves in steps.
    #[test]
    fn the_pointer_is_left_where_it_is_smooth() {
        let yaml = Router::of(&legion()).yaml();
        assert!(yaml.contains("speed_pps: 900"), "{yaml}");
    }

    /// The machine this grew on: every button it has is routed, and none of
    /// them is one this desktop cannot name.
    #[test]
    fn the_machine_this_grew_on_is_routed_whole() {
        let router = Router::of(&legion_go());
        assert!(router.without.is_empty(), "{:?}", router.without);
        assert_eq!(router.buttons.len(), routing::ROUTE.len());
        assert!(router.profile().is_ok());
    }

    /// A device with none of the buttons this desktop puts jobs on still gets
    /// a profile, and the setup screen is where that is answered.
    #[test]
    fn a_device_with_almost_nothing_still_gets_a_profile() {
        let bare: BTreeSet<String> = ["Gamepad:Button:South"].into_iter().map(String::from).collect();
        let router = Router::of(&bare);
        let profile = Profile::read(Path::new(FILE), &router.yaml()).expect("a profile");
        assert_eq!(profile.mappings.len(), 5);
    }
}
