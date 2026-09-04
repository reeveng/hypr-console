//! The profile that asks which button that was.
//!
//! Somebody moving the menu onto a button their device actually has should say
//! so by pressing it. Nothing here can read the press as it leaves the
//! hardware -- on this handheld half the buttons are hidraw and InputPlumber
//! is the only thing that sees them -- so the question is asked the way
//! everything else here asks the front of the machine something: with a
//! profile.
//!
//! It maps every button the device says it has to a key of its own, and then a
//! press is a key, and the key says which button it was. The keys are chosen
//! for being keys nothing on this desktop does anything with. That is the
//! whole trick, and it matters more than it sounds: under a profile that let
//! presses through, pressing Legion left to bind it would leave for Game Mode,
//! X would open the on-screen keyboard over the question, and the shoulders
//! would carry the window away. While this profile is loaded the front of the
//! machine is inert, which is exactly what a screen asking "press one" wants.

use std::collections::BTreeSet;

use crate::vocabulary::{BUTTON, key_code};

/// Keys nothing on this desktop is listening for.
///
/// Not F13 to F17: the router sends the paddles and Legion right on those, and
/// they are the buttons somebody is most likely to be binding. The daemon
/// stands down while this card is up, so a capture key it would otherwise act
/// on is not a fault -- but a press that could be read twice if the card went
/// away mid-hold is a fault waiting for the one machine where it lands, and
/// the keys cost nothing.
///
/// The rest are chosen for being inert rather than for being tidy. The
/// function keys above the ones in use, the four Prog keys no keyboard here
/// has, the Japanese input keys, and the keypad's brackets: nothing in this
/// desktop, in GTK, or in the compositor binds any of them, and none of them
/// types a letter into the question they are being pressed at.
pub const SPARE: [&str; 25] = [
    "KeyF18",
    "KeyF19",
    "KeyF20",
    "KeyF21",
    "KeyF22",
    "KeyF23",
    "KeyF24",
    "KeyProg1",
    "KeyProg2",
    "KeyProg3",
    "KeyProg4",
    "KeyKatakana",
    "KeyHiragana",
    "KeyKatakanaHiragana",
    "KeyHenkan",
    "KeyMuhenkan",
    "KeyZenkakuhankaku",
    "KeyRo",
    "KeyYen",
    "KeyHanja",
    "KeyKpJpComma",
    "KeyKpEqual",
    "KeyKpLeftParen",
    "KeyKpRightParen",
    "KeyAgain",
];

/// What a button has to be before a role can be moved onto it.
///
/// A press, made on purpose, by a thumb. The sticks and the triggers are left
/// out because they are not presses; the two `StickTouch` capabilities are
/// left out because they are a finger resting on a stick, which would bind the
/// menu to holding the thing normally.
pub fn pressable(capability: &str) -> Sends {
    let pressable = match capability.strip_prefix(BUTTON) {
        Some(button) => !button.ends_with("StickTouch") && !button.ends_with("Trigger"),
        None => false,
    };

    match pressable {
        true => Sends::APress,
        false => Sends::SomethingElse,
    }
}

/// Whether a capability is a button being pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sends {
    /// A press, which is a thing a job can be bound to.
    APress,
    /// A stick, a trigger or a finger resting on one -- movement rather than a
    /// press, and binding a job to it would fire it by holding the pad
    /// normally.
    SomethingElse,
}

/// The two triggers, passed through untouched while the question is up.
///
/// Everything else about this profile is about making the machine inert, and
/// these are the exception on purpose: a job can be put on a chord, so the
/// card has to be able to see one. It reads how far each trigger is pulled off
/// the pad at the moment of the press, and a trigger that is not passed
/// through is a trigger the card cannot see being held.
///
/// They are safe to leave live for the same reason they are the layers: a
/// trigger is not a press. `pressable` refuses to bind anything to one, so
/// nothing here can be answered by pulling one.
pub const HELD: [&str; 2] = ["LeftTrigger", "RightTrigger"];

/// A profile that turns every button this device has into a key of its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asking {
    /// Each button, and the key it is being made to send.
    pub keys: Vec<(String, &'static str)>,
    /// The buttons there was no spare key left for.
    ///
    /// Never any on the machine this was written for, which has twenty-two
    /// buttons a thumb can press and twenty-five keys to lend them. A device
    /// with more is one where the last few are chosen from the list instead of
    /// pressed, which is worth saying out loud rather than quietly dropping.
    pub without: Vec<String>,
}

