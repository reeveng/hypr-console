//! Offers the shared Documents and Pictures to the devices paired with Syncthing.

use console_core_external_programs::Program;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Error};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

type Failure = Box<dyn std::error::Error>;

fn cli(arguments: &[&str]) -> Command {
  let Ok(mut command) = Program::Syncthing.command();
  command.args(["cli", "config"]).args(arguments);
  command
}

fn apply(command: &mut Command) -> io::Result<()> {
  let status = command.status()?;
  if status.success() {
    Ok(())
  } else {
    Err(Error::other(format!("Syncthing exited with {status}")))
  }
}

fn list(arguments: &[&str]) -> Result<Vec<String>, Failure> {
  let output = cli(arguments).stderr(Stdio::inherit()).output()?;
  if !output.status.success() {
    return Err(
      format!(
        "Syncthing could not list {} ({})",
        arguments.join(" "),
        output.status
      )
      .into(),
    );
  }
  let printed = String::from_utf8(output.stdout)?;
  Ok(printed.lines().map(str::to_owned).collect())
}

fn wait_for_syncthing() -> Result<(), Failure> {
  for waited in 0..=60 {
    let Ok(mut command) = Program::Syncthing.command();
    let status = command
      .args(["cli", "show", "system"])
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()?;
    if status.success() {
      return Ok(());
    }
    if waited < 60 {
      thread::sleep(Duration::from_secs(1));
    }
  }
  Err("Syncthing did not become ready".into())
}

fn offer_folder(shared: &Path, id: &str, label: &str, devices: &[String]) -> Result<(), Failure> {
  let folder = shared.join(label);
  fs::create_dir_all(&folder)?;
  if !list(&["folders", "list"])?
    .iter()
    .any(|existing| existing == id)
  {
    apply(cli(&["folders", "add", "--id", id, "--label", label, "--path"]).arg(&folder))?;
  }

  let paired = list(&["folders", id, "devices", "list"])?;
  let mut failures = 0;
  for device in devices {
    if !paired.contains(device) {
      let changed = apply(&mut cli(&[
        "folders",
        id,
        "devices",
        "add",
        "--device-id",
        device,
      ]));
      if let Err(fault) = changed {
        eprintln!("Could not offer {label} to {device}: {fault}");
        failures += 1;
      }
    }
  }
  if failures > 0 {
    return Err(format!("{failures} device(s) could not be offered {label}").into());
  }
  Ok(())
}

fn run(shared: &Path) -> Result<(), Failure> {
  wait_for_syncthing()?;
  let devices = list(&["devices", "list"])?;
  let mut failed = false;

  if let Err(fault) = apply(cli(&["defaults", "folder", "path", "set"]).arg(shared)) {
    eprintln!("Could not set the default folder path: {fault}");
    failed = true;
  }
  for device in &devices {
    if let Err(fault) = apply(&mut cli(&[
      "devices",
      device,
      "auto-accept-folders",
      "set",
      "true",
    ])) {
      eprintln!("Could not accept folders from {device}: {fault}");
      failed = true;
    }
  }
  for (id, label) in [
    ("shared-documents", "Documents"),
    ("shared-pictures", "Pictures"),
  ] {
    if let Err(fault) = offer_folder(shared, id, label, &devices) {
      eprintln!("Could not configure {label}: {fault}");
      failed = true;
    }
  }

  if failed {
    return Err("some shared-folder settings could not be applied".into());
  }
  Ok(())
}

fn main() -> Result<(), Failure> {
  let home = std::env::var_os("HOME")
    .filter(|path| path != OsStr::new(""))
    .ok_or("HOME is required for the shared folders")?;
  run(&PathBuf::from(home).join("Shared"))
}
