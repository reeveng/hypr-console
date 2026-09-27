//! The map in `docs/architecture/` is the one the tree draws today.
//!
//! The same shape as the palette's: the picture is written by a program, kept
//! in the tree so it can be read without running anything, and held here
//! against what the program would write now. A unit added, a service enabled
//! or an ordering changed is in `desktop.conf` and `files/`, which this reads,
//! so a map that was not drawn again is red here. What a program does is in
//! the facts, which want the lint suite's nightly to collect -- that half is
//! held by `just ready`, which collects them again and fails on a changed line.

use console_architecture::{Architecture, MAP};

type Failure = Box<dyn std::error::Error>;

#[test]
fn the_map_is_the_one_the_tree_draws() -> Result<(), Failure> {
    let root = console_repository::root()?;
    let architecture = Architecture::read(&root)?;
    let Ok(drawn) = architecture.render();
    let held = std::fs::read_to_string(root.join(MAP))?;

    assert!(held == drawn, "{MAP} is not what the tree draws today: run `just map` and commit what it writes");

    Ok(())
}

#[test]
fn every_unit_the_manifest_enables_is_on_the_map() -> Result<(), Failure> {
    let root = console_repository::root()?;
    let architecture = Architecture::read(&root)?;
    let Ok(drawn) = architecture.render();
    let missing: Vec<&String> =
        architecture.enabled.iter().filter(|unit| !drawn.contains(&format!("\"unit:{unit}\""))).collect();

    assert!(missing.is_empty(), "enabled in desktop.conf and not drawn: {missing:?}");

    Ok(())
}