impl Asking {
    /// What to ask, given what the machine said it has.
    pub fn of(capabilities: &BTreeSet<String>) -> Self {
        let mut spare = SPARE.iter();
        let mut asking = Asking::default();

        for capability in capabilities.iter().filter(|said| pressable(said) == Sends::APress) {
            match spare.next() {
                Some(key) => asking.keys.push((capability.clone(), *key)),
                None => asking.without.push(capability.clone()),
            }
        }

        asking
    }

    /// Which button a key means, if this profile lent that key to one.
    pub fn pressed(&self, key: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|(_, lent)| *lent == key)
            .map(|(capability, _)| capability.as_str())
    }

    /// Which button a press was, given the code the key arrived as.
    ///
    /// The kernel says a number and a profile says a name, and this is the one
    /// place the two are joined for the keys lent out here.
    pub fn pressed_code(&self, code: u16) -> Option<&str> {
        self.keys
            .iter()
            .find(|(_, lent)| key_code(lent).is_ok_and(|key| key.0 == code))
            .map(|(capability, _)| capability.as_str())
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
            "# Written by the setup screen, and loaded only while it is asking.\n\
             #\n\
             # Every button this device has sends a key nothing is listening for, so\n\
             # that a press can be read as the button it came from and does nothing else\n\
             # on its way past. Not a file to edit: it is made again, out of what the\n\
             # machine says it has, every time the question is asked.\n\
             version: 1\n\
             kind: DeviceProfile\n\
             name: Asking\n\
             description: Every button, sent somewhere nothing is listening.\n\
             target_devices:\n  - mouse\n  - keyboard\n  - xbox-elite\n\
             \nmapping:\n",
        );

        for (capability, key) in &self.keys {
            let button = capability.strip_prefix(BUTTON).unwrap_or(capability);
            said.push_str(&format!(
                "  - name: {button} - which button that was\n\
                 \x20   source_event:\n      gamepad:\n        button: {button}\n\
                 \x20   target_events:\n      - keyboard: {key}\n\n"
            ));
        }

        for trigger in HELD {
            said.push_str(&format!(
                "  - name: {trigger} - so the card can see a chord being held\n\
                 \x20   source_event:\n      gamepad:\n        trigger:\n\
                 \x20         name: {trigger}\n          deadzone: 0.3\n\
                 \x20   target_events:\n      - gamepad:\n          trigger:\n\
                 \x20         name: {trigger}\n\n"
            ));
        }

        said
    }
}

#[cfg(test)]
mod tests {
    use crate::devices::Has;
    use super::*;
    use crate::profile::{Profile, Source};
    use std::path::Path;

