//! The config on the machine is the curve this workspace decides.
//!
//! `hyprsunset` reads a file and this repository holds a table, and the two
//! could disagree without anything failing: the daemon would happily wear a
//! curve nobody here has written down, and the only way to notice would be to
//! be looking at the screen at the right minute on the right evening.
//!
//! So the file is not written by hand. `console-warm curve` prints it and this
//! says the tree holds exactly that, which makes an edit to the file a test
//! failure rather than a colour nobody can account for.

use std::path::{Path, PathBuf};

use console_settings::warm::config;

const LIVE: &str = "files/home/@user@/.config/hypr/hyprsunset.conf";

fn tree() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

#[test]
fn the_config_in_the_tree_is_the_curve_this_workspace_says() {
    let at = tree().join(LIVE);
    let held = std::fs::read_to_string(&at)
        .unwrap_or_else(|fault| panic!("{}: {fault}", at.display()));
    let Ok(config) = config();

    assert_eq!(
        held,
        config,
        "{} is not what `console-warm curve` prints. Write it again:\n\
         \n    cargo run --bin console-warm -- curve > {LIVE}\n",
        at.display()
    );
}

#[test]
fn the_manifest_names_the_config() {
    let manifest = tree().join("desktop.conf");
    let held = std::fs::read_to_string(&manifest).expect("desktop.conf");
    let declared = LIVE.trim_start_matches("files");
    assert!(
        held.lines().any(|line| line.trim() == declared),
        "desktop.conf does not name {declared}, so `console apply` would not lay it down"
    );
}
