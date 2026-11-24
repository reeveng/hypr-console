//! Switch between the Console desktop and CachyOS Handheld's Steam session.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_session::{DESKTOP_ENTRY, Session, SwitchBackend};
use console_waiting::clock::LiveClock;
use console_waiting::{Schedule, until_some_handed_on_result};

pub type Failure = Box<dyn std::error::Error>;

const GAME_ENTRY: &str = "gamescope-session.desktop";
const GAME_TARGET: &str = "gamescope-session.target";
const SWITCHER: &str = "/usr/local/bin/steamos-session-select";
const DESKTOP_REQUEST: &str = "plasma";
const GAME_REQUEST: &str = "gamescope";

pub struct CachyOs;

impl SwitchBackend for CachyOs {
  fn current(&self) -> Result<Session, Never> {
    let Ok(mut systemctl) = Program::Systemctl.command();

    Ok(
      match systemctl
        .args(["--user", "is-active", "--quiet", GAME_TARGET])
        .status()
      {
        Ok(status) => match status.success().then_some(()) {
          Some(()) => Session::Game,
          None => Session::Desktop,
        },
        Err(fault) => {
          console_core_logging::warn!("could not query the Steam session: {fault}");

          Session::Desktop
        }
      },
    )
  }

  fn switch_command(&self, to: Session) -> Result<Vec<String>, Never> {
    let request = match to {
      Session::Desktop => DESKTOP_REQUEST,
      Session::Game => GAME_REQUEST,
    };

    Ok(vec![SWITCHER.to_string(), request.to_string()])
  }
}

fn select(session: &str) -> io::Result<()> {
  let Ok(helper) = Program::SteamSetSession.name();
  let Ok(mut command) = Program::Pkexec.command();
  let status = command.args([helper, session]).status()?;

  match { ::core::primitive::bool::then_some(status.success(), ()) } {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(io::Error::other(format!(
      "the session selector exited with {status}"
    ))),
  }
}

fn stop_steam() -> Result<(), Never> {
  let Ok(mut steam) = Program::Steam.command();

  match steam.arg("-shutdown").status() {
    Ok(status) => match bool::then_some(status.success(), ()) {
      Some(()) => {}
      None => console_core_logging::warn!("Steam shutdown request exited with {status}"),
    },
    Err(fault) => console_core_logging::warn!("could not request Steam shutdown: {fault}"),
  }

  Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Steam {
  Running,
  Gone,
}

#[derive(Debug, PartialEq, Eq)]
struct PgrepFailure;

impl Steam {
  fn from_exit_code(code: Option<i32>) -> Result<Self, PgrepFailure> {
    match code {
      Some(0) => Ok(Self::Running),
      Some(1) => Ok(Self::Gone),
      Some(_) | None => Err(PgrepFailure),
    }
  }
}

fn steam_state(user: &str) -> io::Result<Steam> {
  let Ok(mut pgrep) = Program::Pgrep.command();
  let status = pgrep
    .args(["-u", user, "-x", "steam"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()?;
  Steam::from_exit_code(status.code())
    .map_err(|PgrepFailure| io::Error::other(format!("pgrep exited with {status}")))
}

fn steam_user() -> io::Result<u32> {
  let Ok(mut id) = Program::Id.command();
  let output = id.arg("-u").output()?;

  match bool::then_some(output.status.success(), ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(format!(
        "id exited with {}",
        output.status
      )));
    }
  }

  let user = std::str::from_utf8(&output.stdout)
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))?;
  user
    .trim()
    .parse()
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))
}

fn settle() -> io::Result<()> {
  let id = steam_user()?;
  let user = id.to_string();
  let before = steam_state(&user)?;

  match before {
    Steam::Gone => return Ok(()),
    Steam::Running => {}
  }

  let Ok(()) = stop_steam();

  let Ok(schedule) = Schedule::asking_every(Duration::from_secs(10), Duration::from_secs(1));
  let Ok(mut clock) = LiveClock::started();
  let exited = until_some_handed_on_result(&mut clock, schedule, &mut (), |()| {
    let current = steam_state(&user)?;

    Ok::<_, io::Error>(match current {
      Steam::Gone => Some(()),
      Steam::Running => None,
    })
  })?;

  match exited {
    Some(()) => {}
    None => {
      console_core_logging::debug!("Steam was still running when its shutdown deadline passed")
    }
  }

  Ok(())
}

fn game() -> Result<(), Failure> {
  match settle() {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("could not check Steam shutdown: {fault}"),
  }

  select(GAME_ENTRY)
    .map_err(|fault| format!("the session was not recorded; staying on the desktop: {fault}"))?;

  let Ok(leaving) = console_compositor::exit();

  match leaving {
    console_compositor::DispatchResult::Failure(reason) => {
      select(DESKTOP_ENTRY)?;
      return Err(Failure::from(format!(
        "the compositor did not exit and the desktop session was restored: {reason}"
      )));
    }
    console_compositor::DispatchResult::Success => {}
  }

  let Ok(mut unset) = Program::Systemctl.command();
  let _cleared = unset
    .args([
      "--user",
      "unset-environment",
      "DISPLAY",
      console_compositor::session::SIGNATURE,
      console_compositor::session::DISPLAY,
      "XDG_CURRENT_DESKTOP",
    ])
    .status();
  stop_target(console_session::TARGET)
}

fn desktop() -> Result<(), Failure> {
  select(DESKTOP_ENTRY)
    .map_err(|fault| format!("the session was not recorded; staying in game mode: {fault}"))?;

  let ::core::result::Result::Ok(()) = stop_steam();
  stop_target(GAME_TARGET)
}

fn stop_target(target: &str) -> Result<(), Failure> {
  let Ok(mut systemctl) = Program::Systemctl.command();
  Err(Failure::from(
    systemctl.args(["--user", "stop", target]).exec(),
  ))
}

pub fn switch(arguments: Vec<OsString>) -> Result<(), Failure> {
  match arguments.first().and_then(|value| OsStr::to_str(value)) {
    Some(GAME_REQUEST | "gamescope-session") => game(),
    Some(DESKTOP_REQUEST | "plasma-session") => desktop(),
    Some("persistent") => Ok(()),
    Some("oneshot") => Err(Failure::from(
      "this machine does not boot into Game Mode; see the masked unit",
    )),
    Some(_) | None => {
      let Ok(mut stock) = Program::StockSessionSelect.command();
      Err(Failure::from(stock.args(arguments).exec()))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_pgrep_no_match_means_steam_exited() {
    assert_eq!(Steam::from_exit_code(Some(0)), Ok(Steam::Running));
    assert_eq!(Steam::from_exit_code(Some(1)), Ok(Steam::Gone));
    assert_eq!(Steam::from_exit_code(Some(2)), Err(PgrepFailure));
    assert_eq!(Steam::from_exit_code(None), Err(PgrepFailure));
  }

  #[test]
  fn both_directions_use_the_installed_compatibility_switcher() {
    assert_eq!(
      CachyOs.switch_command(Session::Game),
      Ok(vec![
        "/usr/local/bin/steamos-session-select".to_string(),
        "gamescope".to_string()
      ])
    );
    assert_eq!(
      CachyOs.switch_command(Session::Desktop),
      Ok(vec![
        "/usr/local/bin/steamos-session-select".to_string(),
        "plasma".to_string()
      ])
    );
  }
}
