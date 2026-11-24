use std::ffi::OsStr;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use console_core_never::Never;

pub(crate) struct Name<'a>(&'a OsStr);

impl<'a> Name<'a> {
  pub(crate) fn new(word: &'a OsStr) -> Result<Option<Self>, Never> {
    let bytes = word.as_encoded_bytes();

    Ok(match bytes {
      [] | b"." | b".." => None,
      _ => match bytes
        .iter()
        .any(|byte| matches!(byte, b'/' | 0))
        .then_some(())
      {
        Some(()) => None,
        None => Some(Self(word)),
      },
    })
  }
}

impl AsRef<Path> for Name<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
  Directory,
  File,
  Other,
  Unavailable,
}

pub(crate) fn status(at: &Path) -> Result<Status, Never> {
  let metadata = match at.metadata() {
    Ok(metadata) => metadata,
    Err(_unavailable) => return Ok(Status::Unavailable),
  };

  Ok(match (metadata.is_dir(), metadata.is_file()) {
    (true, _) => Status::Directory,
    (false, true) => Status::File,
    (false, false) => Status::Other,
  })
}

#[cfg(test)]
pub(crate) fn fixture(name: &str) -> Result<PathBuf, Never> {
  let crate_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
  let parent = match crate_directory.parent() {
    Some(parent) => parent,
    None => return Ok(crate_directory.to_path_buf()),
  };
  let clips = parent.join("console-films/tests/films");
  let Ok(name_2) = Name::new(OsStr::new(name));

  Ok(match name_2 {
    Some(name_3) => clips.join(name_3),
    None => clips,
  })
}
