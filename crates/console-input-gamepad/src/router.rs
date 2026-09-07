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
//! Made rather than kept in the tree: what it holds is one device's buttons,
//! and the tree is what every machine running this desktop has in common. A
//! handheld with no paddles gets a profile with no paddles in it, and the jobs
//! that were on them say so on the setup screen instead of being bound to
//! something nobody can press.
//!
//! The left stick is the one thing here that goes two places, and the reason is
//! the on-screen keyboard. It moves the pointer, which is the desktop's, and it
//! also reaches the pad as itself, which is what the keyboard walks its
//! highlight with while it has the devices claimed. It used to reach the pad
//! because the keyboard's own profile translated nothing; there is no such
//! profile now, and a profile called "every button, said as itself" that
//! swallowed one of them would be a promise this file does not keep. The daemon
//! reads the right stick and never ABS_X, so nothing downstream sees it twice.

use std::collections::BTreeSet;

use console_core_never::Never;

use crate::devices::Has;
use crate::routing;
use crate::vocabulary::BUTTON;

pub const NAME: &str = "router";
pub const FILE: &str = "router.yaml";

pub const PROFILES: &str = "/etc/inputplumber/profiles/";

pub const POINTER_PPS: u32 = 900;

pub const PASSED: [&str; 2] = ["LeftTrigger", "RightTrigger"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Router {
    pub buttons: Vec<&'static str>,
    pub without: Vec<String>,
}

impl Router {
    pub fn of(capabilities: &BTreeSet<String>) -> Result<Self, Never> {
        let mut router = Router::default();

        for capability in capabilities {
            let Some(button) = capability.strip_prefix(BUTTON) else { continue };

            match button.ends_with("Trigger") || button.ends_with("StickTouch") {
                true => continue,
                false => {},
            }

            match routing::ROUTE.iter().find(|(named, _)| *named == button) {
                Some((named, _)) => router.buttons.push(named),
                None => router.without.push(button.to_string()),
            }
        }

        router.buttons.sort_unstable_by_key(|button| {
            routing::ROUTE.iter().position(|(named, _)| named == button)
        });

        Ok(router)
    }

    pub fn has(&self, button: &str) -> Result<Has, Never> {
        Ok(match self.buttons.contains(&button) {
            true => Has::Yes,
            false => Has::No,
        })
    }

    pub fn yaml(&self) -> Result<String, Never> {
        let mut said = String::from(
            "# Written by `console apply`, out of what this device says it can send.\n\
             #\n\
             # Nothing here says what a button means. Every button goes to something the\n\
             # controller daemon can tell from every other button, and what a press comes\n\
             # to -- on the desktop, with a chooser up, with a trigger held -- is decided\n\
             # in one table there and in one file of this machine owner's own. See\n\
             # crates/console-input-controller/src/means.rs.\n\
             #\n\
             # Not a file to edit: it is made again out of the machine on every apply.\n\
             version: 1\n\
             kind: DeviceProfile\n\
             name: Router\n\
             description: Every button, said as itself, for the daemon to read.\n\
             target_devices:\n  - mouse\n  - keyboard\n  - xbox-elite\n\
             \nmapping:\n",
        );
        said.push_str(&format!(
            "  - name: Left stick - move the pointer, and reach the pad as itself\n\
             \x20   source_event:\n      gamepad:\n        axis:\n          name: LeftStick\n\
             \x20   target_events:\n      - mouse:\n          motion:\n            speed_pps: {POINTER_PPS}\n\
             \x20     - gamepad:\n          axis:\n            name: LeftStick\n\n"
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
                 \x20           name: {trigger}\n\n"
            ));
        }

        for button in &self.buttons {
            let mapping = routing::mapping(button)?;

            match mapping {
                Some(mapping) => said.push_str(&mapping),
                None => {},
            }
        }

        Ok(said)
    }
}

pub fn legion_go() -> Result<BTreeSet<String>, Never> {
    Ok(crate::vocabulary::BUTTONS
        .iter()
        .map(|(_, named)| format!("{BUTTON}{named}"))
        .chain(crate::vocabulary::AXES.iter().map(|(_, named)| format!("Gamepad:Axis:{named}")))
        .chain(
            crate::vocabulary::TRIGGERS
                .iter()
                .map(|(_, named)| format!("Gamepad:Trigger:{named}")),
        )
        .collect())
}

