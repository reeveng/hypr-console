//! The promises the front of the machine makes.
//!
//! A person holding this thing learns four buttons once and then stops
//! thinking about them. That only holds if the answer is the same in every
//! program, and what a button means is decided in four separate files, so it
//! is checked here rather than remembered:
//!
//! ```text
//! D-pad   moves between things: options, windows, pages. Never does anything.
//! A       accepts. Whatever is highlighted, that one.
//! B       goes back: cancels, closes, and deletes in the keyboard.
//! X       shows the keyboard, and hides it again, wherever you are.
//! Y       is not spoken for.
//!
//! Left    paddle, top: opens the menu. Only ever opens.
//! Right   paddle, top: closes whatever is up.
//! ```
//!
//! The fifth rule is not about a person at all. An event can only reach a
//! device the profile lists in `target_devices`, because InputPlumber builds
//! what a profile names and destroys the rest, so a mapping that sends a pad
//! button from a profile with no pad in it is a button that does nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use evdev::KeyCode;
use console_pad::capture::captured;
use console_pad::profile::{Kind, Profile, Target, load_all};
use console_pad::vocabulary;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

fn profiles() -> BTreeMap<String, Profile> {
    load_all(&root()).expect("the profiles")
}

/// The profiles that map anything.
///
/// Two of them map nothing on purpose, and neither is a surface these rules
/// are about. While the on-screen keyboard is up it reads the pad itself, and
/// anything translated there would happen twice. While Game Mode has the
/// screen the pad is Steam's and a game's, which is a pad rather than this
/// desktop, and a button that accepted here would be a button a game could not
/// use.
fn mapped() -> BTreeMap<String, Profile> {
    profiles().into_iter().filter(|(_, profile)| !profile.mappings.is_empty()).collect()
}

fn target(kind: Kind, name: &str) -> Target {
    Target { kind, name: name.to_string() }
}

/// What A accepting looks like, which depends on what is on screen. A chooser
/// is driven by the highlight, so A is Enter there. On the desktop there is no
/// highlight to confirm, and accepting is clicking what the pointer is on.
fn accepts() -> BTreeSet<Target> {
    [target(Kind::Key, "KeyEnter"), target(Kind::MouseButton, "Left")].into()
}

fn back() -> BTreeSet<Target> {
    [target(Kind::Key, "KeyEsc")].into()
}

/// The signal the on-screen keyboard watches for. It reads the pad itself, so
/// what X has to do is arrive on the pad as North, whatever the profile.
fn keyboard_toggle() -> Target {
    target(Kind::GamepadButton, "North")
}

/// Moving between things, and nothing else.
const NAVIGATION: [KeyCode; 9] = [
    KeyCode::KEY_DOWN,
    KeyCode::KEY_END,
    KeyCode::KEY_HOME,
    KeyCode::KEY_LEFT,
    KeyCode::KEY_PAGEDOWN,
    KeyCode::KEY_PAGEUP,
    KeyCode::KEY_RIGHT,
    KeyCode::KEY_TAB,
    KeyCode::KEY_UP,
];

const DPAD: [&str; 4] = ["dpad-down", "dpad-left", "dpad-right", "dpad-up"];

fn targets_of(profile: &Profile, button: &str) -> BTreeSet<Target> {
    profile.targets_of(button).expect("a button we have a word for").into_iter().cloned().collect()
}

#[test]
fn there_is_a_profile_for_each_word_controller_profile_takes() {
    let named: BTreeSet<String> = profiles().into_keys().collect();
    assert_eq!(named, ["desktop", "game", "keyboard", "tabs"].map(String::from).into());
}

/// And each word names one of these, rather than a file from a package.
///
/// Game Mode used to be handed the shipped Default profile, which publishes
/// whatever it publishes: a profile switch that destroys a target and builds
/// another is what the rule below is about, and it cannot be kept against a
/// file this repository does not hold.
#[test]
fn every_profile_the_switcher_names_is_one_of_these() {
    let switcher = root().join("files/usr/local/bin/controller-profile");
    let said = std::fs::read_to_string(switcher).expect("controller-profile");
    let named: BTreeSet<&str> = said
        .split_whitespace()
        .filter(|word| word.contains("/inputplumber/profiles/"))
        .map(|word| word.trim_start_matches("P="))
        .collect();
    assert!(!named.is_empty(), "controller-profile names no profile at all");
    for path in named {
        let held = root().join("files").join(path.trim_start_matches('/'));
        assert!(held.is_file(), "controller-profile loads {path}, which is not in the tree");
    }
}

