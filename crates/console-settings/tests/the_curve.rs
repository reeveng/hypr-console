//! The config on the machine is the curve this workspace decides.
//!
//! `hyprsunset` reads a file and this repository holds a table, and the two
//! could disagree without anything failing: the daemon would happily wear a
//! curve no one here has written down, and the only way to notice would be to
//! be looking at the screen at the right minute on the right evening.
//!
//! So the file is not written by hand. `console-warm curve` prints it and this
//! says the tree holds exactly that, which makes an edit to the file a test
//! failure rather than a color no one can account for.

use std::error::Error;
use std::path::Path;

use console_settings::warm::configuration;

const LIVE: &str = "files/home/@user@/.config/console/hypr/hyprsunset.conf";

const TREE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

#[test]
fn the_config_in_the_tree_is_the_curve_this_workspace_says() -> Result<(), Box<dyn Error>> {
    let at = Path::new(TREE).join(LIVE);
    let held = std::fs::read_to_string(&at)?;
    let Ok(configuration) = configuration();

    assert_eq!(
        held,
        configuration,
        "{} is not what `console-warm curve` prints. Write it again:\n\
         \n    cargo run --bin console-warm -- curve > {LIVE}\n",
        at.display()
    );

    Ok(())
}

#[test]
fn the_manifest_names_the_config() -> Result<(), Box<dyn Error>> {
    let held = std::fs::read_to_string(Path::new(TREE).join("desktop.conf"))?;
    let declared = LIVE.trim_start_matches("files");

    assert!(
        held.lines().any(|line| line.trim() == declared),
        "desktop.conf does not name {declared}, so `console apply` would not lay it down"
    );

    Ok(())
}
