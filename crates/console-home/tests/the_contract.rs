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

/// What this surface calls itself, and what the daemon looks for.
///
/// The whole of `Mode::Home` hangs off these being one word: A opens what the
/// d-pad is standing on because the compositor lists a layer under this name.
#[test]
fn the_daemon_looks_for_the_name_this_surface_publishes() {
    let ours = read("crates/console-home/src/bin/console-home.rs");
    let theirs = read("crates/console-controller/src/mode.rs");

    assert!(
        ours.contains(r#"const NAMESPACE: &str = "console-home";"#),
        "the surface names itself somewhere else now"
    );
    assert!(
        theirs.contains(r#"pub const HOME: &str = "console-home";"#),
        "the daemon looks for another name now"
    );
}

/// And that the daemon counts it as furniture. Read as a panel it would be a
/// panel that never closed, and every job written for the desktop -- the
/// shoulders, Game Mode, the browser -- would be gone from the moment the home
/// screen started.
#[test]
fn the_home_screen_is_furniture_and_not_something_you_are_in() {
    let said = read("crates/console-controller/src/mode.rs");
    let list = said.split("pub const FURNITURE").nth(1).expect("the furniture");
    let list = list.split("];").next().expect("the end of it");

    assert!(list.contains("HOME"), "the home screen is not in FURNITURE");
}

/// The applications a first home screen fills itself with are ones this
/// machine installs. A name that has drifted is a square that never fills.
#[test]
fn the_desktops_own_applications_are_on_this_machine() {
    let entries = root().join("files/usr/share/applications");
    let said: String = std::fs::read_dir(&entries)
        .expect("the applications")
        .filter_map(Result::ok)
        .filter_map(|file| std::fs::read_to_string(file.path()).ok())
        .collect();

    for ours in console_home::OURS {
        assert!(said.contains(&format!("Name={ours}\n")), "nothing here is called {ours}");
    }
}

/// The surface is deaf to exclusive zones -- the keyboard's must not move it
/// -- so the bar's room is cleared by a margin of its own instead, and that
/// margin has to be what the bar actually reserves. Both of waybar's bars buy
/// their rows with `height`; a number drifted apart is a grid drawn under the
/// bar, or a strip of wallpaper nothing may stand on.
#[test]
fn the_room_left_for_the_bar_is_what_the_bar_reserves() {
    let bars = read("files/home/@user@/.config/waybar/config.jsonc");
    let reserved: i32 = bars
        .lines()
        .filter_map(|line| line.trim().strip_prefix("\"height\":"))
        .filter_map(|rest| rest.trim().trim_end_matches(',').parse::<i32>().ok())
        .sum();
    let ours = read("crates/console-home/src/bin/console-home.rs");

    assert!(reserved > 0, "waybar reserves no rows at all now");
    assert!(
        ours.contains(&format!("const CLEARED: i32 = {reserved};")),
        "the bar reserves {reserved} pixels and the surface clears something else"
    );
}

/// Both programs are on the machine the manifest describes, and the surface is
/// a service that is started with the desktop. A home screen that is built and
/// never started is a wallpaper.
#[test]
fn the_manifest_carries_both_programs_and_starts_the_surface() {
    let said = read("desktop.conf");

    for word in ["console-home\n", "home-place\n", "console-home.service\n"] {
        assert!(said.contains(word), "the manifest does not carry {word:?}");
    }
}