#[cfg(feature = "read")]
impl Router {
    pub fn profile(&self) -> Result<crate::profile::Profile, String> {
        let Ok(yaml) = self.yaml();

        crate::profile::Profile::read(std::path::Path::new(FILE), &yaml)
    }
}

#[cfg(feature = "read")]
pub fn every_profile(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, crate::profile::Profile>, String> {
    let mut profiles = crate::profile::load_all(root)?;

    let Ok(every) = legion_go();

    let Ok(router) = Router::of(&every);

    let routed = router.profile()?;

    profiles.insert(NAME.to_string(), routed);
    Ok(profiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }
    use crate::profile::{Kind, Profile, Source, Target};
    use std::path::Path;

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
        let router = ok(Router::of(&legion()));
        assert_eq!(router.buttons, ["South", "DPadUp", "LeftPaddle1", "QuickAccess", "North"]);
        assert!(router.without.is_empty());
    }

    #[test]
    fn a_trigger_is_not_one_of_the_buttons() {
        let router = ok(Router::of(&legion()));
        assert_eq!(router.has("LeftTrigger"), Ok(Has::No));
        assert!(ok(router.yaml()).contains("LeftTrigger - a layer, held"));
    }

    #[test]
    fn a_trigger_reaches_the_pad_as_a_trigger() {
        let Ok(written) = ok(Router::of(&ok(legion_go()))).yaml();
        let profile = Profile::read(Path::new(FILE), &written).expect("a profile");
        for trigger in PASSED {
            let held = profile
                .mappings
                .iter()
                .find(|mapping| mapping.source == Source::Trigger {
                    name: trigger.to_string(),
                    deadzone: Some(0.3),
                })
                .unwrap_or_else(|| panic!("{trigger} is not in the profile"));
            assert_eq!(
                held.targets,
                [Target { kind: Kind::GamepadTrigger, name: trigger.to_string() }],
                "{trigger} does not come out of the profile as itself",
            );
        }
    }

    #[test]
    fn a_button_nothing_here_can_name_is_said_rather_than_dropped() {
        let mut odd = legion();
        odd.insert("Gamepad:Button:ThirdShoulder".to_string());
        let router = ok(Router::of(&odd));
        assert_eq!(router.without, ["ThirdShoulder"]);
        assert!(!ok(router.yaml()).contains("ThirdShoulder"));
    }

    #[test]
    fn what_it_writes_is_a_profile_that_reads_back() {
        let router = ok(Router::of(&legion()));
        let profile = Profile::read(Path::new(FILE), &ok(router.yaml())).expect("it is a profile");
        assert_eq!(profile.name, "Router");
        assert_eq!(profile.publishes("xbox-elite"), Ok(Has::Yes));
        assert_eq!(profile.publishes("keyboard"), Ok(Has::Yes));
        assert_eq!(profile.publishes("mouse"), Ok(Has::Yes));
        assert_eq!(profile.mappings.len(), router.buttons.len() + 4);
        assert!(
            profile
                .mappings
                .iter()
                .any(|mapping| mapping.source == Source::Button("LeftPaddle1".into()))
        );
    }

    #[test]
    fn the_pointer_is_left_where_it_is_smooth() {
        let Ok(yaml) = ok(Router::of(&legion())).yaml();
        assert!(yaml.contains("speed_pps: 900"), "{yaml}");
    }

    #[test]
    fn the_machine_this_grew_on_is_routed_whole() {
        let router = ok(Router::of(&ok(legion_go())));
        assert!(router.without.is_empty(), "{:?}", router.without);
        assert_eq!(router.buttons.len(), routing::ROUTE.len());
        assert!(router.profile().is_ok());
    }

    #[test]
    fn a_device_with_almost_nothing_still_gets_a_profile() {
        let bare: BTreeSet<String> = ["Gamepad:Button:South"].into_iter().map(String::from).collect();
        let router = ok(Router::of(&bare));
        let profile = Profile::read(Path::new(FILE), &ok(router.yaml())).expect("a profile");
        assert_eq!(profile.mappings.len(), 5);
    }
}
