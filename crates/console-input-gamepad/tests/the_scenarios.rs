//! The written-down scenarios, played where nothing can go wrong.
//!
//! A scenario is what someone did with their thumbs, kept so it can be done
//! again. They are worth keeping only if they still run, and a scenario naming a
//! button that has since been renamed is a scenario no one will find out about
//! until they reach for it.

use std::error::Error;
use std::path::PathBuf;

use console_input_gamepad::capture::load_capture;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::LegionGo;
use console_waiting::clock::TestClock;
use console_input_gamepad::router::every_profile;
use console_input_gamepad::script::play;
use console_input_gamepad::world::World;

fn scenarios() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = console_repository::root()?;
    let mut found = Vec::new();

    let entries = std::fs::read_dir(root.join("scenarios"))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        match path.extension().is_some_and(|kind| kind == "txt") {
            true => found.push(path),
            false => {}
        }
    }

    found.sort();

    Ok(found)
}

#[test]
fn there_are_some() -> Result<(), Box<dyn Error>> {
    let scenarios = scenarios()?;

    assert!(!scenarios.is_empty());

    Ok(())
}

#[test]
fn every_scenario_plays() -> Result<(), Box<dyn Error>> {
    let root = console_repository::root()?;

    let scenarios = scenarios()?;

    for path in scenarios {
        let seen = load_capture()?;
        let descriptors = load_capture()?;
        let Ok(world) = World::of(seen);
        let Ok(devices) = Devices::new(descriptors, world);
        let profiles = every_profile(&root)?;
        let mut go = LegionGo::new(profiles, devices, TestClock::default(), console_input_gamepad::router::NAME)?;
        let said = std::fs::read_to_string(&path)?;
        let name = path.display();

        play(&mut go, &said).map_err(|fault| format!("{name}: {fault}"))?;

        assert!(!go.devices.sink.log.is_empty(), "{name} pressed nothing");
    }

    Ok(())
}
