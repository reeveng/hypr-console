//! A nested desktop leaves nothing of itself running.
//!
//! The compositor is the only process this starts directly; the pool, the bar,
//! the keyboard and the wallpaper are started by `session-start` inside it, and
//! killing their compositor reaches none of them -- they are reparented to the
//! user manager and stay. Nothing about a picture that came out right says
//! whether they are still there, which is why this was found by `ps` on an
//! evening when eight hundred of them had built up rather than by anything in
//! the tree.
//!
//! So this is pressed rather than reasoned about: one run of the thing somebody
//! actually types, and then the question a person would ask afterwards -- is
//! anything still running out of that stage. The stage is named here so the
//! answer can be read off `/proc` without naming a single program, because the
//! programs are the ones the session chose and the list would go stale the
//! first time it chose another.
//!
//! Skipped and said out loud where the answer would mean nothing: a machine
//! with no user manager has no scope to put the session in, and a machine where
//! the compositor never comes up has not been asked the question at all.

use std::path::Path;
use std::process::{Command, Stdio};

use console_core_external_programs::Program;

const STAGE: &str = "a-run-that-leaves-nothing";

#[test]
fn a_nested_desktop_leaves_nothing_of_itself_running() {
    if !has_systemd_run() || !has_user_systemd() {
        eprintln!("skipped: no user manager here, so a session has no scope to be held in");
        return;
    }

    let shot = std::env::temp_dir().join("what-a-run-leaves.png");
    let _ = std::fs::remove_file(&shot);

    let done = Command::new(env!("CARGO_BIN_EXE_console-desktop"))
        .arg("shot")
        .arg(&shot)
        .env("CONSOLE_STAGE", STAGE)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let done = match done {
        Ok(done) => done,
        Err(fault) => {
            eprintln!("skipped: console-desktop would not start: {fault}");
            return;
        }
    };

    if !done.success() || !shot.exists() {
        eprintln!("skipped: the nested compositor never came up, so nothing was asked of it");
        let _ = std::fs::remove_file(&shot);
        return;
    }

    let _ = std::fs::remove_file(&shot);

    let left = still_holding(STAGE);

    assert!(
        left.is_empty(),
        "the run is over and {} processes are still running out of its stage: {}",
        left.len(),
        left.join(", ")
    );
}

fn still_holding(stage: &str) -> Vec<String> {
    let mark = format!(".stage/{stage}");
    let mut found = Vec::new();

    let entries = match std::fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(_) => return found,
    };

    for entry in entries.flatten() {
        let at = entry.path();
        let pid = match at.file_name().and_then(|name| name.to_str()) {
            Some(pid) if pid.chars().all(|digit| digit.is_ascii_digit()) => pid.to_string(),
            _ => continue,
        };
        let held = match std::fs::read(at.join("environ")) {
            Ok(held) => held,
            Err(_) => continue,
        };
        if !String::from_utf8_lossy(&held).contains(&mark) {
            continue;
        }
        let said = std::fs::read(at.join("comm")).unwrap_or_default();
        found.push(format!("{pid} {}", String::from_utf8_lossy(&said).trim()));
    }

    found
}

fn has_systemd_run() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    let Ok(named) = Program::SystemdRun.name();

    path.split(':')
        .filter(|at| !at.is_empty())
        .any(|at| Path::new(at).join(named).exists())
}

fn has_user_systemd() -> bool {
    let Ok(mut asking) = Program::Systemctl.command();

    asking
        .args(["--user", "show", "-p", "Version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|how| how.success())
        .unwrap_or(false)
}
