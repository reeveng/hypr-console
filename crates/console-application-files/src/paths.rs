//! A child name stays separate from the directory it names something in.
//! Native paths stay intact until a panel command needs text.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Component, Path};

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidName;

impl fmt::Display for InvalidName {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str("a name must stay inside its directory")
  }
}

impl std::error::Error for InvalidName {}

pub struct ChildName<'a>(&'a OsStr);

impl<'a> ChildName<'a> {
  pub fn new(name: &'a (impl AsRef<OsStr> + ?Sized)) -> Result<Self, InvalidName> {
    let native = name.as_ref();
    let entire = Path::new(native).file_name().filter(|part| *part == native);

    match (entire, native.as_encoded_bytes().contains(&0)) {
      (Some(part), false) => Ok(Self(part)),
      (Some(_), true) | (None, _) => Err(InvalidName),
    }
  }
}

impl AsRef<Path> for ChildName<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

pub(crate) struct RelativePath<'a>(&'a Path);

impl<'a> RelativePath<'a> {
  pub(crate) fn new(path: &'a Path) -> Result<Self, InvalidName> {
    let inside = path
      .components()
      .all(|part| matches!(part, Component::Normal(_)))
      && !path.as_os_str().as_encoded_bytes().contains(&0);

    match inside.then_some(()) {
      Some(()) => Ok(Self(path)),
      None => Err(InvalidName),
    }
  }
}

impl AsRef<Path> for RelativePath<'_> {
  fn as_ref(&self) -> &Path {
    self.0
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonTextPath<'a>(&'a Path);

impl fmt::Display for NonTextPath<'_> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "{} cannot be passed as a text path", self.0.display())
  }
}

impl std::error::Error for NonTextPath<'_> {}

pub(crate) fn text(path: &Path) -> Result<&str, NonTextPath<'_>> {
  path.to_str().ok_or(NonTextPath(path))
}

pub(crate) fn display_name(path: &Path) -> Result<String, Never> {
  let named = match path.file_name() {
    Some(name) => Path::new(name),
    None => path,
  };

  Ok(named.display().to_string())
}
