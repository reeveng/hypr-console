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

/// What is written beside the packed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    /// What the files came to when they were last packed.
    pub hash: String,
    /// And what the browser was told to call that.
    pub version: String,
}

/// The version an add-on nobody has packed before is given.
pub const FIRST: &str = "1.0.0";

/// The note, read.
pub fn read(said: &str) -> Option<Stamp> {
    let (hash, version) = said.trim().split_once(' ')?;
    match hash.is_empty() || version.is_empty() {
        true => None,
        false => Some(Stamp { hash: hash.to_string(), version: version.to_string() }),
    }
}

/// And written.
pub fn written(stamp: &Stamp) -> String {
    format!("{} {}\n", stamp.hash, stamp.version)
}

/// The next version after this one, which is the last number and one.
pub fn next(was: Option<&str>) -> String {
    let Some(was) = was else { return FIRST.to_string() };
    let Some((front, last)) = was.rsplit_once('.') else { return FIRST.to_string() };
    match last.parse::<u32>() {
        Ok(number) => format!("{front}.{}", number + 1),
        Err(_) => FIRST.to_string(),
    }
}

/// The version of what is already packed, read out of the packed file itself.
///
/// Nothing here compresses, so the manifest inside the archive is the manifest
/// as it was written and the version in it can be found by looking. This is
/// not the ordinary answer -- the note beside it is -- it is the answer for the
/// day the note has gone and the archive has not, which would otherwise be a
/// browser holding a newer add-on than the one being handed to it and rightly
/// keeping the one it has.
pub fn packed(bytes: &[u8]) -> Option<String> {
    let mark = b"\"version\": \"";
    let at = bytes.windows(mark.len()).position(|window| window == mark)? + mark.len();
    let rest = &bytes[at..];
    let end = rest.iter().position(|byte| *byte == b'"')?;
    String::from_utf8(rest[..end].to_vec()).ok().filter(|said| !said.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Nothing packed before, and nothing anybody can make sense of, are the
    /// same answer: start again, and go up from there.
    #[test]
    fn nothing_to_go_up_from_starts_at_the_first_one() {
        assert_eq!(next(None), FIRST);
        assert_eq!(next(Some("what")), FIRST);
    }

    #[test]
    fn the_version_can_be_read_back_out_of_what_was_packed() {
        let held = crate::pack::zip(&crate::source::every("1.2.3", ":root { --pink: #ffb5e2; }"));
        assert_eq!(packed(&held).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn an_archive_that_is_not_ours_says_nothing_about_a_version() {
        assert_eq!(packed(b"PK\x03\x04 and nothing else"), None);
    }
}
