//! The rules the profiles and the switcher have to keep to each other.  Most of
//! what this file used to hold was about what a button *meant* in each of two
//! profiles -- that A accepted everywhere, that B went back, that X reached the
//! on-screen keyboard whatever was on screen. None of that is a profile's to
//! say any more. It is one table in the controller daemon, and
//! `console-input-controller/tests/what_reaches_the_desktop.rs` is where it is
//! held to its word.  What is left is about the files: there is a profile for
//! each thing the pad can be wearing, and every profile publishes all three
//! devices. The second is the one that has cost the most.  The two that held
//! the switcher to those files have gone to
//! `console-input-controller/tests/the_profiles.rs`. They used to read
//! `files/usr/local/bin/controller-profile` and look for paths in its text; the
//! switcher is a Rust program now and the paths are what `Which::file` returns,
//! so the question is asked of the thing that answers it rather than of a shell
//! script that happened to be lying there. It could not stay here either way:
//! this crate is under the daemon, and a test of the daemon's program would
//! point the dependency backwards.  Two words came off that list rather than
//! being tested harder. `keyboard` and `asking` translated nothing and were
//! loaded so that one program could have the front of the machine while its
//! surface was up -- which a profile cannot promise, because any program can
//! load one over it. `console_input_focus` asks the kernel instead. What is
//! left is the desktop's profile and Game Mode's, which is one switch per
//! session rather than one per surface.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use console_input_gamepad::devices::Has;
use console_input_gamepad::profile::Profile;
use console_input_gamepad::router::{self, every_profile};

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn profiles() -> BTreeMap<String, Profile> {
    every_profile(&root()).expect("the profiles")
}

#[test]
fn there_is_a_profile_for_each_thing_the_pad_can_be_wearing() {
    let named: BTreeSet<String> = profiles().into_keys().collect();
    assert_eq!(named, ["game", router::NAME].map(String::from).into());
}

#[test]
fn every_profile_publishes_all_three_devices() {
    for (name, profile) in profiles() {
        for device in ["mouse", "keyboard", "xbox-elite"] {
            assert_eq!(
                profile.publishes(device),
                Ok(Has::Yes),
                "{name} does not publish the {device}"
            );
        }
    }
}
