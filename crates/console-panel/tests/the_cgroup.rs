//! A launched application is in a cgroup of its own, not the controller's.
//!
//! The menu, the panel, the music panel and the file manager are all started
//! by `console_panel::running::left_running`. Each one of them is a process
//! whose life is a single press, and what is started under it (the player, the
//! browser, the file viewer) is a process that has to outlive it. Under cgroup
//! v2 a child inherits its parent's cgroup, so without an explicit move the
//! launched program sits in `console-controller.service`'s cgroup. Restarting
//! the controller then takes the program with it, which is the harm the entry
//! in `todos.md` describes.
//!
//! `left_running` wraps the program in `systemd-run --user --scope`, which
//! moves the child into a transient scope unit named `run-<pid>-<id>.scope`.
//! This test runs the same wrap against `/bin/sleep` and reads the child's
//! cgroup path back out of `/proc/<pid>/cgroup`, asserting the path ends in a
//! scope rather than the controller's slice.
//!
//! Skipped, not failed, on a machine without `systemd-run` or without a user
//! systemd to talk to. Both are common in a CI environment and neither is a
//! reason to fail the rest of the suite.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use console_panel::running::scope_around;

/// A launched program is in its own scope, named by `systemd-run`.
///
/// Reads the cgroup path back out of `/proc` and asserts it ends in a scope
/// unit. A path that just shows the parent's slice is a launched process that
/// is in the controller's cgroup, which is the bug this test exists to catch.
#[test]
fn a_launched_program_is_in_a_scope_of_its_own() {
    let Some(argv) = wrap("sleep", &[HELD]) else {
        return;
    };
    let Some(mut child) = run(&argv) else {
        return;
    };
    let pid = child.id();
    let cgroup = settled(pid);
    let _ = child.kill();
    let _ = child.wait();
    let Some(cgroup) = cgroup else {
        panic!(
            "/proc/{pid}/cgroup could not be read, though the program was given {HELD} seconds \
             to be there. Something ended it early, and this test has asked nothing."
        )
    };
    assert!(
        moved(&cgroup),
        "the launched program is in the parent's cgroup, not a run-p scope: {cgroup}"
    );
}

/// The argv `left_running` would build for a program named `name` and given
/// `args`. None of this runs; the helper just returns the argv and `None` if
/// the prerequisites for a scope are not here.
fn wrap(name: &str, args: &[&str]) -> Option<Vec<String>> {
    if !has_systemd_run() {
        eprintln!("skipped: no systemd-run on PATH; scopes cannot be made");
        return None;
    }
    if !has_user_systemd() {
        eprintln!("skipped: no user systemd to talk to; scopes cannot be made");
        return None;
    }
    let argv: Vec<String> = std::iter::once(name.to_string())
        .chain(args.iter().map(|word| (*word).to_string()))
        .collect();
    let (_, wrapped) = scope_around(&argv);
    Some(wrapped)
}

/// How long the program is held open, in seconds, so that reading its cgroup
/// cannot race it ending. Killed the moment the reading is done, so this is a
/// ceiling and not a wait.
const HELD: &str = "5";

/// Start the wrapped program and hand back the child itself, so it can be
/// killed and reaped rather than left behind.
///
/// The pid is the program's own, not a wrapper's: `systemd-run --scope` makes
/// the scope and then becomes the program, so the process that was spawned here
/// is the one that ends up inside the scope. Watched rather than assumed --
/// `/proc/<pid>/comm` says `sleep`.
fn run(argv: &[String]) -> Option<Child> {
    Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// Whether a cgroup path is a scope `systemd-run` made.
///
/// It names them `run-p<pid>-i<id>.scope`. An ordinary scope -- the one a
/// graphical app is launched into, which is what a child would inherit -- is
/// named for the app instead, so the `run-` prefix is the mark of the wrap and
/// not merely of being in a scope at all.
fn moved(cgroup: &str) -> bool {
    let segment = cgroup.rsplit('/').next().unwrap_or("");
    segment.starts_with("run-") && segment.ends_with(".scope")
}

/// How long the move into the scope is given.
const MOVES: Duration = Duration::from_secs(5);

/// The program's cgroup once it has stopped being its parent's.
///
/// `systemd-run` is spawned into the cgroup of whoever spawned it and only asks
/// the manager for a scope afterwards, so for the first moments of its life the
/// honest answer is the parent's. Reading once and asserting is a race, and it
/// is one this test lost on the machine it was written on: it read the terminal's
/// own `app-Hyprland-...` scope and called it a bug in `left_running`.
///
/// So it is watched rather than sampled. Whatever was read last is handed back
/// when the move never comes, because that is the thing worth printing.
fn settled(pid: u32) -> Option<String> {
    let until = Instant::now() + MOVES;
    let mut last = None;
    while Instant::now() < until {
        let now = read_cgroup(pid);
        if now.as_deref().is_some_and(moved) {
            return now;
        }
        last = now.or(last);
        std::thread::sleep(Duration::from_millis(50));
    }
    last
}

/// The cgroup path of a process, or nothing if the process has gone.
fn read_cgroup(pid: u32) -> Option<String> {
    let at = format!("/proc/{pid}/cgroup");
    let said = std::fs::read_to_string(at).ok()?;
    // The v2 file holds a single line, "0::/path/to/cgroup". Earlier
    // controllers sit on their own lines, but the path we want is the v2 one
    // because that is the cgroup systemd uses.
    let v2 = said.lines().find(|line| line.starts_with("0::"))?;
    Some(v2.trim_start_matches("0::").to_string())
}

fn has_systemd_run() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .filter(|at| !at.is_empty())
        .any(|at| Path::new(at).join("systemd-run").exists())
}

/// Whether there is a user systemd on the other end to make a scope.
///
/// Asked by making it say its version, which is the smallest question it
/// answers. It used to be asked `is-active` with nothing to be active about,
/// which is "Too few arguments" and a failure every time -- so this test
/// skipped on every machine it has ever run on, including the two it was
/// written on. `is-system-running` is the other tempting answer and is worse:
/// it fails on a session that is merely degraded, which most are.
fn has_user_systemd() -> bool {
    Command::new("systemctl")
        .args(["--user", "show", "-p", "Version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|how| how.success())
        .unwrap_or(false)
}
