//! A path spelled as a `file://` address, and read back out of one.
//!
//! Four places wrote this and no two agreed. The thumbnails spelled a path the
//! way the thumbnail standard does, byte by byte, because the name of the
//! thumbnail is a digest of that spelling and a different one is a different
//! file. The music player put `file://` in front of the path and nothing else,
//! its bus took the prefix off and nothing else, and the music panel
//! percent-decoded what the player had not percent-encoded -- one byte at a
//! time into a `char`, so an `é` somebody else encoded came back as two Latin-1
//! letters, and a file with `%41` in its name was read as one with `A`.
//!
//! One spelling now, the standard's, both ways. What is written escapes every
//! byte outside the letters, digits and the marks RFC 2396 leaves alone, and
//! what is read turns every `%XX` back into the byte it was and only then reads
//! the bytes as a path, so a name that is not UTF-8 survives the trip. What an
//! escape is belongs to `console-core-percent-decoding`, which a book's links
//! and a `data:` address ask as well.

use console_core_never::Never;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

pub const SCHEME: &str = "file://";

pub const PLAIN: &str = "-_.!~*'()/&=:@+$,";

enum PathByteEncoding {
  Verbatim,
  PercentEscaped,
}

impl From<bool> for PathByteEncoding {
  fn from(can_write_verbatim: bool) -> Self {
    match bool::then_some(can_write_verbatim, ()) {
      Some(()) => Self::Verbatim,
      None => Self::PercentEscaped,
    }
  }
}

pub fn escaped(path: &Path) -> Result<String, Never> {
  let mut written = String::new();

  for byte in path.as_os_str().as_bytes() {
    let letter = char::from(*byte);

    match PathByteEncoding::from(letter.is_ascii_alphanumeric() || PLAIN.contains(letter)) {
      PathByteEncoding::Verbatim => written.push(letter),
      PathByteEncoding::PercentEscaped => written.push_str(&format!("%{byte:02X}")),
    }
  }

  Ok(written)
}

pub fn url(path: &Path) -> Result<String, Never> {
  let Ok(escaped) = escaped(path);

  Ok(format!("{SCHEME}{escaped}"))
}

pub fn path(url: &str) -> Result<Option<PathBuf>, Never> {
  let spelled = match url.strip_prefix(SCHEME) {
    Some(spelled) => spelled,
    None => return Ok(None),
  };

  let Ok(bytes) = console_core_percent_decoding::decoded(spelled.as_bytes());

  Ok(Some(PathBuf::from(OsString::from_vec(bytes))))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_path_is_read_back_as_the_path_it_was() {
    for spelled in [
      "/music/The Last Shadow Puppets.opus",
      "/music/Beyonc\u{e9} [x9].opus",
      "/music/100%41 sure.flac",
      "/home/ada/Pictures/#q%20[1].jpg",
    ] {
      let at = Path::new(spelled);
      let Ok(written) = url(at);

      assert_eq!(
        path(&written),
        Ok(Some(at.to_path_buf())),
        "{spelled} came back as something else"
      );
    }
  }

  #[test]
  fn a_letter_somebody_else_encoded_is_the_letter_and_not_its_bytes() {
    assert_eq!(
      path("file:///music/Beyonc%C3%A9.opus"),
      Ok(Some(PathBuf::from("/music/Beyonc\u{e9}.opus")))
    );
  }

  #[test]
  fn a_percent_that_escapes_nothing_is_a_percent() {
    assert_eq!(
      path("file:///a/100%.flac"),
      Ok(Some(PathBuf::from("/a/100%.flac")))
    );
    assert_eq!(path("file:///a/%zz"), Ok(Some(PathBuf::from("/a/%zz"))));
  }

  #[test]
  fn an_address_that_is_not_a_file_is_no_path() {
    assert_eq!(path("https://example.com/cover.jpg"), Ok(None));
  }
}
