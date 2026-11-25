//! Applications in Flatpak's user and system installations and configured remotes.

#![forbid(unsafe_code)]

use std::io;
use std::process::{Command, Stdio};

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;

pub fn availability() -> Result<Installed, Never> {
  let Ok(name) = Program::Flatpak.name();

  installed(name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Installation {
  User,
  System,
}

impl Installation {
  #[must_use]
  fn option(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::User => "--user",
      Self::System => "--system",
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reference {
  pub id: String,
  pub branch: String,
  pub remote: String,
  pub installation: Installation,
}

pub struct Application {
  pub reference: Reference,
  pub title: String,
  pub version: String,
}

#[derive(Clone, Copy)]
pub enum Listing {
  Installed,
  Available,
}

pub fn applications(
  installation: Installation,
  listing: Listing,
) -> io::Result<Vec<Application>> {
  let columns = "--columns=application:f,branch:f,name:f,version:f,origin:f";
  let arguments: &[&str] = match listing {
    Listing::Installed => &["list", "--app", columns],
    Listing::Available => &["remote-ls", "--app", "--cached", columns],
  };
  let mut flatpak = Program::Flatpak.provide()?;
  let Ok(scope) = installation.option();
  flatpak.args(arguments).arg(scope);
  let output = invoke(flatpak)?;

  Ok(
    output
      .lines()
      .filter_map(|line| {
        let Ok(parsed) = application(line, installation);

        parsed
      })
      .collect(),
  )
}

#[must_use]
fn application(
  line: &str,
  installation: Installation,
) -> Result<Option<Application>, Never> {
  let mut columns = line.split('\t').map(str::trim);
  let (id, branch, title, version, remote) = match (
    columns.next(),
    columns.next(),
    columns.next(),
    columns.next(),
    columns.next(),
  ) {
    (Some(id), Some(branch), Some(title), Some(version), Some(remote)) => {
      (id, branch, title, version, remote)
    }
    _ => return Ok(None),
  };

  Ok(
    match (id.contains('.') && !branch.is_empty()).then_some(()) {
      Some(()) => Some(Application {
        reference: Reference {
          id: id.to_owned(),
          branch: branch.to_owned(),
          remote: remote.to_owned(),
          installation,
        },
        title: match title {
          "" => id.to_owned(),
          _ => title.to_owned(),
        },
        version: version.to_owned(),
      }),
      None => None,
    },
  )
}

fn invoke(mut command: Command) -> io::Result<String> {
  let output = command.env("LC_ALL", "C").stdin(Stdio::null()).output()?;

  match output.status.success().then_some(()) {
    Some(()) => String::from_utf8(output.stdout).map_err(io::Error::other),
    None => {
      let detail = match std::str::from_utf8(&output.stderr) {
        Ok(message) => message.trim().to_owned(),
        Err(fault) => format!("Invalid UTF-8 in stderr: {fault}"),
      };

      Err(io::Error::other(format!(
        "Flatpak exited with {}. {detail}",
        output.status
      )))
    }
  }
}

impl Reference {
  pub fn install(&self) -> io::Result<()> {
    let mut flatpak = Program::Flatpak.provide()?;
    let Ok(scope) = self.installation.option();
    let app = format!("app/{}//{}", self.id, self.branch);
    flatpak.args([
      "install",
      scope,
      "--noninteractive",
      "--",
      &self.remote,
      &app,
    ]);
    let _diagnostics = invoke(flatpak)?;

    Ok(())
  }

  pub fn remove(&self) -> io::Result<()> {
    let mut flatpak = Program::Flatpak.provide()?;
    let Ok(scope) = self.installation.option();
    let app = format!("app/{}//{}", self.id, self.branch);
    flatpak.args(["uninstall", scope, "--noninteractive", "--", &app]);
    let _diagnostics = invoke(flatpak)?;

    Ok(())
  }

  pub fn launch(&self) -> io::Result<Command> {
    let mut flatpak = Program::Flatpak.provide()?;
    let Ok(scope) = self.installation.option();
    let branch = format!("--branch={}", self.branch);
    flatpak.args(["run", scope, &branch, "--", &self.id]);

    Ok(flatpak)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn catalogue_rows_keep_the_remote_branch_and_installation() -> io::Result<()> {
    let row = "app.zen_browser.zen\tstable\tZen Browser\t1.24.2b\tflathub";
    let Ok(parsed) = application(row, Installation::User);
    let app = parsed.ok_or_else(|| io::Error::other("Expected a Flatpak application"))?;

    assert_eq!(app.title, "Zen Browser");
    assert_eq!(
      app.reference,
      Reference {
        id: String::from("app.zen_browser.zen"),
        branch: String::from("stable"),
        remote: String::from("flathub"),
        installation: Installation::User,
      }
    );

    Ok(())
  }
}
