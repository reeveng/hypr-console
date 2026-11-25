//! The full-screen shortcut used by the controller and keyboard.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{ExitCode, Stdio};

use console_core_external_programs::{Installed, Program, installed};
use console_notifications::delivery::{Action, Options};

const SEND_KEY: &str = "send";
const SEND: Action<'static> = Action {
  key: SEND_KEY,
  label: "Send to device",
};

fn main() -> ExitCode {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();

  match run(&arguments) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::error!("console-screenshot: {fault}");
      ExitCode::FAILURE
    }
  }
}

fn run(arguments: &[OsString]) -> io::Result<()> {
  match arguments {
    [] => capture(),
    [notice, path] => match notice.to_str() {
      Some("--notice-copied") => {
        announce(Path::new(path), "Screenshot saved and copied to clipboard")
      }
      Some("--notice-saved") => announce(
        Path::new(path),
        "Screenshot saved to Pictures. Clipboard unavailable.",
      ),
      Some(_) | None => Err(io::Error::other("invalid screenshot notice")),
    },
    [_] | [_, _, _, ..] => Err(io::Error::other("invalid screenshot arguments")),
  }
}

fn capture() -> io::Result<()> {
  let shot = console_application_camera::screenshot(None)?;
  println!("{}", shot.path.display());

  match shot.announce() {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("couldn’t announce screenshot: {fault}"),
  }

  Ok(())
}

fn announce(path: &Path, message: &str) -> io::Result<()> {
  let Ok(program) = Program::LocalSend.name();
  let Ok(installed) = installed(program);
  let action = match (path.is_absolute(), path.to_str(), installed) {
    (true, Some(_), Installed::Yes) => Some(SEND),
    _ => None,
  };
  let options = Options {
    app_name: Some("Console"),
    expire_ms: Some(10_000),
    ..Default::default()
  };
  let Ok(mut notice) = match action {
    Some(offered) => options.action_command(message, offered),
    None => options.command(message, None),
  };
  let answer = notice.stdin(Stdio::null()).output()?;

  match answer.status.success().then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(format!(
        "notification exited with {}",
        answer.status
      )));
    }
  }

  let selected = std::str::from_utf8(&answer.stdout).map_err(io::Error::other)?;

  match (action, selected.trim()) {
    (Some(_), SEND_KEY) => share(path),
    (Some(_) | None, _) => Ok(()),
  }
}

fn share(path: &Path) -> io::Result<()> {
  let Ok(program) = Program::LocalSend.name();
  let Ok(mut service) = Program::SystemdRun.command();
  let status = service
    .args([
      "--user",
      "--quiet",
      "--collect",
      "--expand-environment=no",
      "--",
      program,
    ])
    .arg(path)
    .status()?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => {
      let Ok(mut failed) = Options::default().command("Could not open LocalSend", None);
      let _posted = failed.status();
      Err(io::Error::other(format!(
        "LocalSend launch exited with {status}"
      )))
    }
  }
}
