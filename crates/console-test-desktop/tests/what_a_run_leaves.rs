//! A nested desktop leaves nothing of itself running.
//!
//! The pool, the bar and the keyboard are started by `session-start`, and
//! killing their compositor reaches none of them -- they are reparented to the
//! user manager and stay. Nothing about a picture that came out right says
//! whether they are still there, which is why this was found by `ps` on an
//! evening when eight hundred of them had built up rather than by anything in
//! the tree.
//!
//! So this is pressed rather than reasoned about: one run of the thing someone
//! actually types, and then the question a person would ask afterwards -- is
//! anything still running out of that stage. The stage is named here so the
//! answer can be read off `/proc` without naming a single program, because the
//! programs are the ones the session chose and the list would go stale the
//! first time it chose another.
//! The wallpaper is owned directly by the runner and carries the same staged
//! environment, so it is included in the question even outside that scope.
//!
//! The VM must have a user manager and be able to take the desktop screenshot.

use std::error::Error;
use std::io;
use std::process::{Command, Stdio};

use console_core_never::Never;
use console_operating_system_linux::systemd::{Scopes, scopes};

const STAGE: &str = "a-run-that-leaves-nothing";

#[test]
#[ignore = "run just test e2e-desktop in a disposable NixOS VM"]
fn a_nested_desktop_leaves_nothing_of_itself_running() -> Result<(), Box<dyn Error>> {
  let _ = std::fs::metadata("/etc/console-test-vm").map_err(|fault| {
    io::Error::new(
      fault.kind(),
      "this test requires a VM. Run just test e2e-desktop",
    )
  })?;
  let Ok(held) = scopes();

  match held {
    Scopes::Available => {}
    Scopes::Unavailable => {
      return Err(Box::<dyn Error>::from(io::Error::other(
        "the guest has no user manager",
      )));
    }
  }

  let folder = console_core_temporary_directories::fresh("what-a-run-leaves")?;
  let shot = folder.join("what-a-run-leaves.png");

  let done = Command::new(env!("CARGO_BIN_EXE_console-desktop"))
    .arg("shot")
    .arg(&shot)
    .env("CONSOLE_STAGE", STAGE)
    .stdout(Stdio::null())
    .stderr(Stdio::inherit())
    .status();
  let captured = std::fs::remove_file(&shot);
  console_test_desktop::session::remove_tree(&folder)?;

  let status = done?;

  match status.success().then_some(()) {
    Some(()) => {}
    None => {
      return Err(Box::<dyn Error>::from(io::Error::other(format!(
        "the nested compositor exited with {status}"
      ))));
    }
  }

  captured?;
  let Ok(left) = still_holding(STAGE);

  assert!(
    left.is_empty(),
    "the run is over and {} processes are still running out of its stage: {}",
    left.len(),
    left.join(", ")
  );

  Ok(())
}

fn still_holding(stage: &str) -> Result<Vec<String>, Never> {
  let stage_path = format!(".stage/{stage}");
  let mark = stage_path.as_bytes();
  let mut found = Vec::new();

  let entries = match std::fs::read_dir("/proc") {
    Ok(entries) => entries,
    Err(_unlisted) => return Ok(found),
  };

  for entry in entries.flatten() {
    let at = entry.path();

    let pid = match at.file_name().and_then(|name| name.to_str()) {
      Some(pid) => pid.to_string(),
      None => continue,
    };

    match ::core::primitive::bool::then_some(pid.chars().all(|digit| digit.is_ascii_digit()), ())
    {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => continue,
    }

    let held = match std::fs::read(at.join("environ")) {
      Ok(held) => held,
      Err(_ended_or_not_ours) => continue,
    };

    let in_stage = held.windows(mark.len().max(1)).any(|bytes| bytes == mark);

    match ::core::primitive::bool::then_some(in_stage, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => continue,
    }

    let said = match std::fs::read(at.join("comm")) {
      Ok(said) => match std::str::from_utf8(&said) {
        Ok(text) => text.trim().to_string(),
        Err(_invalid_text) => format!("{said:?}"),
      },
      Err(_ended) => "(ended while it was asked)".to_string(),
    };

    found.push(format!("{pid} {said}"));
  }

  Ok(found)
}
