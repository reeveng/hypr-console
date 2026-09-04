//! The written-down scenarios, played where nothing can go wrong.
//!
//! A scenario is what somebody did with their thumbs, kept so it can be done
//! again. They are worth keeping only if they still run, and a scenario naming a
//! button that has since been renamed is a scenario nobody will find out about
//! until they reach for it.

use std::path::{Path, PathBuf};

use console_pad::capture::captured;
use console_pad::devices::Devices;
use console_pad::go::{Held, LegionGo};
use console_pad::router::every_profile;
use console_pad::script::play;
use console_pad::world::World;

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

/// Every scenario in the tree, by name.
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

/// Read out of the directory rather than named here, so a new one is picked up
/// without being added to a list, and one going missing is a failure rather than
/// a silence.
#[test]
fn every_scenario_plays() {
    for path in scenarios() {
        let world = World::of(captured().expect("the capture carried in this program parses"));
        let devices = Devices::new(captured().expect("the capture carried in this program parses"), world);
        let mut go = LegionGo::new(
            every_profile(&root()).expect("the profiles"),
            devices,
            Held::default(),
            console_pad::router::NAME,
        )
        .expect("a pad");
        let said = std::fs::read_to_string(&path).expect("a scenario");
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        play(&mut go, &said).unwrap_or_else(|fault| panic!("{name}: {fault}"));
        assert!(!go.devices.sink.log.is_empty(), "{name} pressed nothing");
    }
}
