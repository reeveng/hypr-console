//! The four profiles this desktop actually ships, read.

use std::path::Path;

use console_pad::profile::{Source, load_all};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

#[test]
fn every_profile_in_the_checkout_reads() {
    let profiles = load_all(&root()).expect("the profiles read");
    assert!(profiles.contains_key("desktop"), "there is a desktop profile");
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
