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
//! the bytes as a path, so a name that is not UTF-8 survives the trip.

use console_core_never::Never;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

pub const SCHEME: &str = "file://";

pub const PLAIN: &str = "-_.!~*'()/&=:@+$,";

pub fn escaped(path: &Path) -> Result<String, Never> {
    let mut written = String::new();

    for byte in path.as_os_str().as_bytes() {
        let letter = char::from(*byte);

        match letter.is_ascii_alphanumeric() || PLAIN.contains(letter) {
            true => written.push(letter),
            false => written.push_str(&format!("%{byte:02X}")),
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

    let Ok(bytes) = unescaped(spelled.as_bytes());

    Ok(Some(PathBuf::from(OsString::from_vec(bytes))))
}

fn unescaped(spelled: &[u8]) -> Result<Vec<u8>, Never> {
    let Ok(first) = unescaped_byte(spelled);

    Ok(std::iter::successors(first, |(_, rest)| {
        let Ok(next) = unescaped_byte(rest);

        next
    })
    .map(|(byte, _)| byte)
    .collect())
}

fn unescaped_byte(rest: &[u8]) -> Result<Option<(u8, &[u8])>, Never> {
    let (first, after) = match rest.split_first() {
        Some(split) => split,
        None => return Ok(None),
    };

    let Ok(escape) = escape(after);

    Ok(Some(match (*first, escape) {
        (b'%', Some(byte)) => (byte, match after.get(2..) {
            Some(beyond) => beyond,
            None => &[],
        }),
        (plain, Some(_)) | (plain, None) => (plain, after),
    }))
}

fn escape(after: &[u8]) -> Result<Option<u8>, Never> {
    let digits = match after.get(..2) {
        Some(digits) => digits,
        None => return Ok(None),
    };

    let written = match std::str::from_utf8(digits) {
        Ok(written) => written,
        Err(_not_two_digits) => return Ok(None),
    };

    Ok(match u8::from_str_radix(written, 16) {
        Ok(byte) => Some(byte),
        Err(_not_hexadecimal) => None,
    })
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

            assert_eq!(path(&written), Ok(Some(at.to_path_buf())), "{spelled} came back as something else");
        }
    }

    #[test]
    fn a_letter_somebody_else_encoded_is_the_letter_and_not_its_bytes() {
        assert_eq!(path("file:///music/Beyonc%C3%A9.opus"), Ok(Some(PathBuf::from("/music/Beyonc\u{e9}.opus"))));
    }

    #[test]
    fn a_percent_that_escapes_nothing_is_a_percent() {
        assert_eq!(path("file:///a/100%.flac"), Ok(Some(PathBuf::from("/a/100%.flac"))));
        assert_eq!(path("file:///a/%zz"), Ok(Some(PathBuf::from("/a/%zz"))));
    }

    #[test]
    fn an_address_that_is_not_a_file_is_no_path() {
        assert_eq!(path("https://example.com/cover.jpg"), Ok(None));
    }
}