    /// What this device answered, cut to the buttons and the two kinds of
    /// thing that are not presses.
    fn said() -> BTreeSet<String> {
        [
            "Gamepad:Axis:LeftStick",
            "Gamepad:Button:South",
            "Gamepad:Button:LeftPaddle1",
            "Gamepad:Button:QuickAccess",
            "Gamepad:Button:LeftStickTouch",
            "Gamepad:Trigger:LeftTrigger",
            "Gamepad:Button:LeftTrigger",
            "Gyroscope:Center",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    #[test]
    fn only_what_a_thumb_presses_on_purpose_is_asked_about() {
        let asking = Asking::of(&said());
        let buttons: Vec<&str> = asking.keys.iter().map(|(said, _)| said.as_str()).collect();
        assert_eq!(
            buttons,
            [
                "Gamepad:Button:LeftPaddle1",
                "Gamepad:Button:QuickAccess",
                "Gamepad:Button:South"
            ]
        );
    }

    /// The card asks for a button and a job may be on a chord, so the triggers
    /// stay live while everything else is inert. Nothing can be bound to one:
    /// they are what is held, not what is pressed.
    #[test]
    fn the_triggers_are_passed_through_so_a_chord_can_be_seen() {
        let yaml = Asking::of(&said()).yaml();
        for trigger in HELD {
            assert!(yaml.contains(&format!("name: {trigger}")), "{trigger} is not passed through");
        }
        let profile = Profile::read(Path::new("asking.yaml"), &yaml).expect("a profile");
        let held: Vec<&Source> = profile
            .mappings
            .iter()
            .map(|mapping| &mapping.source)
            .filter(|source| matches!(source, Source::Trigger { .. }))
            .collect();
        assert_eq!(held.len(), HELD.len());
        assert!(
            HELD.iter()
                .all(|trigger| pressable(&format!("{BUTTON}{trigger}")) == Sends::SomethingElse)
        );
    }

    #[test]
    fn a_stick_being_rested_on_is_not_a_press() {
        assert_eq!(pressable("Gamepad:Button:LeftStickTouch"), Sends::SomethingElse);
        assert_eq!(pressable("Gamepad:Button:LeftTrigger"), Sends::SomethingElse);
        assert_eq!(pressable("Gamepad:Axis:LeftStick"), Sends::SomethingElse);
        assert_eq!(pressable("Gamepad:Button:South"), Sends::APress);
    }

    /// Every button gets its own key, or the question cannot tell two presses
    /// apart.
    #[test]
    fn no_two_buttons_are_lent_the_same_key() {
        let many: BTreeSet<String> = (0..SPARE.len())
            .map(|at| format!("Gamepad:Button:Made{at:02}"))
            .collect();
        let asking = Asking::of(&many);
        let lent: BTreeSet<&str> = asking.keys.iter().map(|(_, key)| *key).collect();
        assert_eq!(lent.len(), asking.keys.len());
        assert!(asking.without.is_empty());
    }

    /// A device with more buttons than there are spare keys says which ones it
    /// could not ask about, rather than dropping them where nobody looks.
    #[test]
    fn a_device_with_more_buttons_than_keys_says_which_it_could_not_ask_about() {
        let many: BTreeSet<String> = (0..SPARE.len() + 2)
            .map(|at| format!("Gamepad:Button:Made{at:02}"))
            .collect();
        let asking = Asking::of(&many);
        assert_eq!(asking.keys.len(), SPARE.len());
        assert_eq!(asking.without.len(), 2);
    }

    #[test]
    fn a_press_is_read_back_as_the_button_it_came_from() {
        let asking = Asking::of(&said());
        let (button, key) = asking.keys[0].clone();
        assert_eq!(asking.pressed(key), Some(button.as_str()));
        assert_eq!(asking.pressed("KeyEnter"), None);
    }

    /// None of the five the daemon acts on. A capture key that opened the menu
    /// would be a question that answered itself.
    /// Every key lent out is a key this repository can name to the kernel. A
    /// name InputPlumber takes and evdev does not is a press that arrives and
    /// is never recognised, which would read as a button that does nothing.
    #[test]
    fn every_spare_key_is_one_both_sides_know() {
        for key in SPARE {
            assert!(key_code(key).is_ok(), "nothing in the kernel is called {key}");
        }
    }

    #[test]
    fn a_press_is_read_back_from_the_code_the_kernel_sent() {
        let asking = Asking::of(&said());
        let (button, key) = asking.keys[0].clone();
        let code = key_code(key).expect("a key").0;
        assert_eq!(asking.pressed_code(code), Some(button.as_str()));
        assert_eq!(asking.pressed_code(evdev::KeyCode::KEY_ENTER.0), None);
    }

    #[test]
    fn no_capture_key_is_one_this_desktop_listens_for() {
        for taken in ["KeyF13", "KeyF14", "KeyF15", "KeyF16", "KeyF17"] {
            assert!(!SPARE.contains(&taken), "{taken} is what a paddle already sends");
        }
    }

    /// The profile it writes is a profile: the same reader the real ones go
    /// through parses it, and finds every button pointed at its own key.
    #[test]
    fn what_it_writes_is_a_profile_that_reads_back()
    {
        let asking = Asking::of(&said());
        let profile =
            Profile::read(Path::new("asking.yaml"), &asking.yaml()).expect("it is a profile");
        assert_eq!(profile.name, "Asking");
        assert_eq!(profile.mappings.len(), asking.keys.len() + HELD.len());
        assert_eq!(profile.publishes("xbox-elite"), Has::Yes);
        assert_eq!(profile.publishes("keyboard"), Has::Yes);
        let first = &profile.mappings[0];
        assert_eq!(first.source, Source::Button("LeftPaddle1".into()));
        assert_eq!(first.targets[0].name, asking.keys[0].1);
    }
}
