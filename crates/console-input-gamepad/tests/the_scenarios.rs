//! The written-down scenarios, played where nothing can go wrong.
//!
//! A scenario is what somebody did with their thumbs, kept so it can be done
//! again. They are worth keeping only if they still run, and a scenario naming a
//! button that has since been renamed is a scenario nobody will find out about
//! until they reach for it.

use std::path::{Path, PathBuf};

use console_input_gamepad::capture::captured;
use console_input_gamepad::devices::Devices;
use console_input_gamepad::go::{Held, LegionGo};
use console_input_gamepad::router::every_profile;
use console_input_gamepad::script::play;
use console_input_gamepad::world::World;

fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
    let Ok(value) = answer;

    value
}

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn scenarios() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root().join("scenarios"))
        .expect("the scenarios")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "txt"))
        .collect();
    found.sort();
    found
}

#[test]
fn there_are_some() {
    assert!(!scenarios().is_empty());
}

#[test]
fn every_scenario_plays() {
    for path in scenarios() {
        let world = ok(World::of(captured().expect("the capture carried in this program parses")));
        let devices = ok(Devices::new(captured().expect("the capture"), world));
        let mut go = LegionGo::new(
            every_profile(&root()).expect("the profiles"),
            devices,
            Held::default(),
            console_input_gamepad::router::NAME,
        )
        .expect("a pad");
        let said = std::fs::read_to_string(&path).expect("a scenario");
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        play(&mut go, &said).unwrap_or_else(|fault| panic!("{name}: {fault}"));
        assert!(!go.devices.sink.log.is_empty(), "{name} pressed nothing");
    }
}
