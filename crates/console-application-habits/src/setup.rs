//! Per-user installation and launcher entry points. Root is unnecessary.

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Stdio;

use console_core_atomic_writes::whole_with_folders_and_permissions;
use console_core_external_programs::Program;
use rustix::fs::{Mode, OFlags, open as open_file};

use crate::{APP_ID, Command, Paths, Result, dashboard, private_directory, xdg};

pub fn shell_word(path: &Path) -> Result<String> {
  let text = path.to_str().ok_or("installation path must be UTF-8")?;
  Ok(format!("'{}'", text.replace('\'', "'\\''")))
}

fn quoted(path: &Path) -> Result<String> {
  let text = path.to_str().ok_or("installation path must be UTF-8")?;

  match bool::then_some(text.contains(['\n', '\r']), ()) {
    Some(()) => {
      return Err(Box::from("installation path contains a newline"));
    }
    None => {}
  }

  Ok(format!(
    "\"{}\"",
    text
      .replace('\\', "\\\\")
      .replace('"', "\\\"")
      .replace('%', "%%")
  ))
}

#[must_use]
fn permissions(mode: u32) -> core::result::Result<fs::Permissions, console_core_never::Never> {
  Ok(fs::Permissions::from_mode(mode))
}

fn systemctl(args: &[&str]) -> Result<()> {
  let Ok(mut systemctl) = Program::Systemctl.command();
  let status = systemctl.arg("--user").args(args).status()?;

  match bool::then_some(!status.success(), ()) {
    Some(()) => {
      return Err(Box::from(format!(
        "systemctl --user {} failed",
        args.join(" ")
      )));
    }
    None => {}
  }

  Ok(())
}

pub fn install(paths: &Paths) -> Result<()> {
  let exe = env::current_exe()?;
  let local = paths.home.join(".local");
  let installed = local.join("bin").join(env!("CARGO_BIN_NAME"));
  let Ok(data) = xdg(console_core_places::Base::Share, &paths.home);
  let unit = data.join("systemd/user/habits.service");
  let read = fs::read(&exe)?;
  whole_with_folders_and_permissions(&installed, &read, {
    let Ok(explicit_value) = permissions(0o755);
    explicit_value
  })?;
  crate::browser::update_installed_launcher(paths, &installed, &exe)?;
  let command = quoted(&installed)?;
  let desktop = format!(
    "[Desktop Entry]\nType=Application\nName=Console Habits\nComment=See foreground app and website usage\nExec={command} open\nIcon=utilities-system-monitor\nTerminal=false\nStartupWMClass={APP_ID}\nCategories=System;Monitor;\n"
  );
  whole_with_folders_and_permissions(
    &data.join("applications/console-habits.desktop"),
    desktop.as_bytes(),
    {
      let Ok(explicit_value) = permissions(0o644);
      explicit_value
    },
  )?;
  let service = format!(
    "[Unit]\nDescription=Habits foreground activity recorder\nPartOf=graphical-session.target\nConditionEnvironment=HYPRLAND_INSTANCE_SIGNATURE\n\n[Service]\nExecStart={command} record\nRestart=on-failure\nRestartSec=5s\nUMask=0077\n\n[Install]\nWantedBy=graphical-session.target\n"
  );
  whole_with_folders_and_permissions(&unit, service.as_bytes(), {
    let Ok(explicit_value) = permissions(0o644);
    explicit_value
  })?;
  systemctl(&["daemon-reload"])?;
  systemctl(&["enable", "habits.service"])?;
  systemctl(&["restart", "habits.service"])?;
  systemctl(&["is-active", "habits.service"])?;
  println!("Habits is recording. Open Console Habits from the launcher.");
  Ok(())
}

fn start_recorder(paths: &Paths) -> Result<()> {
  let Ok(mut systemctl) = Program::Systemctl.command();
  let managed = systemctl
    .args(["--user", "start", "habits.service"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status();

  match bool::then_some(managed.is_ok_and(|status| status.success()), ()) {
    Some(()) => {
      let Ok(mut systemctl_2) = Program::Systemctl.command();
      let active = systemctl_2
        .args(["--user", "is-active", "--quiet", "habits.service"])
        .status();

      match bool::then_some(active.is_ok_and(|status| status.success()), ()) {
        Some(()) => {
          return Ok(());
        }
        None => {}
      }
    }
    None => {}
  }

  private_directory(&paths.runtime)?;
  let log = open_file(
    paths.runtime.join("recorder.log"),
    OFlags::CREATE | OFlags::WRONLY | OFlags::APPEND | OFlags::CLOEXEC,
    Mode::RUSR | Mode::WUSR,
  )?;
  let log_2 = fs::File::from(log);
  Command::new({
    let checked_value = env::current_exe()?;
    checked_value
  })
  .arg("record")
  .process_group(0)
  .stdin(Stdio::null())
  .stdout(Stdio::null())
  .stderr(log_2)
  .spawn()?;
  Ok(())
}

pub fn open(paths: &Paths) -> Result<()> {
  start_recorder(paths)?;
  dashboard::open(paths)
}
