//! GIO commands for launching desktop entries and moving files to the trash.

use std::io;
use std::path::Path;
use std::process::Command;

use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use console_program_contract::{Command as EffectCommand, Executable};

const TRASH: [&str; 2] = ["trash", "--"];

pub fn availability() -> Result<Installed, Never> {
  let Ok(name) = Program::Gio.name();

  installed(name)
}

pub fn launch(entry: &Path) -> Result<Command, Never> {
  let Ok(mut command) = Program::Gio.command();

  command.arg("launch").arg(entry);

  Ok(command)
}

pub fn trash_command(paths: &[String]) -> Result<EffectCommand, Never> {
  let arguments = TRASH
    .into_iter()
    .map(str::to_owned)
    .chain(paths.iter().cloned())
    .collect();

  Ok(EffectCommand {
    program: Executable::External(Program::Gio),
    arguments,
  })
}

pub fn trash(path: &Path) -> io::Result<()> {
  let mut command = Program::Gio.provide()?;
  let output = command.args(TRASH).arg(path).output()?;

  match output.status.success().then_some(()) {
    Some(()) => Ok(()),
    None => {
      #[cfg_attr(
        dylint_lib = "explicit089_no_lossy_text",
        expect(
          explicit089_no_lossy_text,
          reason = "GIO's stderr is displayed as diagnostic text and drives no decision"
        )
      )]
      let stderr = String::from_utf8_lossy(&output.stderr);

      Err(io::Error::other(format!(
        "moving {} to the trash failed ({}): {}",
        path.display(),
        output.status,
        stderr.trim(),
      )))
    }
  }
}
