//! What this panel can show, and what it cannot.
//!
//! Two kinds of thing, and the difference between them is the whole of what
//! the card does differently: a photograph is looked at and a film is watched.
//! One wants zooming and the other wants a transport, and nothing else about
//! the card changes.
//!
//! # The families are not written here
//!
//! Which types are pictures and which are film is this desktop's opinion, and
//! it is already written down once, in `console_settings::defaults::KINDS`.
//! Saying it again here would be the fault that entry exists to record: a kind
//! of thing is a family of types, and the type left out of the second copy is
//! the one that opens somewhere surprising.
//!
//! So what is here is the *shape* of the question -- given a type, is this a
//! picture, a film, or nothing this panel knows -- read off the prefix, which
//! is the one thing about a media type that is not this desktop's opinion.
//! `tests/the_families.rs` crosses what this panel's desktop file claims
//! against that list, from both ends, so a type added there and not to the
//! desktop file is a failing test rather than a file that opens in a browser.

use console_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Picture,
    Film,
}

impl Kind {
    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Kind::Picture => "picture",
            Kind::Film => "film",
        })
    }
}

pub fn of(mime: &str) -> Result<Option<Kind>, Never> {
    Ok(match () {
        () if mime.starts_with("image/") => Some(Kind::Picture),
        () if mime.starts_with("video/") => Some(Kind::Film),
        () => None,
    })
}

pub fn shows(mime: &str) -> Result<Shows, Never> {
    let Ok(of) = of(mime);

    Ok(match of {
        Some(_) => Shows::It,
        None => Shows::Not,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    It,
    Not,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_photograph_is_a_picture_and_a_film_is_a_film() {
        assert_eq!(of("image/jpeg"), Ok(Some(Kind::Picture)));
        assert_eq!(of("image/png"), Ok(Some(Kind::Picture)));
        assert_eq!(of("video/mp4"), Ok(Some(Kind::Film)));
        assert_eq!(of("video/matroska"), Ok(Some(Kind::Film)));
    }

    #[test]
    fn a_type_nothing_here_names_is_still_placed_by_its_prefix() {
        assert_eq!(of("image/x-canon-cr2"), Ok(Some(Kind::Picture)));
        assert_eq!(of("video/x-ms-wmv"), Ok(Some(Kind::Film)));
    }

    #[test]
    fn music_and_writing_are_not_this_panels_business() {
        assert_eq!(of("audio/mpeg"), Ok(None));
        assert_eq!(of("audio/x-opus+ogg"), Ok(None));
        assert_eq!(of("text/plain"), Ok(None));
        assert_eq!(of("inode/directory"), Ok(None));
        assert_eq!(shows("audio/flac"), Ok(Shows::Not));
    }

    #[test]
    fn the_prefix_is_the_front_of_the_type_and_not_a_word_in_it() {
        assert_eq!(of("application/vnd.image-thing"), Ok(None));
        assert_eq!(of("text/video-script"), Ok(None));
        assert_eq!(of(""), Ok(None));
    }

    #[test]
    fn each_kind_says_what_it_is_in_a_word() {
        assert_eq!(Kind::Picture.says(), Ok("picture"));
        assert_eq!(Kind::Film.says(), Ok("film"));

        for kind in [Kind::Picture, Kind::Film] {
            let Ok(said) = kind.says();

            assert!(!said.is_empty());
            assert_eq!(said, said.to_lowercase(), "said at the top of a sentence");
        }
    }
}
