//! Uses SteamOS's session selector with Console's compositor and services.

use console_core_external_programs::Program;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

type Failure = Box<dyn std::error::Error>;

const CONSOLE: &str = "console.desktop";
const GAME: &str = "gamescope-session.desktop";

fn select(session: &str) -> io::Result<()> {
  let Ok(helper) = Program::SteamSetSession.name();
  let Ok(mut command) = Program::Pkexec.command();
  let status = command.args([helper, session]).status()?;
  if status.success() {
    Ok(())
  } else {
    Err(io::Error::other(format!(
      "the session selector exited with {status}"
    )))
  }
}

fn stop_steam() {
  let Ok(mut steam) = Program::Steam.command();
  let _asked = steam.arg("-shutdown").status();
}

fn steam_running(user: &str) -> bool {
  let Ok(mut pgrep) = Program::Pgrep.command();
  pgrep
    .args(["-u", user, "-x", "steam"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .is_ok_and(|status| status.success())
}

fn settle() {
  let Ok(mut id) = Program::Id.command();
  let Ok(output) = id.arg("-u").output() else {
    return;
  };
  let Ok(user) = std::str::from_utf8(&output.stdout) else {
    return;
  };
  let user = user.trim();
  if !output.status.success() || !steam_running(user) {
    return;
  }

  // Asking an absent Steam to shut down would start it. Give a running Steam
  // ten seconds to leave cleanly before the compositor closes.
  stop_steam();
  for _ in 0..10 {
    if !steam_running(user) {
      break;
    }
    thread::sleep(Duration::from_secs(1));
  }
}

fn game() -> Result<(), Failure> {
  settle();
  select(GAME)
    .map_err(|fault| format!("the session was not recorded; staying on the desktop: {fault}"))?;

  let Ok(mut hyprctl) = Program::Hyprctl.command();
  let leaving = hyprctl.args(["dispatch", "hl.dsp.exit()"]).status();
  if !leaving.is_ok_and(|status| status.success()) {
    select(CONSOLE)?;
    return Err("the compositor did not exit; restored the desktop session".into());
  }

  let Ok(mut unset) = Program::Systemctl.command();
  let _cleared = unset
    .args([
      "--user",
      "unset-environment",
      "DISPLAY",
      "HYPRLAND_INSTANCE_SIGNATURE",
      "WAYLAND_DISPLAY",
      "XDG_CURRENT_DESKTOP",
    ])
    .status();
  stop_target("console.target")
}

fn desktop() -> Result<(), Failure> {
  select(CONSOLE)
    .map_err(|fault| format!("the session was not recorded; staying in game mode: {fault}"))?;

  // The Game Mode unit restarts Steam until its target stops.
  stop_steam();
  stop_target("gamescope-session.target")
}

fn stop_target(target: &str) -> Result<(), Failure> {
  let Ok(mut systemctl) = Program::Systemctl.command();
  Err(systemctl.args(["--user", "stop", target]).exec().into())
}

fn main() -> Result<(), Failure> {
  let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
  match arguments.first().and_then(|value| OsStr::to_str(value)) {
    Some("gamescope" | "gamescope-session") => game(),
    Some("plasma" | "plasma-session") => desktop(),
    Some("persistent") => Ok(()),
    Some("oneshot") => {
      Err("this machine does not boot into Game Mode; see the masked unit".into())
    }
    _ => {
      let Ok(mut stock) = Program::StockSessionSelect.command();
      Err(stock.args(arguments).exec().into())
    }
  }
}