#[test]
fn a_accepts_everywhere() {
    for (name, profile) in mapped() {
        let targets = targets_of(&profile, "a");
        assert!(!targets.is_empty(), "{name}: A does nothing");
        assert!(targets.is_subset(&accepts()), "{name}: A does more than accept: {targets:?}");
    }
}

/// The paddles keep one meaning in every profile.
///
/// The pad changes profile a beat after the screen changes, and a thumb is
/// quicker than that beat: a paddle that means one thing while a chooser is up
/// and another while it is not means the wrong one for anybody pressing it
/// just then. The left paddle sent Escape into a menu that had already closed,
/// which is the press that went missing out of every open and close. What
/// opening and closing come to is worked out by the daemon, which can see the
/// screen, so there is nothing here for a profile to decide.
#[test]
fn the_paddles_mean_the_same_thing_everywhere() {
    for (name, profile) in mapped() {
        assert_eq!(
            targets_of(&profile, "left-paddle-top"),
            [target(Kind::Key, "KeyF13")].into(),
            "{name}: the left paddle is something else"
        );
        assert_eq!(
            targets_of(&profile, "right-paddle-top"),
            [target(Kind::Key, "KeyF15")].into(),
            "{name}: the right paddle is something else"
        );
    }
}

#[test]
fn b_goes_back_everywhere() {
    for (name, profile) in mapped() {
        assert_eq!(targets_of(&profile, "b"), back(), "{name}: B is something else");
    }
}

#[test]
fn x_shows_and_hides_the_keyboard_everywhere() {
    for (name, profile) in mapped() {
        let targets = targets_of(&profile, "x");
        assert!(targets.contains(&keyboard_toggle()), "{name}: X does not reach the keyboard");
    }
}

/// Which is how X still closes the keyboard that X opened.
#[test]
fn the_keyboard_profile_passes_everything_through() {
    assert!(profiles()["keyboard"].mappings.is_empty());
}

/// And so does Game Mode's, for the other reason: what is on the screen there
/// is Steam and the games under it, and every button on the front belongs to
/// them. The way back out is the same button held, which nothing here
/// translates: `game-return` reads it off the pad, so Steam goes on getting
/// the press.
#[test]
fn the_game_profile_passes_everything_through() {
    assert!(profiles()["game"].mappings.is_empty());
}

#[test]
fn the_dpad_only_moves_between_things() {
    for (name, profile) in mapped() {
        for button in DPAD {
            for target in targets_of(&profile, button) {
                assert_eq!(target.kind, Kind::Key, "{name}: {button} does something: {target:?}");
                let code = target.code().expect("a key we have a word for");
                assert!(
                    NAVIGATION.contains(&code),
                    "{name}: {button} sends {}, which is not moving between things",
                    target.name
                );
            }
        }
    }
}

#[test]
fn the_dpad_moves_up_and_down_wherever_there_is_a_list() {
    for (name, profile) in mapped() {
        for button in ["dpad-down", "dpad-up"] {
            assert!(!targets_of(&profile, button).is_empty(), "{name}: {button} does nothing");
        }
    }
}

/// It has no job that a person has to learn, so nothing may quietly give it
/// one that another rule already owns.
#[test]
fn y_is_not_spoken_for() {
    for (name, profile) in mapped() {
        let targets = targets_of(&profile, "y");
        let spoken_for: BTreeSet<Target> = accepts().union(&back()).cloned().collect();
        let taken: Vec<&Target> = targets.intersection(&spoken_for).collect();
        assert!(taken.is_empty(), "{name}: Y has taken a job that belongs to A or B");
        assert!(!targets.contains(&keyboard_toggle()), "{name}: Y has taken the keyboard, X's");
    }
}

/// InputPlumber builds the targets a profile names and destroys the rest. A
/// mapping onto a device this profile has not asked for goes nowhere, and goes
/// nowhere silently.
#[test]
fn nothing_is_sent_to_a_device_the_profile_does_not_publish() {
    for (name, profile) in profiles() {
        for mapping in &profile.mappings {
            for target in &mapping.targets {
                let wanted = target.kind.needs();
                assert!(
                    profile.publishes(wanted),
                    "{name}: {:?} sends to {wanted}, which {name} does not publish",
                    mapping.label
                );
            }
        }
    }
}

