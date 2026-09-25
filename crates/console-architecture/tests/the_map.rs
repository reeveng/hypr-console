//! The map in `docs/architecture/` is the one the tree draws today.
//!
//! The same shape as the palette's: the picture is written by a program, kept
//! in the tree so it can be read without running anything, and held here
//! against what the program would write now. A unit added, a service enabled
//! or an ordering changed is in `desktop.conf` and `files/`, which this reads,
//! so a map that was not drawn again is red here. What a program does is in
//! the facts, which want the lint suite's nightly to collect -- that half is
//! held by `just ready`, which collects them again and fails on a changed line.

use std::path::{Path, PathBuf};

use console_architecture::{Architecture, MAP};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    from.canonicalize().unwrap_or(from)
}

#[test]
fn the_map_is_the_one_the_tree_draws() {
    let root = root();
    let architecture = Architecture::read(&root).expect("the facts, the manifest and the units");
    let Ok(drawn) = architecture.drawn();
    let held = std::fs::read_to_string(root.join(MAP)).expect("the map");

    assert!(held == drawn, "{MAP} is not what the tree draws today: run `just map` and commit what it writes");
}

#[test]
fn every_unit_the_manifest_enables_is_on_the_map() {
    let architecture = Architecture::read(&root()).expect("the facts, the manifest and the units");
    let Ok(drawn) = architecture.drawn();
    let missing: Vec<&String> =
        architecture.enabled.iter().filter(|unit| !drawn.contains(&format!("\"unit:{unit}\""))).collect();

    assert!(missing.is_empty(), "enabled in desktop.conf and not drawn: {missing:?}");
}
