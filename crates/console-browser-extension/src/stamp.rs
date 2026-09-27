//! What was packed last, and as what version.
//!
//! A browser installs an add-on from a file once and then looks at the version
//! in it. So the version has to go up when the files change, and has to stay
//! where it is when they do not: raised every apply, the browser would take an
//! add-on no one had touched every time the machine was told to catch up; left
//! alone, it would go on running the copy it installed in March.
//!
//! Neither of those is a thing to remember by hand. What was packed is written
//! down beside what it was packed as, and the two together answer both.

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub hash: String,
    pub version: String,
}

pub const FIRST: &str = "1.0.0";

pub fn read(said: &str) -> Result<Option<Stamp>, Never> {
    let (hash, version) = match said.trim().split_once(' ') {
        Some((hash, version)) => (hash, version),
        None => return Ok(None),
    };

    Ok(match hash.is_empty() || version.is_empty() {
        true => None,
        false => Some(Stamp { hash: hash.to_string(), version: version.to_string() }),
    })
}

pub fn format_stamp(stamp: &Stamp) -> Result<String, Never> {
    Ok(format!("{} {}\n", stamp.hash, stamp.version))
}

pub fn next(was: Option<&str>) -> Result<String, Never> {
    let was = match was {
        Some(was) => was,
        None => return Ok(FIRST.to_string()),
    };

    let (front, last) = match was.rsplit_once('.') {
        Some((front, last)) => (front, last),
        None => return Ok(FIRST.to_string()),
    };

    Ok(match last.parse::<u32>() {
        Ok(number) => match number.checked_add(1) {
            Some(next) => format!("{front}.{next}"),
            None => FIRST.to_string(),
        },
        Err(_not_a_number) => FIRST.to_string(),
    })
}

pub fn version_of(bytes: &[u8]) -> Result<Option<String>, Never> {
    let text = String::from_utf8_lossy(bytes);

    let rest = match text.split_once("\"version\": \"") {
        Some((_, rest)) => rest,
        None => return Ok(None),
    };

    let said = match rest.split_once('"') {
        Some((said, _)) => said,
        None => return Ok(None),
    };

    Ok(match said.is_empty() || said.contains(char::REPLACEMENT_CHARACTER) {
        true => None,
        false => Some(said.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn a_note_is_what_was_packed_and_what_it_was_called() -> Result<(), Box<dyn Error>> {
        let Ok(stamp) = read("abc123 1.0.4\n");
        let stamp = stamp.ok_or("a note")?;

        assert_eq!(stamp.hash, "abc123");
        assert_eq!(stamp.version, "1.0.4");
        assert_eq!(format_stamp(&stamp), Ok("abc123 1.0.4\n".to_string()));

        Ok(())
    }

    #[test]
    fn a_note_that_says_nothing_is_no_note_at_all() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read("abc123"), Ok(None));
    }

    #[test]
    fn the_next_version_is_the_last_number_and_one() {
        assert_eq!(next(Some("1.0.9")), Ok("1.0.10".to_string()));
        assert_eq!(next(Some("2.3.99")), Ok("2.3.100".to_string()));
    }

    #[test]
    fn nothing_to_go_up_from_starts_at_the_first_one() {
        assert_eq!(next(None), Ok(FIRST.to_string()));
        assert_eq!(next(Some("what")), Ok(FIRST.to_string()));
    }

    #[test]
    fn the_version_can_be_read_back_out_of_what_was_packed() {
        let palette = crate::source::Palette(":root { --pink: #ffb5e2; }");
        let Ok(files) = crate::source::every("1.2.3", palette);
        let Ok(held) = crate::pack::zip(&files);

        assert_eq!(version_of(&held), Ok(Some("1.2.3".to_string())));
    }

    #[test]
    fn an_archive_that_is_not_ours_says_nothing_about_a_version() {
        assert_eq!(version_of(b"PK\x03\x04 and nothing else"), Ok(None));
    }
}
