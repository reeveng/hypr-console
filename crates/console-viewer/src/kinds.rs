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

/// What one thing is, as far as this panel is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A still image. Looked at: fitted, zoomed, turned.
    Picture,
    /// A moving one. Watched: played, paused, moved along.
    Film,
}

impl Kind {
    /// What the card calls it, where it has to say the word.
    ///
    /// Singular and lowercase, because every place this is used has it in the
    /// middle of a sentence rather than at the top of a card.
    pub fn says(self) -> &'static str {
        match self {
            Kind::Picture => "picture",
            Kind::Film => "film",
        }
    }
}

/// Which kind a media type is, or nothing where it is neither.
///
/// Read off the prefix rather than off a list of types. A list here would be a
/// second copy of the families in `console_settings::defaults::KINDS` and would
/// go out of date the first time somebody added a type to one and not the
/// other -- and the failure of that is silent, because a type this did not
/// recognise would simply be a file the panel refused to show.
///
/// The prefix is safe to read in a way the family is not. `image/` and
/// `video/` are the media type registry's own top-level types and are not a
/// judgement anybody here is making; whether `image/x-canon-cr2` is a
/// photograph a person wants to look at *is* a judgement, and it is made by
/// what the desktop file claims, not here.
pub fn of(mime: &str) -> Option<Kind> {
    match () {
        () if mime.starts_with("image/") => Some(Kind::Picture),
        () if mime.starts_with("video/") => Some(Kind::Film),
        () => None,
    }
}

/// Whether a thing is one this panel will open at all.
pub fn shows(mime: &str) -> Shows {
    match of(mime) {
        Some(_) => Shows::It,
        None => Shows::Not,
    }
}

/// Whether this panel has anything to do with a thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    /// It is a picture or a film, so the panel can show it.
    It,
    /// It is neither, so it is not this panel's business.
    Not,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_photograph_is_a_picture_and_a_film_is_a_film() {
        assert_eq!(of("image/jpeg"), Some(Kind::Picture));
        assert_eq!(of("image/png"), Some(Kind::Picture));
        assert_eq!(of("video/mp4"), Some(Kind::Film));
        assert_eq!(of("video/matroska"), Some(Kind::Film));
    }

    /// The point of reading the prefix: a type nobody here has ever written
    /// down still lands in the right half.
    #[test]
    fn a_type_nothing_here_names_is_still_placed_by_its_prefix() {
        assert_eq!(of("image/x-canon-cr2"), Some(Kind::Picture));
        assert_eq!(of("video/x-ms-wmv"), Some(Kind::Film));
    }

    #[test]
    fn music_and_writing_are_not_this_panels_business() {
        assert_eq!(of("audio/mpeg"), None);
        assert_eq!(of("audio/x-opus+ogg"), None);
        assert_eq!(of("text/plain"), None);
        assert_eq!(of("inode/directory"), None);
        assert_eq!(shows("audio/flac"), Shows::Not);
    }

    /// A type that merely has the word in it somewhere is not the prefix. The
    /// `/` matters: `application/vnd.image-thing` is not an image.
    #[test]
    fn the_prefix_is_the_front_of_the_type_and_not_a_word_in_it() {
        assert_eq!(of("application/vnd.image-thing"), None);
        assert_eq!(of("text/video-script"), None);
        assert_eq!(of(""), None);
    }

    #[test]
    fn each_kind_says_what_it_is_in_a_word() {
        assert_eq!(Kind::Picture.says(), "picture");
        assert_eq!(Kind::Film.says(), "film");
        for kind in [Kind::Picture, Kind::Film] {
            let said = kind.says();
            assert!(!said.is_empty());
            assert_eq!(said, said.to_lowercase(), "said at the top of a sentence");
        }
    }
}