/// Switching profiles must not destroy one and build it again. The compositor
/// does not deliver anything from a keyboard that appeared after it started:
/// the device is there, it is listed, and every key it sends is dropped. That
/// is what made the on-screen keyboard break over and over.
#[test]
fn every_profile_publishes_the_same_devices() {
    for (name, profile) in profiles() {
        let published: BTreeSet<String> = profile.target_devices.iter().cloned().collect();
        assert_eq!(
            published,
            ["keyboard", "mouse", "xbox-elite"].map(String::from).into(),
            "{name} publishes something else"
        );
    }
}

/// The keyboard InputPlumber publishes carries a fixed set of keys. One it does
/// not carry cannot be sent, however the profile spells it.
#[test]
fn every_key_a_profile_sends_is_a_key_the_keyboard_has() {
    let has: BTreeSet<u16> = captured()["keyboard"].capabilities.key.iter().copied().collect();
    for (name, profile) in profiles() {
        for mapping in &profile.mappings {
            for target in mapping.targets.iter().filter(|target| target.kind == Kind::Key) {
                let code = target.code().expect("a key we have a word for");
                assert!(
                    has.contains(&code.0),
                    "{name}: {:?} sends {}, which is not on the keyboard",
                    mapping.label,
                    target.name
                );
            }
        }
    }
}

/// Named and given nothing, which means the same thing whether an unmapped
/// button is passed through or dropped.
///
/// Only these two. The settings button and the menu button each open a chooser
/// of their own and go on doing it with a chooser already up, because a button
/// on the front of the machine means one thing wherever it is pressed. Leaving
/// the desktop is the exception: that one stays quiet, so a menu open under a
/// thumb cannot become Game Mode.
#[test]
fn a_button_with_nothing_to_do_here_says_so() {
    let profiles = profiles();
    // The chooser profiles, of which there is one just now. Asked as a list
    // because the question is about all of them, and the next one added is a
    // line here rather than a test rewritten.
    #[allow(clippy::single_element_loop)]
    for where_ in ["tabs"] {
        let silent: BTreeSet<&str> = profiles[where_]
            .mappings
            .iter()
            .filter(|mapping| mapping.targets.is_empty())
            .filter_map(|mapping| mapping.source.button())
            .collect();
        for button in ["legion-left", "view"] {
            assert!(silent.contains(button), "{where_}: {button} is not named and silenced");
        }
    }
}

#[test]
fn the_shoulders_move_between_pages_where_there_are_pages() {
    let profiles = profiles();
    for (button, key) in [("l1", KeyCode::KEY_PAGEUP), ("r1", KeyCode::KEY_PAGEDOWN)] {
        let codes: Vec<Option<KeyCode>> =
            targets_of(&profiles["tabs"], button).iter().map(Target::code).collect();
        assert_eq!(codes, [Some(key)], "tabs: {button} is something else");
    }
}

/// Every button the desktop names, a chooser names too.
///
/// InputPlumber passes a source it was told nothing about straight through to
/// whatever pad the profile publishes. So a button left out of a chooser's
/// profile does not stop working: it arrives as the pad's own button, on a
/// device nothing is reading it from, and does nothing for a reason nobody can
/// see from either file. Three of the four paddles were in that state, showing
/// up in the journal as BTN_TRIGGER_HAPPY while a chooser was up and answered
/// by no one.
///
/// Named and sent nowhere is a decision. Left out is an accident that reads
/// the same.
#[test]
fn a_chooser_leaves_no_button_to_chance() {
    let profiles = profiles();
    let named: Vec<&str> = vocabulary::BUTTONS
        .iter()
        .map(|(spoken, _)| *spoken)
        .filter(|spoken| !profiles["desktop"].for_button(spoken).expect("a button").is_empty())
        .collect();
    // The chooser profiles again, asked as a list for the same reason.
    #[allow(clippy::single_element_loop)]
    for where_ in ["tabs"] {
        let missing: Vec<&str> = named
            .iter()
            .copied()
            .filter(|spoken| profiles[where_].for_button(spoken).expect("a button").is_empty())
            .collect();
        assert!(missing.is_empty(), "{where_} says nothing about {}", missing.join(", "));
    }
}
