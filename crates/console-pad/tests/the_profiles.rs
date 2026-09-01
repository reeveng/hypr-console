//! The profiles this desktop actually has, read.
//!
//! Two are in the tree and two are made out of the device by an apply, so
//! `every_profile` is what a machine is asked for rather than what a checkout
//! holds. The made ones stand in for the machine this desktop grew on.

use std::path::Path;

use console_pad::profile::Source;
use console_pad::router::every_profile as load_all;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

#[test]
fn every_profile_in_the_checkout_reads() {
    let profiles = load_all(&root()).expect("the profiles read");
    assert!(profiles.contains_key(console_pad::router::NAME), "there is a profile to be driven by");
    for (stem, profile) in &profiles {
        assert!(!profile.name.is_empty(), "{stem} has no name");
        for mapping in &profile.mappings {
            assert!(!mapping.label.is_empty(), "{stem} has a mapping with no name");
        }
    }
}

/// A profile with no mappings at all means everything passes through, and
/// `keyboard.yaml` is documented as doing exactly that. It is the case that
/// says an empty profile is a profile rather than a file that failed to read.
#[test]
fn the_keyboard_profile_passes_everything_through() {
    let profiles = load_all(&root()).expect("the profiles read");
    assert!(profiles["keyboard"].mappings.is_empty());
}

/// Printed by `console-emulate what`, and by the guide on the device.
#[test]
fn every_mapping_says_what_it_does() {
    let profiles = load_all(&root()).expect("the profiles read");
    for (stem, profile) in &profiles {
        for mapping in &profile.mappings {
            assert!(!mapping.does().is_empty(), "{stem}: {:?} says nothing", mapping.label);
        }
    }
}

/// Every button a profile maps is a button this vocabulary knows. A profile
/// naming a button nothing here speaks for is a mapping nothing can press.
#[test]
fn every_button_a_profile_maps_is_one_we_have_a_word_for() {
    let profiles = load_all(&root()).expect("the profiles read");
    for (stem, profile) in &profiles {
        for mapping in &profile.mappings {
            if let Source::Button(name) = &mapping.source {
                let spoken = console_pad::vocabulary::spoken_for(name);
                assert_ne!(spoken, name.as_str(), "{stem} maps {name}, which nothing can press");
            }
        }
    }
}

// What a device that is not a Legion Go is handed is not one of these files at
// all. It is the router, made out of what that machine says it can send, and
// `router::what_it_writes_is_a_profile_that_reads_back` is where it is held to
// being a profile. There were two tests here about rewriting these files
// through a table of moved buttons; there is no such rewriting now, because a
// button's meaning is not in a profile to be moved.
