//! The places outside this crate that have to agree with it.
//!
//! The home screen is a name to the compositor, two programs in the manifest,
//! and a handful of applications it puts on a machine that has never drawn
//! one. Each of those is written down somewhere else as well, and a name
//! changed on one side and not the other fails in the quietest way there is:
//! the daemon never sees a home screen, so A stays a click; or the first pane
//! fills with a hole where an application used to be.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the tree")
}

fn read(what: &str) -> String {
    let at = root().join(what);

    std::fs::read_to_string(&at).unwrap_or_else(|fault| panic!("{}: {fault}", at.display()))
}

#[test]
fn the_daemon_looks_for_the_name_this_surface_publishes() {
    let ours = read("crates/console-home-screen/src/bin/console-home.rs");
    let theirs = read("crates/console-onscreen/src/lib.rs");

    assert!(
        ours.contains(r#"const NAMESPACE: &str = "console-home";"#),
        "the surface names itself somewhere else now"
    );
    assert!(
        theirs.contains(r#"pub const HOME: &str = "console-home";"#),
        "the desktop looks for another name now"
    );
}

#[test]
fn the_home_screen_is_furniture_and_not_something_you_are_in() {
    let said = read("crates/console-onscreen/src/lib.rs");
    let list = said.split("pub const FURNITURE").nth(1).expect("the furniture");
    let list = list.split("];").next().expect("the end of it");

    assert!(list.contains("HOME"), "the home screen is not in FURNITURE");
}

#[test]
fn the_wallpaper_asks_the_same_question_rather_than_keeping_its_own_list() {
    let said = read("crates/console-wallpaper/src/covered.rs");

    assert!(
        said.contains("pub use console_onscreen::FURNITURE as BEHIND;"),
        "the wallpaper keeps its own list of what is allowed to be behind again"
    );
    assert!(
        !said.contains("pub const BEHIND"),
        "the wallpaper spells the list out again rather than asking for it"
    );
}

#[test]
fn the_desktops_own_applications_are_on_this_machine() {
    let entries = root().join("files/usr/share/applications");
    let said: String = std::fs::read_dir(&entries)
        .expect("the applications")
        .filter_map(Result::ok)
        .filter_map(|file| std::fs::read_to_string(file.path()).ok())
        .collect();

    for ours in console_home_screen::OURS {
        assert!(said.contains(&format!("Name={ours}\n")), "nothing here is called {ours}");
    }
}

#[test]
fn the_room_left_for_the_bar_is_what_the_bar_reserves() {
    let bars = read("files/home/@user@/.config/waybar/config.jsonc");
    let reserved: i32 = bars
        .lines()
        .filter_map(|line| line.trim().strip_prefix("\"height\":"))
        .filter_map(|rest| rest.trim().trim_end_matches(',').parse::<i32>().ok())
        .sum();
    let ours = read("crates/console-home-screen/src/bin/console-home.rs");

    assert!(reserved > 0, "waybar reserves no rows at all now");
    assert!(
        ours.contains(&format!("const CLEARED: i32 = {reserved};")),
        "the bar reserves {reserved} pixels and the surface clears something else"
    );
}

#[test]
fn the_manifest_carries_both_programs_and_starts_the_surface() {
    let said = read("desktop.conf");

    for word in ["console-home\n", "home-square\n", "console-home.service\n"] {
        assert!(said.contains(word), "the manifest does not carry {word:?}");
    }
}
