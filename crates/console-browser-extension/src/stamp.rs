//! What was packed last, and as what version.
//!
//! A browser installs an add-on from a file once and then looks at the version
//! in it. So the version has to go up when the files change, and has to stay
//! where it is when they do not: raised every apply, the browser would take an
//! add-on nobody had touched every time the machine was told to catch up; left
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
    let Some((hash, version)) = said.trim().split_once(' ') else { return Ok(None) };

    Ok(match hash.is_empty() || version.is_empty() {
        true => None,
        false => Some(Stamp { hash: hash.to_string(), version: version.to_string() }),
    })
}

pub fn written(stamp: &Stamp) -> Result<String, Never> {
    Ok(format!("{} {}\n", stamp.hash, stamp.version))
}

pub fn next(was: Option<&str>) -> Result<String, Never> {
    let Some(was) = was else { return Ok(FIRST.to_string()) };

    let Some((front, last)) = was.rsplit_once('.') else { return Ok(FIRST.to_string()) };

    Ok(match last.parse::<u32>() {
        Ok(number) => match number.checked_add(1) {
            Some(next) => format!("{front}.{next}"),
            None => FIRST.to_string(),
        },
        Err(_) => FIRST.to_string(),
    })
}

pub fn packed(bytes: &[u8]) -> Result<Option<String>, Never> {
    let mark = b"\"version\": \"";

    let Some(found) = bytes.windows(mark.len()).position(|window| window == mark) else { return Ok(None) };

    let at = found.saturating_add(mark.len());

    let Some(rest) = bytes.get(at..) else { return Ok(None) };

    let Some(end) = rest.iter().position(|byte| *byte == b'"') else { return Ok(None) };

    let Some(inside) = rest.get(..end) else { return Ok(None) };

    let Ok(said) = String::from_utf8(inside.to_vec()) else {
        return Ok(None);
    };

    Ok(match said.is_empty() {
        true => None,
        false => Some(said),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(said: &str) -> Option<Stamp> {
        let Ok(stamp) = super::read(said);

        stamp
    }

    fn written(stamp: &Stamp) -> String {
        let Ok(said) = super::written(stamp);

        said
    }

    fn next(was: Option<&str>) -> String {
        let Ok(said) = super::next(was);

        said
    }

    fn packed(bytes: &[u8]) -> Option<String> {
        let Ok(said) = super::packed(bytes);

        said
    }

    #[test]
    fn a_note_is_what_was_packed_and_what_it_was_called() {
        let stamp = read("abc123 1.0.4\n").expect("a note");
        assert_eq!(stamp.hash, "abc123");
        assert_eq!(stamp.version, "1.0.4");
        assert_eq!(written(&stamp), "abc123 1.0.4\n");
    }

    #[test]
    fn a_note_that_says_nothing_is_no_note_at_all() {
        assert_eq!(read(""), None);
        assert_eq!(read("abc123"), None);
    }

    #[test]
    fn the_next_version_is_the_last_number_and_one() {
        assert_eq!(next(Some("1.0.9")), "1.0.10");
        assert_eq!(next(Some("2.3.99")), "2.3.100");
    }

    #[test]
    fn nothing_to_go_up_from_starts_at_the_first_one() {
        assert_eq!(next(None), FIRST);
        assert_eq!(next(Some("what")), FIRST);
    }

    #[test]
    fn the_version_can_be_read_back_out_of_what_was_packed() {
        let Ok(files) = crate::source::every("1.2.3", ":root { --pink: #ffb5e2; }");
        let Ok(held) = crate::pack::zip(&files);
        assert_eq!(packed(&held).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn an_archive_that_is_not_ours_says_nothing_about_a_version() {
        assert_eq!(packed(b"PK\x03\x04 and nothing else"), None);
    }
}
