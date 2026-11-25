//! Gather identity values from the local build host and source checkout.

use std::path::Path;
use std::process::ExitStatus;

use console_core_external_programs::Program;
use console_core_never::Never;

use super::{Watched, push};

type Reading = (Probe, Result<Option<String>, Unavailable>);

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  allow(
    explicit057_no_unbounded_wait,
    reason = "each OS identity command runs with timeout, including the worker's two probes"
  )
)]
pub fn forbidden_names(repository: &Path) -> Result<(Vec<Watched>, Vec<String>), Never> {
  let readings = std::thread::scope(|scope| {
    let os_probes = [Probe::Account, Probe::Hostname];
    let os = std::thread::Builder::new().spawn_scoped(scope, move || read(repository, os_probes));
    let Ok(git) = read(repository, [Probe::GitName, Probe::GitEmail]);
    let Ok(local) = match os {
      Ok(worker) => match worker.join() {
        Ok(found) => found,
        Err(_panicked) => Ok(os_probes.map(|probe| (probe, Err(Unavailable::WorkerStopped)))),
      },
      Err(_could_not_start) => read(repository, os_probes),
    };

    local.into_iter().chain(git)
  });
  let mut names = Vec::new();
  let mut missing = Vec::new();

  for (probe, reading) in readings {
    let Ok(what) = probe.label();

    match reading {
      Ok(Some(name)) => {
        let Ok(()) = push(&mut names, Watched { name, what });
      }
      Ok(None) => {}
      Err(fault) => missing.push(format!("Could not check {what}. {fault}")),
    }
  }

  Ok((names, missing))
}

#[derive(Clone, Copy)]
enum Probe {
  Account,
  Hostname,
  GitName,
  GitEmail,
}

fn read(repository: &Path, probes: [Probe; 2]) -> Result<[Reading; 2], Never> {
  Ok(probes.map(|probe| (probe, probe.observe(repository))))
}

impl Probe {
  fn label(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Account => "the local account name",
      Self::Hostname => "the local hostname",
      Self::GitName => "the configured Git name",
      Self::GitEmail => "the configured Git email",
    })
  }

  fn observe(self, repository: &Path) -> Result<Option<String>, Unavailable> {
    let (program, args): (Program, &[&str]) = match self {
      Self::Account => (Program::Id, &["-un"]),
      Self::Hostname => (Program::Hostname, &[]),
      Self::GitName => (Program::Git, &["config", "--get", "user.name"]),
      Self::GitEmail => (Program::Git, &["config", "--get", "user.email"]),
    };
    let Ok(name) = program.name();
    let Ok(mut timeout) = Program::Timeout.command();
    let output = timeout
      .current_dir(repository)
      .args(["--kill-after=2s", "3s", name])
      .args(args)
      .output()
      .map_err(Unavailable::Launch);
    let reply = output?;

    match (self, reply.status.code()) {
      (_, Some(0)) => {
        let text = std::str::from_utf8(&reply.stdout).map_err(Unavailable::Output);
        let value = text?;
        self.value(value.trim())
      }
      (Self::GitName | Self::GitEmail, Some(1)) => Ok(None),
      (_, _) => Err(Unavailable::Exit(reply.status)),
    }
  }

  fn value(self, text: &str) -> Result<Option<String>, Unavailable> {
    match (self, text) {
      (Self::Account | Self::Hostname, "") => Err(Unavailable::Empty),
      (Self::GitName | Self::GitEmail, "") => Ok(None),
      (_, identity) => Ok(Some(identity.to_owned())),
    }
  }
}

enum Unavailable {
  Launch(std::io::Error),
  Output(std::str::Utf8Error),
  Exit(ExitStatus),
  Empty,
  WorkerStopped,
}

impl std::fmt::Display for Unavailable {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Launch(fault) => write!(out, "Could not start the command. {fault}"),
      Self::Output(fault) => write!(out, "The command returned invalid UTF-8. {fault}"),
      Self::Exit(status) => write!(out, "The command reported {status}."),
      Self::Empty => out.write_str("The command returned an empty identity."),
      Self::WorkerStopped => out.write_str("The local identity worker stopped unexpectedly."),
    }
  }
}
