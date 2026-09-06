//! The profiles this desktop actually has, read.
//!
//! One is in the tree and one is made out of the device by an apply, so
//! `every_profile` is what a machine is asked for rather than what a checkout
//! holds. The made one stands in for the machine this desktop grew on.

use std::path::Path;

use console_gamepad::profile::{Kind, Source};
use console_gamepad::router::every_profile as load_all;

fn root() -> std::path::PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

#[test]
fn every_profile_in_the_checkout_reads() {
    let profiles = load_all(&root()).expect("the profiles read");
    assert!(profiles.contains_key(console_gamepad::router::NAME), "there is a profile to be driven by");
    for (stem, profile) in &profiles {
        assert!(!profile.name.is_empty(), "{stem} has no name");
        for mapping in &profile.mappings {
            assert!(!mapping.label.is_empty(), "{stem} has a mapping with no name");
        }
    }
}

#[test]
fn the_game_profile_passes_everything_through() {
    let profiles = load_all(&root()).expect("the profiles read");
    assert!(profiles["game"].mappings.is_empty());
}

#[test]
fn the_left_stick_moves_the_pointer_and_reaches_the_pad_as_well() {
    let profiles = load_all(&root()).expect("the profiles read");
    let router = &profiles[console_gamepad::router::NAME];
    let stick = router
        .mappings
        .iter()
        .find(|mapping| matches!(&mapping.source, Source::Axis { name, .. } if name == "LeftStick"))
        .expect("the router says nothing about the left stick");
    let goes: Vec<Kind> = stick.targets.iter().map(|target| target.kind).collect();
    assert!(goes.contains(&Kind::MouseMotion), "the pointer no longer moves: {goes:?}");
    assert!(
        goes.contains(&Kind::GamepadAxis),
        "the left stick does not reach the pad, so the on-screen keyboard cannot walk with it: \
         {goes:?}"
    );
}

#[test]
fn every_mapping_says_what_it_does() {
    let profiles = load_all(&root()).expect("the profiles read");
    for (stem, profile) in &profiles {
        for mapping in &profile.mappings {
            let Ok(does) = mapping.does();

            assert!(!does.is_empty(), "{stem}: {:?} says nothing", mapping.label);
        }
    }
}

#[test]
fn every_button_a_profile_maps_is_one_we_have_a_word_for() {
    let profiles = load_all(&root()).expect("the profiles read");
    for (stem, profile) in &profiles {
        for mapping in &profile.mappings {
            if let Source::Button(name) = &mapping.source {
                let Ok(spoken) = console_gamepad::vocabulary::spoken_for(name);

                assert_ne!(spoken, name.as_str(), "{stem} maps {name}, which nothing can press");
            }
        }
    }
}
