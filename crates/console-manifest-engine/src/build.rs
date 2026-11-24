//! The programs the device compiles for itself.
//!
//! Nothing compiled is kept in this repository. The source is here, `rust` is
//! in the manifest like any other package, and `console apply` builds before it
//! installs. So the manifest stays the whole truth about the machine and the
//! public copy of this repository is source and nothing else.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use console_core_internal_programs::EXECUTABLE_DIRECTORY;
use console_core_never::Never;
use console_core_words::Words;

use crate::{manifest::BuildName, unapplied::Unapplied, up_to_date::UpToDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Programs {
  Compile,
  Prebuilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum State {
  #[words(name = "ok")]
  Ok,
  #[words(name = "differs")]
  Differs,
  #[words(name = "missing")]
  NotFound,
  #[words(name = "not built")]
  Unbuilt,
}

impl State {
  pub fn up_to_date(self) -> Result<UpToDate, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self == State::Ok, ()) {
        ::core::option::Option::Some(()) => UpToDate::Yes,
        ::core::option::Option::None => UpToDate::No,
      },
    )
  }
}

pub fn built_path(root: &Path, name: BuildName<'_>) -> Result<PathBuf, Never> {
  Ok(root.join("target/release").join(name))
}

pub fn live(name: BuildName<'_>) -> Result<String, Never> {
  let Ok(text) = name.as_str();

  Ok(format!("{EXECUTABLE_DIRECTORY}/{text}"))
}

pub fn verify_prebuilt<'a>(
  root: &Path,
  names: impl IntoIterator<Item = BuildName<'a>>,
) -> Result<(), Unapplied> {
  for name in names {
    let Ok(path) = built_path(root, name);
    let checked =
      std::fs::symlink_metadata(&path).map_err(|fault| Unapplied::Read(path.clone(), fault))?;

    match ::core::primitive::bool::then_some(
      checked.is_file() && checked.permissions().mode() & 0o111 != 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(Unapplied::NotAnExecutable(path)),
    }
  }

  Ok(())
}

pub fn state(root: &Path, name: BuildName<'_>) -> Result<State, Never> {
  let Ok(made) = built_path(root, name);
  let Ok(live) = live(name);

  Ok(match (std::fs::read(made), std::fs::read(live)) {
    (Err(_not_built), _) => State::Unbuilt,
    (Ok(_), Err(_not_installed)) => State::NotFound,
    (Ok(built), Ok(there)) => match ::core::primitive::bool::then_some(built == there, ()) {
      ::core::option::Option::Some(()) => State::Ok,
      ::core::option::Option::None => State::Differs,
    },
  })
}

pub fn how<'a>(names: impl IntoIterator<Item = BuildName<'a>>) -> Result<Vec<String>, Never> {
  Ok(
    ["build", "--release", "--locked"]
      .iter()
      .map(|word| word.to_string())
      .chain(names.into_iter().flat_map(|name| {
        let Ok(text) = name.as_str();

        ["--bin".to_string(), text.to_string()]
      }))
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_program_is_installed_where_everything_not_from_a_package_lives()
  -> Result<(), crate::unapplied::Unapplied> {
    let program = BuildName::parse("console-panel")?;

    assert_eq!(
      live(program),
      Ok("/usr/local/bin/console-panel".to_string())
    );

    Ok(())
  }

  #[test]
  fn cargo_is_asked_once_for_every_program() -> Result<(), crate::unapplied::Unapplied> {
    let panel = BuildName::parse("console-panel")?;
    let buttons = BuildName::parse("console-buttons")?;
    let Ok(how) = how([panel, buttons]);

    assert_eq!(
      how,
      [
        "build",
        "--release",
        "--locked",
        "--bin",
        "console-panel",
        "--bin",
        "console-buttons",
      ]
    );

    Ok(())
  }

  #[test]
  fn nothing_built_is_not_the_same_as_nothing_installed() {
    let Ok(settled) = State::Unbuilt.up_to_date();
    let Ok(name) = State::Unbuilt.name();

    assert_ne!(State::Unbuilt, State::NotFound);
    assert_eq!(settled, UpToDate::No);
    assert_eq!(name, "not built");
  }

  #[test]
  fn a_program_nothing_has_compiled_is_unbuilt_whatever_the_machine_holds()
  -> Result<(), crate::unapplied::Unapplied> {
    let nowhere = Path::new("/nonexistent-tree");
    let program = BuildName::parse("console-panel")?;

    assert_eq!(state(nowhere, program), Ok(State::Unbuilt));

    Ok(())
  }
}
