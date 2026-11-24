//! Offers the shared Documents and Pictures to the devices paired with Syncthing.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Error};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_waiting::{Schedule, clock::LiveClock, until_some_handed_on_result};

type Failure = Box<dyn std::error::Error>;

#[derive(Clone, Copy, Debug)]
enum SharedFolder {
  Documents,
  Pictures,
}

fn cli(arguments: &[&str]) -> Result<Command, Never> {
  let Ok(mut command) = Program::Syncthing.command();
  command.args(["cli", "config"]).args(arguments);

  Ok(command)
}

fn apply(command: &mut Command) -> io::Result<()> {
  let status = command.status()?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(Error::other(format!("Syncthing exited with {status}"))),
  }
}

fn list(arguments: &[&str]) -> Result<Vec<String>, Failure> {
  let Ok(mut command) = cli(arguments);
  let output = command.stderr(Stdio::inherit()).output()?;

  match (!output.status.success()).then_some(()) {
    Some(()) => {
      return Err(Failure::from(format!(
        "Syncthing could not list {} ({})",
        arguments.join(" "),
        output.status
      )));
    }
    None => {}
  }

  let printed = String::from_utf8(output.stdout)?;
  Ok(printed.lines().map(str::to_owned).collect())
}

fn wait_for_syncthing() -> Result<(), Failure> {
  let Ok(mut clock) = LiveClock::started();
  let Ok(schedule) = Schedule::asking_every(Duration::from_secs(60), Duration::from_secs(1));
  let Ok(mut probe) = Program::Syncthing.command();
  probe
    .args(["cli", "show", "system"])
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  let ready = until_some_handed_on_result(&mut clock, schedule, &mut probe, |command| {
    let status = command.status()?;

    Ok::<_, io::Error>(status.success().then_some(()))
  })?;

  ready.ok_or_else(|| Failure::from("Syncthing did not become ready"))
}

fn offer_folder(shared: &Path, choice: SharedFolder, devices: &[String]) -> Result<(), Failure> {
  let (id, label, folder) = match choice {
    SharedFolder::Documents => ("shared-documents", "Documents", shared.join("Documents")),
    SharedFolder::Pictures => ("shared-pictures", "Pictures", shared.join("Pictures")),
  };
  fs::create_dir_all(&folder)?;
  let folders = list(&["folders", "list"])?;

  match folders.iter().find(|existing| existing.as_str() == id) {
    Some(_) => {}
    None => {
      let Ok(mut add) = cli(&["folders", "add", "--id", id, "--label", label, "--path"]);
      apply(add.arg(&folder))?;
    }
  }

  let paired = list(&["folders", id, "devices", "list"])?;
  let paired_devices: HashSet<_> = paired.into_iter().collect();
  let mut failures = 0u32;

  for device in devices {
    match paired_devices.get(device) {
      Some(_) => {}
      None => {
        let Ok(mut share) = cli(&["folders", id, "devices", "add", "--device-id", device]);

        match apply(&mut share) {
          Err(fault) => {
            eprintln!("Could not offer {label} to {device}: {fault}");
            failures = failures.saturating_add(1);
          }
          Ok(()) => {}
        }
      }
    }
  }

  match failures {
    0 => Ok(()),
    _ => Err(Failure::from(format!(
      "{failures} device(s) could not be offered {label}"
    ))),
  }
}

fn run(shared: &Path) -> Result<(), Failure> {
  wait_for_syncthing()?;
  let devices = list(&["devices", "list"])?;
  let mut failed = false;

  let Ok(mut default_path) = cli(&["defaults", "folder", "path", "set"]);

  match apply(default_path.arg(shared)) {
    Err(fault) => {
      eprintln!("Could not set the default folder path: {fault}");
      failed = true;
    }
    Ok(()) => {}
  }

  for device in &devices {
    let Ok(mut accept) = cli(&["devices", device, "auto-accept-folders", "set", "true"]);

    match apply(&mut accept) {
      Err(fault) => {
        eprintln!("Could not accept folders from {device}: {fault}");
        failed = true;
      }
      Ok(()) => {}
    }
  }

  for folder in [SharedFolder::Documents, SharedFolder::Pictures] {
    match offer_folder(shared, folder, &devices) {
      Err(fault) => {
        eprintln!("Could not configure {folder:?}: {fault}");
        failed = true;
      }
      Ok(()) => {}
    }
  }

  match failed.then_some(()) {
    Some(()) => Err(Failure::from(
      "some shared-folder settings could not be applied",
    )),
    None => Ok(()),
  }
}

fn main() -> Result<(), Failure> {
  let Ok(configured) = console_core_places::home();
  let home = configured
    .filter(|path| !path.as_os_str().is_empty())
    .ok_or("HOME is required for the shared folders")?;
  run(&home.join("Shared"))
}
