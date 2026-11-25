//! Project-scoped commands for the mise tool manager.

#![forbid(unsafe_code)]

pub mod applications;

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use console_core_external_programs::Program;

pub enum Operation<'a> {
  Status,
  Install,
  PreviewUpdate,
  UpdateLock,
  Run(&'a [OsString]),
}

pub fn command(project: &Path, operation: Operation<'_>) -> io::Result<Command> {
  let parent = project.parent().ok_or(io::ErrorKind::InvalidInput)?;
  let _config = fs::metadata(project.join("mise.toml"))?;
  let mut mise = Program::Mise.provide()?;

  mise
    .current_dir(project)
    .env("MISE_CEILING_PATHS", parent)
    .env("MISE_GLOBAL_CONFIG_FILE", "/dev/null/console-mise.toml")
    .env("MISE_SYSTEM_CONFIG_FILE", "/dev/null/console-mise.toml")
    .env("MISE_SYSTEM_DEPS", "warn")
    .env("MISE_LOCKED_REQUIRE_CHECKSUM", "1")
    .env("MISE_AUTO_INSTALL", "0")
    .env("MISE_SELF_UPDATE_AUTO", "0")
    .env("MISE_YES", "0");

  match operation {
    Operation::Status => mise.args(["ls", "--project"]),
    Operation::Install => mise.args(["install", "--locked"]),
    Operation::PreviewUpdate => mise.args(["lock", "--bump", "--dry-run"]),
    Operation::UpdateLock => mise.args(["lock", "--bump"]),
    Operation::Run(arguments) => mise.args(["exec", "--locked", "--"]).args(arguments),
  };

  Ok(mise)
}
