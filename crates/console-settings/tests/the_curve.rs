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

/// Where the manifest keeps the copy that reaches the machine.
const LIVE: &str = "files/home/@user@/.config/hypr/hyprsunset.conf";

fn tree() -> PathBuf {
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

#[test]
fn the_config_in_the_tree_is_the_curve_this_workspace_says() {
    let at = tree().join(LIVE);
    let held = std::fs::read_to_string(&at)
        .unwrap_or_else(|fault| panic!("{}: {fault}", at.display()));
    assert_eq!(
        held,
        config(),
        "{} is not what `console-warm curve` prints. Write it again:\n\
         \n    cargo run --bin console-warm -- curve > {LIVE}\n",
        at.display()
    );
}

/// The manifest is what puts it on the machine, so a file written into the
/// tree and not declared is a file the device never sees.
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
