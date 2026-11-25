//! Installed mise tool versions and the registry used for standalone installs.

use std::io;
use std::process::{Command, Stdio};

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;

pub fn availability() -> Result<Installed, Never> {
  let Ok(name) = Program::Mise.name();

  installed(name)
}

fn standalone() -> io::Result<Command> {
  let mut command = Program::Mise.provide()?;
  let home =
    std::env::home_dir().ok_or_else(|| io::Error::other("Home directory is unavailable"))?;
  command
    .current_dir(home)
    .env("NO_COLOR", "1")
    .env("MISE_AUTO_INSTALL", "0")
    .env("MISE_SELF_UPDATE_AUTO", "0")
    .env("MISE_SYSTEM_DEPS", "warn")
    .stdin(Stdio::null());

  Ok(command)
}

fn invoke(arguments: &[&str]) -> io::Result<String> {
  let mut mise = standalone()?;
  let output = mise.args(arguments).output()?;

  match output.status.success().then_some(()) {
    Some(()) => String::from_utf8(output.stdout).map_err(io::Error::other),
    None => {
      let detail = match std::str::from_utf8(&output.stderr) {
        Ok(message) => message.trim().to_owned(),
        Err(fault) => format!("Invalid UTF-8 in stderr: {fault}"),
      };

      Err(io::Error::other(format!(
        "mise exited with {}. {detail}",
        output.status
      )))
    }
  }
}

pub fn installed_versions() -> io::Result<Vec<(String, String)>> {
  let text = invoke(&["ls", "--installed", "--no-header", "--no-truncate"])?;

  Ok(
    text
      .lines()
      .filter_map(|line| {
        let mut columns = line.split_whitespace();
        let name = columns.next()?;
        let version = columns.next()?;

        match (version != "system" && !version.starts_with("path:")).then_some(()) {
          Some(()) => Some((name.to_owned(), version.to_owned())),
          None => None,
        }
      })
      .collect(),
  )
}

pub fn registry() -> io::Result<Vec<String>> {
  let text = invoke(&["registry", "--hide-aliased"])?;

  Ok(
    text
      .lines()
      .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
      .collect(),
  )
}

pub fn install(tool: &str) -> io::Result<()> {
  let spec = format!("{tool}@latest");
  let _diagnostics = invoke(&["install", "--", &spec])?;

  Ok(())
}

pub struct Version<'a>(pub &'a str);

pub fn remove(tool: &str, Version(version): Version<'_>) -> io::Result<()> {
  let spec = format!("{tool}@{version}");
  let _diagnostics = invoke(&["uninstall", "--", &spec])?;

  Ok(())
}
