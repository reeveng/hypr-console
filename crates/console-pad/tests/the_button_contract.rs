//! The rules the profiles and the switcher have to keep to each other.
//!
//! Most of what this file used to hold was about what a button *meant* in each
//! of two profiles -- that A accepted everywhere, that B went back, that X
//! reached the on-screen keyboard whatever was on screen. None of that is a
//! profile's to say any more. It is one table in the controller daemon, and
//! `console-controller/tests/what_reaches_the_desktop.rs` is where it is held
//! to its word.
//!
//! What is left is the part that was always about the files: every word the
//! switcher takes names a profile that exists, and every profile publishes all
//! three devices. The second is the one that has cost the most.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use console_pad::devices::Has;
use console_pad::profile::Profile;
use console_pad::router::{self, PROFILES, every_profile};

fn root() -> PathBuf {
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn profiles() -> BTreeMap<String, Profile> {
    every_profile(&root()).expect("the profiles")
}

fn switcher() -> String {
    std::fs::read_to_string(root().join("files/usr/local/bin/controller-profile"))
        .expect("controller-profile")
}

/// The profiles a machine has: two in the tree, and the two made out of the
/// device by an apply.
#[test]
fn there_is_a_profile_for_each_thing_the_pad_can_be_wearing() {
    let named: BTreeSet<String> = profiles().into_keys().collect();
    assert_eq!(named, ["game", "keyboard", router::NAME].map(String::from).into());
}

/// Every path the switcher can load is a file that exists, or one the engine
/// writes.
///
/// Game Mode used to be handed the shipped Default profile, which publishes
/// whatever it publishes: a profile switch that destroys a target and builds
/// another is what the rule below is about, and it cannot be kept against a
/// file this repository does not hold.
///
/// Two profiles the tree cannot hold are named here rather than excused. The
/// router and the asking profile are both written by `console apply` out of
/// what the device itself says it can send, because what they hold is one
/// machine's buttons and the tree is what every machine has in common. Their
/// paths are what is checked, against the constants the engine writes them at,
/// so the script and the engine cannot drift into naming different files.
#[test]
fn every_profile_the_switcher_names_is_one_of_these() {
    let said = switcher();
    let named: BTreeSet<&str> = said
        .split_whitespace()
        .filter(|word| word.contains("/inputplumber/profiles/"))
        .map(|word| word.trim_start_matches("P="))
        .collect();
    assert!(!named.is_empty(), "controller-profile names no profile at all");
    let made = [format!("{PROFILES}asking.yaml"), format!("{PROFILES}{}", router::FILE)];
    for path in named {
        if made.iter().any(|written| written == path) {
            continue;
        }
        let held = root().join("files").join(path.trim_start_matches('/'));
        assert!(held.is_file(), "controller-profile loads {path}, which is not in the tree");
    }
}

/// Both made profiles are named by the switcher: without the first the desktop
/// has no buttons at all, and without the second the card that asks which
/// button you pressed has no way to make the front of the machine inert.
#[test]
fn the_switcher_knows_the_profiles_that_are_made_rather_than_kept() {
    let said = switcher();
    for written in [format!("{PROFILES}asking.yaml"), format!("{PROFILES}{}", router::FILE)] {
        assert!(said.contains(&written), "controller-profile takes no word for {written}");
    }
}

/// The rule that has cost the most.
///
/// InputPlumber destroys a target device a profile leaves out and builds a new
/// one when it comes back, and the compositor does not deliver anything from a
/// keyboard that appeared after it started: the device is there, it is listed,
/// and every key it sends is dropped. That is what made the on-screen keyboard
/// "break" over and over, and it is why every profile here lists all three
/// whether or not it sends anything to them.
#[test]
fn every_profile_publishes_all_three_devices() {
    for (name, profile) in profiles() {
        for device in ["mouse", "keyboard", "xbox-elite"] {
            assert_eq!(
                profile.publishes(device),
                Has::Yes,
                "{name} does not publish the {device}"
            );
        }
    }
}
