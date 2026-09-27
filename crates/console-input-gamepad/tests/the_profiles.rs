//! The profiles this desktop actually has, read.
//!
//! One is in the tree and one is made out of the device by an apply, so
//! `every_profile` is what a machine is asked for rather than what a checkout
//! holds. The made one stands in for the machine this desktop grew on.

use std::collections::BTreeMap;
use std::error::Error;

use console_input_gamepad::profile::{Kind, Profile, Source};
use console_input_gamepad::router::every_profile;

fn profiles() -> Result<BTreeMap<String, Profile>, Box<dyn Error>> {
    let root = console_repository::root()?;
    let profiles = every_profile(&root)?;

    Ok(profiles)
}

#[test]
fn every_profile_in_the_checkout_reads() -> Result<(), Box<dyn Error>> {
    let profiles = profiles()?;

    assert!(profiles.contains_key(console_input_gamepad::router::NAME), "there is a profile to be driven by");

    for (stem, profile) in &profiles {
        assert!(!profile.name.is_empty(), "{stem} has no name");

        for mapping in &profile.mappings {
            assert!(!mapping.label.is_empty(), "{stem} has a mapping with no name");
        }
    }

    Ok(())
}

#[test]
fn the_game_profile_passes_everything_through() -> Result<(), Box<dyn Error>> {
    let profiles = profiles()?;
    let game = profiles.get("game").ok_or("there is no game profile")?;

    assert!(game.mappings.is_empty());

    Ok(())
}

#[test]
fn the_left_stick_moves_the_pointer_and_reaches_the_pad_as_well() -> Result<(), Box<dyn Error>> {
    let profiles = profiles()?;
    let router = profiles.get(console_input_gamepad::router::NAME).ok_or("there is no router profile")?;
    let stick = router
        .mappings
        .iter()
        .find(|mapping| match &mapping.source {
            Source::Axis { name, .. } => name == "LeftStick",
            Source::Button(_) | Source::Trigger { .. } => false,
        })
        .ok_or("the router says nothing about the left stick")?;
    let goes: Vec<Kind> = stick.targets.iter().map(|target| target.kind).collect();

    assert!(goes.contains(&Kind::MouseMotion), "the pointer no longer moves: {goes:?}");
    assert!(
        goes.contains(&Kind::GamepadAxis),
        "the left stick does not reach the pad, so the on-screen keyboard cannot walk with it: \
         {goes:?}"
    );

    Ok(())
}

#[test]
fn every_mapping_says_what_it_does() -> Result<(), Box<dyn Error>> {
    let profiles = profiles()?;

    for (stem, profile) in &profiles {
        for mapping in &profile.mappings {
            let Ok(does) = mapping.does();

            assert!(!does.is_empty(), "{stem}: {:?} says nothing", mapping.label);
        }
    }

    Ok(())
}

#[test]
fn every_button_a_profile_maps_is_one_we_have_a_word_for() -> Result<(), Box<dyn Error>> {
    let profiles = profiles()?;

    for (stem, profile) in &profiles {
        let buttons = profile.mappings.iter().filter_map(|mapping| match &mapping.source {
            Source::Button(name) => Some(name),
            Source::Axis { .. } | Source::Trigger { .. } => None,
        });

        for name in buttons {
            let Ok(spoken) = console_input_gamepad::vocabulary::spoken_for(name);

            assert_ne!(spoken, name.as_str(), "{stem} maps {name}, which nothing can press");
        }
    }

    Ok(())
}
