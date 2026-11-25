//! Provision the package's system Nix daemon during Console installation.

use std::fs::{self, Metadata};
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::machine::{self, Ran};
use crate::unapplied::Unapplied;

const CONF: &str = "/etc/nix/nix.conf";
const FEATURES: &str = "extra-experimental-features = nix-command flakes";
const SOCKET: &str = "nix-daemon.socket";
const DAEMON_SETUP: &str = "nix-daemon.conf";

const STATE_DIRECTORIES: &[&str] = &[
  "/nix",
  "/nix/var",
  "/nix/var/nix",
  "/nix/var/nix/db",
  "/nix/var/nix/daemon-socket",
  "/nix/var/nix/gcroots",
  "/nix/var/nix/profiles",
];

const DATABASE_FILES: &[&str] = &[
  "/nix/var/nix/db/db.sqlite",
  "/nix/var/nix/db/db.sqlite-journal",
  "/nix/var/nix/db/db.sqlite-shm",
  "/nix/var/nix/db/db.sqlite-wal",
  "/nix/var/nix/db/big-lock",
  "/nix/var/nix/db/schema",
];

#[derive(Clone, Copy)]
enum StatePath {
  Directory,
  DatabaseFile,
  Store,
}

pub fn run() -> Result<(), Unapplied> {
  validate_store()?;
  configure_features(Path::new(CONF))?;
  checked(Program::SystemdSysusers, &[DAEMON_SETUP])?;
  checked(Program::SystemdTmpfiles, &["--create", DAEMON_SETUP])?;
  checked(Program::Systemctl, &["--system", "enable", "--now", SOCKET])?;
  checked(Program::Nix, &["--store", "daemon", "store", "info"])?;

  println!("  {SOCKET} is ready");

  Ok(())
}

fn validate_store() -> Result<(), Unapplied> {
  for directory in STATE_DIRECTORIES {
    check_path(Path::new(directory), StatePath::Directory)?;
  }

  check_path(Path::new("/nix/store"), StatePath::Store)?;

  for file in DATABASE_FILES {
    check_path(Path::new(file), StatePath::DatabaseFile)?;
  }

  Ok(())
}

fn check_path(at: &Path, kind: StatePath) -> Result<(), Unapplied> {
  let metadata = match fs::symlink_metadata(at) {
    Ok(metadata) => metadata,
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(Unapplied::Read(at.to_path_buf(), fault)),
    },
  };
  let Ok(protected) = protection(&metadata, kind);

  match protected {
    Protection::Root => Ok(()),
    Protection::Other => Err(Unapplied::NixStoreOwnership(at.to_path_buf())),
  }
}

enum Protection {
  Root,
  Other,
}

fn protection(metadata: &Metadata, kind: StatePath) -> Result<Protection, Never> {
  let mode = metadata.mode();
  let valid = match kind {
    StatePath::Directory => metadata.is_dir() && mode & 0o022 == 0,
    StatePath::DatabaseFile => metadata.is_file() && mode & 0o022 == 0,
    StatePath::Store => {
      metadata.is_dir() && mode & 0o002 == 0 && (mode & 0o020 == 0 || mode & 0o1000 != 0)
    }
  };

  Ok(match (metadata.uid() == 0 && valid).then_some(()) {
    Some(()) => Protection::Root,
    None => Protection::Other,
  })
}

fn configure_features(at: &Path) -> Result<(), Unapplied> {
  let existing = console_core_atomic_writes::read_text(at);
  let text = existing.map_err(|fault| Unapplied::Read(at.to_path_buf(), fault))?;
  let Ok(replacement) = with_features(&text);

  match replacement {
    Some(updated) => console_core_atomic_writes::whole(at, updated.as_bytes())
      .map_err(|fault| Unapplied::Saving(at.display().to_string(), fault)),
    None => Ok(()),
  }
}

fn with_features(text: &str) -> Result<Option<String>, Never> {
  let configured = text.trim_end().lines().next_back().map(str::trim) == Some(FEATURES);

  Ok(match configured.then_some(()) {
    Some(()) => None,
    None => {
      let separator = match (text.is_empty() || text.ends_with('\n')).then_some(()) {
        Some(()) => "",
        None => "\n",
      };

      Some(format!("{text}{separator}{FEATURES}\n"))
    }
  })
}

fn checked(program: Program, arguments: &[&str]) -> Result<(), Unapplied> {
  let Ok(mut command) = program.command();
  let Ok(output) = machine::capture(command.args(arguments));

  match output.ran {
    Ran::Fine => Ok(()),
    Ran::Badly => {
      let Ok(name) = program.name();
      let invoked = format!("{name} {}", arguments.join(" "));

      Err(Unapplied::NixSetup {
        command: invoked,
        diagnostic: output.said,
      })
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn adding_features_preserves_the_local_configuration_and_can_be_retried() {
    let before = "# configured here\nexperimental-features = fetch-tree\nmax-jobs = 2";
    let after = format!("{before}\n{FEATURES}\n");

    assert_eq!(with_features(before), Ok(Some(after.clone())));
    assert_eq!(with_features(&after), Ok(None));
  }
}
