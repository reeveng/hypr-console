//! An empty directory this process can work in.
//!
//! Crate after crate wanted one, for a test or for a check, and each wrote the
//! same four lines: the machine's temporary directory, a name with the process
//! id in it, whatever an earlier run with that id left there thrown away, and
//! the directory made again. The copies differed only in whether a directory
//! that would not empty was a fault or was ignored, and ignoring it hands the
//! test a directory with somebody else's files already in it.
//!
//! A caller supplies one nonempty filename, never a path. Empty names, dot
//! components, separators and NUL bytes are refused before anything is removed
//! or made. Only the checked, process-qualified name is joined to the temporary
//! directory, so a caller's label cannot select a directory outside it.
//!
//! Nothing here removes the directory afterwards. A test that fails leaves its
//! directory behind to be read, which is the case worth keeping, and the next
//! run with the same id starts from nothing anyway.

use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use console_core_never::Never;

#[derive(Debug)]
pub enum Unmade {
  InvalidName(String),
  Emptying(PathBuf, std::io::Error),
  Making(PathBuf, std::io::Error),
}

impl fmt::Display for Unmade {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unmade::InvalidName(named) => write!(
        to,
        "temporary directory name {named:?} must be a nonempty filename other than . or .., without a slash or NUL byte"
      ),
      Unmade::Emptying(at, fault) => write!(to, "could not empty {}: {fault}", at.display()),
      Unmade::Making(at, fault) => write!(to, "could not make {}: {fault}", at.display()),
    }
  }
}

impl std::error::Error for Unmade {}

struct DirectoryName(String);

impl DirectoryName {
  fn new(named: &str, process: u32) -> Result<Self, Unmade> {
    match (named, named.split_once(['/', '\0'])) {
      ("" | "." | "..", _) | (_, Some(_)) => Err(Unmade::InvalidName(named.to_string())),
      (_name, None) => Ok(Self(format!("console-{named}-{process}"))),
    }
  }

  fn with_suffix(&self, suffix: u128) -> Result<Self, Never> {
    Ok(Self(format!("{}-{suffix:032x}", self.0)))
  }
}

impl AsRef<Path> for DirectoryName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

#[doc = "The remote migration uses this persistent template with `mktemp -d` to make a private attic."]
pub const fn migration_attic_template() -> Result<&'static str, Never> {
  Ok("/var/tmp/console-migration-XXXXXXXXXX")
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "this crate provides the configured temporary directory for callers with their own fallback"
  )
)]
pub fn configured_root() -> Result<Option<PathBuf>, Never> {
  Ok(std::env::var_os("TMPDIR").map(PathBuf::from))
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "the machine's temporary directory and this process's id are what make the directory this process's own, and reading them is the whole of this crate"
  )
)]
pub fn fresh(named: &str) -> Result<PathBuf, Unmade> {
  let name = DirectoryName::new(named, std::process::id())?;
  let at = std::env::temp_dir().join(name);

  match std::fs::remove_dir_all(&at) {
    Ok(()) => {}
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => {}
      None => return Err(Unmade::Emptying(at, fault)),
    },
  }

  match std::fs::create_dir_all(&at) {
    Ok(()) => Ok(at),
    Err(fault) => Err(Unmade::Making(at, fault)),
  }
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "this filesystem boundary chooses the temporary root and owns the decision to obtain a fresh directory"
  )
)]
pub fn private(named: &str) -> Result<PathBuf, Unmade> {
  private_under(&std::env::temp_dir(), named)
}

pub fn private_under(root: &Path, named: &str) -> Result<PathBuf, Unmade> {
  let name = DirectoryName::new(named, std::process::id())?;
  let mut builder = std::fs::DirBuilder::new();

  #[cfg(unix)]
  {
    use std::os::unix::fs::DirBuilderExt;
    builder.mode(0o700);
  }

  for _ in 0..8 {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|fault| {
      Unmade::Making(root.to_path_buf(), std::io::Error::other(fault.to_string()))
    })?;
    let Ok(qualified) = name.with_suffix(u128::from_be_bytes(random));
    let directory = root.join(qualified);

    match builder.create(&directory) {
      Ok(()) => return Ok(directory),
      Err(fault) => match (fault.kind() == ErrorKind::AlreadyExists).then_some(()) {
        Some(()) => {}
        None => return Err(Unmade::Making(directory, fault)),
      },
    }
  }

  Err(Unmade::Making(
    root.to_path_buf(),
    std::io::Error::from(ErrorKind::AlreadyExists),
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  #[test]
  fn a_directory_is_made_and_is_empty() -> Result<(), Failure> {
    let at = fresh("temporary-made")?;
    let inside = std::fs::read_dir(&at)?;

    assert_eq!(inside.count(), 0);

    Ok(())
  }

  #[test]
  fn what_an_earlier_run_left_is_gone() -> Result<(), Failure> {
    let at = fresh("temporary-again")?;

    std::fs::create_dir(at.join("left"))?;

    let again = fresh("temporary-again")?;
    let inside = std::fs::read_dir(&again)?;

    assert_eq!(again, at);
    assert_eq!(inside.count(), 0);

    Ok(())
  }

  #[test]
  fn two_names_are_two_directories() -> Result<(), Failure> {
    let one = fresh("temporary-one")?;
    let other = fresh("temporary-other")?;

    assert_ne!(one, other);

    Ok(())
  }

  #[test]
  fn a_name_that_is_not_one_filename_is_refused() {
    for named in [
      "",
      ".",
      "..",
      "/absolute",
      "child/nested",
      "../escape",
      "x/../../escape",
      "x\0y",
    ] {
      assert!(
        matches!(fresh(named), Err(Unmade::InvalidName(_invalid_name))),
        "{named:?}"
      );
    }
  }

  #[test]
  fn a_name_cannot_empty_another_directory_by_climbing_out_of_its_prefix() -> Result<(), Failure>
  {
    let prefix = fresh("temporary-prefix")?;
    let protected = fresh("temporary-protected")?;
    let kept = protected.join("kept");

    std::fs::create_dir(&kept)?;

    let named = format!(
      "temporary-prefix-{}/../console-temporary-protected",
      std::process::id()
    );
    let refused = fresh(&named);

    let contents = std::fs::read_dir(&kept)?;
    let prefix_contents = std::fs::read_dir(&prefix)?;

    assert!(matches!(&refused, Err(_invalid_name)), "{refused:?}");
    assert_eq!(contents.count(), 0);
    assert_eq!(prefix_contents.count(), 0);

    Ok(())
  }

  #[test]
  fn a_filename_can_contain_unicode_and_spaces() -> Result<(), Failure> {
    let at = fresh("temporary-été 空")?;
    let inside = std::fs::read_dir(&at)?;

    assert_eq!(inside.count(), 0);

    Ok(())
  }

  #[test]
  fn private_directories_keep_the_label_and_use_distinct_names() -> Result<(), Failure> {
    let first = private("temporary-été 空")?;
    let second = private("temporary-été 空")?;

    assert_ne!(first, second);
    assert_eq!(first.parent(), second.parent());
    assert!(
      first
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("console-temporary-été 空-"))
    );

    std::fs::remove_dir(first)?;
    std::fs::remove_dir(second)?;

    Ok(())
  }
}
