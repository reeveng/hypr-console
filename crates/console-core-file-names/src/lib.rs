//! The name a fetched thing is kept under, and the title read back out of it.
//!
//! Downloads keeps what it fetches as `Title [id].ext`: the title a person
//! reads, the site's id so the same thing is not fetched twice, the ending the
//! file is. Two shelves then read those names, and each wrote its own reading.
//! The music player took the ` [id]` off and left the rest; the books shelf
//! left the ` [id]` on and turned underscores into spaces. So the same book
//! was "Meditations" on the download panel and "Meditations [2680]" in Books,
//! and a song kept as `a_song.opus` was "a_song" where a book would have said
//! "a song".
//!
//! One reading now, which does both: the ending goes, a bracketed id at the
//! end goes, and an underscore is read as the space somebody could not type
//! into a file name. A title with a real underscore in it loses it on the
//! shelf, which is the cheaper of the two mistakes -- a name with its id still
//! showing is a mistake on every row, and an underscore that meant an
//! underscore is rare enough to be the name's own problem.

use console_core_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stored<'a> {
    pub title: &'a str,
    pub id: Option<&'a str>,
    pub ending: &'a str,
}

impl Stored<'_> {
    pub fn name(&self) -> Result<String, Never> {
        let Stored { title, id, ending } = self;

        Ok(match id {
            Some(id) => format!("{title} [{id}].{ending}"),
            None => format!("{title}.{ending}"),
        })
    }
}

pub fn title(name: &str) -> Result<String, Never> {
    let stem = match name.rsplit_once('.') {
        Some((stem, _ending)) => stem,
        None => name,
    };

    let Ok(without) = without_id(stem);

    Ok(without.replace('_', " ").trim().to_string())
}

fn without_id(stem: &str) -> Result<&str, Never> {
    Ok(match stem.rsplit_once(" [") {
        Some((title, tail)) => match tail.ends_with(']') {
            true => title,
            false => stem,
        },
        None => stem,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_a_download_was_kept_under_is_not_part_of_its_title() {
        assert_eq!(title("Meditations [2680].epub"), Ok("Meditations".to_string()));
        assert_eq!(title("505 [qU9mHegkTc4].opus"), Ok("505".to_string()));
    }

    #[test]
    fn a_name_is_read_back_as_the_title_it_was_kept_under() {
        let Ok(kept) = Stored { title: "Frankenstein; or the modern prometheus", id: Some("84"), ending: "epub" }.name();

        assert_eq!(kept, "Frankenstein; or the modern prometheus [84].epub");
        assert_eq!(title(&kept), Ok("Frankenstein; or the modern prometheus".to_string()));
    }

    #[test]
    fn an_underscore_is_the_space_a_file_name_could_not_hold() {
        assert_eq!(title("war_and_peace.epub"), Ok("war and peace".to_string()));
    }

    #[test]
    fn a_bracket_that_is_part_of_the_title_stays() {
        assert_eq!(title("227.Pink + White.flac"), Ok("227.Pink + White".to_string()));
        assert_eq!(title("Live [at the Hall] again.mp3"), Ok("Live [at the Hall] again".to_string()));
        assert_eq!(title("no ending"), Ok("no ending".to_string()));
    }
}
