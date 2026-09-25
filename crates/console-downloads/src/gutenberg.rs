//! Books, from Project Gutenberg, and from nowhere else.
//!
//! A book tab was asked for on the condition that someone new to the internet
//! could press anything on it and come to no harm, and that is the whole of why
//! it is this one library. Gutenberg holds books whose copyright has run out,
//! serves them from its own domain, and has no advertising to click through and
//! nothing to install: what arrives is the EPUB it names. The sites that carry
//! manga and newer books are the opposite on every count, which is why there is
//! no tab for them.
//!
//! Gutenberg's own catalog is the search, asked for the OPDS feed its search
//! page is also written as, most read first, so the first ten rows are the
//! books people mean. A book is then fetched from gutenberg.org by its number,
//! so the only address a press can reach is the library's.
//!
//! Gutendex was the search before it: the same catalog answered as JSON, by a
//! volunteer's server rather than the library's. It took half a minute or more
//! for any word it had not been asked lately, which is every word a person on
//! this device types, so the search sat on "Searching for" until curl gave up.
//! The library answers the same question in under a second.
//!
//! A feed lists the author and subject headings that matched before the books.
//! A book is the entry whose identifier is a book's own page; the rest are
//! headings and are stepped over.

use console_core_never::Never;

use crate::looking::Found;
use crate::opds::{self, Byline, Entry, Feed};

pub const SEARCH: &str = "https://www.gutenberg.org/ebooks/search.opds/";

pub const BOOK_PAGE: &str = "https://www.gutenberg.org/ebooks/";

pub const PUBLICATION: &str = "epub3.images";

const FEED_ENDING: &str = ".opds";

const COVERS: &str = "https://www.gutenberg.org/cache/epub/";

pub const NOT_ANSWERING: &str = "Project Gutenberg and Standard Ebooks aren't answering. Try again later.";

pub fn search(asked: &str) -> Result<Vec<String>, Never> {
    opds::search(Feed(SEARCH), asked)
}

pub fn found_in(text: &str) -> Result<Vec<Found>, Never> {
    opds::found_in(text, Byline::Content, book)
}

fn book(entry: Entry) -> Result<Option<Found>, Never> {
    let written = entry.identifier.strip_prefix(BOOK_PAGE).and_then(|rest| rest.strip_suffix(FEED_ENDING));

    let number = match written.map(str::parse::<u64>) {
        Some(Ok(number)) => number,
        Some(Err(_)) | None => return Ok(None),
    };

    Ok(match entry.title.is_empty() {
        false => Some(Found {
            id: number.to_string(),
            title: entry.title,
            url: format!("{BOOK_PAGE}{number}"),
            by: entry.by,
            seconds: 0,
            views: 0,
            live: false,
            picture: format!("{COVERS}{number}/pg{number}.cover.medium.jpg"),
        }),
        true => None,
    })
}

pub fn publication(page: &str) -> Result<String, Never> {
    Ok(format!("{page}.{PUBLICATION}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
<id>http://www.gutenberg.org/ebooks/search.opds/?query=meditations</id>
<title>Books: meditations</title>
<entry>
<id>https://www.gutenberg.org/ebooks/subjects/search.opds/?query=meditations</id>
<title>Subjects</title>
<content type="text">18 subject headings match your search.</content>
<link type="image/png" rel="http://opds-spec.org/image/thumbnail" href="data:image/png;base64,iVBOR"/>
</entry>
<entry>
<id>https://www.gutenberg.org/ebooks/2680.opds</id>
<title>Meditations</title>
<content type="text">Emperor of Rome Marcus Aurelius</content>
<link type="application/atom+xml;profile=opds-catalog" rel="subsection" href="/ebooks/2680.opds"/>
</entry>
<entry>
<id>https://www.gutenberg.org/ebooks/1653.opds</id>
<title>The Imitation of Christ</title>
<content type="text">&#224; Kempis Thomas</content>
</entry>
<entry>
<id>https://www.gutenberg.org/ebooks/99.opds</id>
<title></title>
</entry>
</feed>"#;

    fn found() -> Vec<Found> {
        let Ok(found) = found_in(ANSWER);

        found
    }

    #[test]
    fn a_heading_is_not_a_book_and_a_book_needs_a_title() {
        let titles: Vec<String> = found().into_iter().map(|found| found.title).collect();

        assert_eq!(titles, vec!["Meditations", "The Imitation of Christ"]);
    }

    #[test]
    fn the_author_is_said_as_the_library_says_it() {
        assert_eq!(found()[0].by, "Emperor of Rome Marcus Aurelius");
        assert_eq!(found()[1].by, "\u{e0} Kempis Thomas");
    }

    #[test]
    fn the_cover_is_the_one_the_library_keeps_for_that_number() {
        assert_eq!(found()[0].picture, "https://www.gutenberg.org/cache/epub/2680/pg2680.cover.medium.jpg");
    }

    #[test]
    fn a_book_is_fetched_from_gutenberg_whatever_link_the_answer_carried() {
        assert_eq!(found()[0].url, "https://www.gutenberg.org/ebooks/2680");
        assert_eq!(publication(&found()[0].url), Ok("https://www.gutenberg.org/ebooks/2680.epub3.images".to_string()));
    }

    #[test]
    fn an_answer_that_is_not_one_is_no_books() {
        assert_eq!(found_in("<html>busy</html>"), Ok(Vec::new()));
        assert_eq!(found_in(""), Ok(Vec::new()));
    }

    #[test]
    fn what_is_typed_is_one_encoded_value_and_never_a_flag() {
        let Ok(arguments) = search("--output /etc/passwd");

        assert!(arguments.contains(&"query=--output /etc/passwd".to_string()));
        assert_eq!(arguments.last().map(String::as_str), Some(SEARCH));
    }
}
