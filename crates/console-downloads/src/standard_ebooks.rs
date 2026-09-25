//! Books, from Standard Ebooks, beside Gutenberg.
//!
//! Standard Ebooks takes books whose copyright has run out, mostly out of
//! Gutenberg, and sets them again by hand: the typography corrected, a proper
//! cover, a table of contents that goes where it says. It is the same promise
//! the tab was built on -- its own domain, nothing to click through, nothing to
//! install, what arrives is the EPUB -- so a book it has comes from it rather
//! than as Gutenberg's plainer copy, and a book it does not have still comes
//! from Gutenberg.
//!
//! Its search is the OPDS feed the site offers, which answers in well under a
//! second. A book is known by the path of its page -- author, title and
//! translator -- and the EPUB sits under that page by a name built from the same
//! path, so a press is fetched from the library's page rather than from a link
//! out of the answer, the same way a Gutenberg book is fetched by its number.
//! The address carries the feed's own `source=feed`, because without it the
//! site answers a download with a page thanking the reader for it, and that
//! page is what landed in Books under the book's name.

use console_core_never::Never;

use crate::looking::Found;
use crate::opds::{self, Byline, Entry, Feed};

pub const SEARCH: &str = "https://standardebooks.org/feeds/opds/all";

pub const BOOK_PAGE: &str = "https://standardebooks.org/ebooks/";

const STRAIGHT_TO_THE_FILE: &str = "source=feed";

pub fn search(asked: &str) -> Result<Vec<String>, Never> {
    opds::search(Feed(SEARCH), asked)
}

pub fn found_in(text: &str) -> Result<Vec<Found>, Never> {
    opds::found_in(text, Byline::Name, book)
}

fn book(entry: Entry) -> Result<Option<Found>, Never> {
    let Ok(id) = id_in(&entry.identifier);

    Ok(match (id, entry.title.is_empty()) {
        (Some(id), false) => Some(Found {
            id,
            title: entry.title,
            url: entry.identifier,
            by: entry.by,
            seconds: 0,
            views: 0,
            live: false,
            picture: entry.picture,
        }),
        (Some(_), true) | (None, _) => None,
    })
}

pub fn id_in(page: &str) -> Result<Option<String>, Never> {
    let path = match page.strip_prefix(BOOK_PAGE) {
        Some(path) => path.trim_end_matches('/'),
        None => return Ok(None),
    };

    crate::store::named(&path.replace('/', "_"))
}

pub fn publication(page: &str) -> Result<Option<String>, Never> {
    let Ok(id) = id_in(page);

    Ok(id.map(|id| format!("{}/downloads/{id}.epub?{STRAIGHT_TO_THE_FILE}", page.trim_end_matches('/'))))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/terms/">
	<id>https://standardebooks.org/feeds/opds/all</id>
	<title>All Standard Ebooks</title>
	<author><name>Standard Ebooks</name></author>
	<entry>
	<id>https://standardebooks.org/ebooks/marcus-aurelius/meditations/george-long</id>
	<title>Meditations</title>
	<author>
		<name>Marcus Aurelius</name>
		<uri>https://standardebooks.org/ebooks/marcus-aurelius</uri>
	</author>
	<content type="html">&lt;p&gt;Roman Emperor&lt;/p&gt;</content>
	<link href="https://standardebooks.org/ebooks/marcus-aurelius/meditations/george-long/downloads/cover.jpg" rel="http://opds-spec.org/image" type="image/jpeg"/>
	<link href="https://standardebooks.org/ebooks/marcus-aurelius/meditations/george-long/downloads/cover-thumbnail.jpg" rel="http://opds-spec.org/image/thumbnail" type="image/jpeg"/>
	<link href="https://elsewhere.example/meditations.epub" rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" />
	</entry>
	<entry>
	<id>https://elsewhere.example/ebooks/a-stranger</id>
	<title>Not From Here</title>
	</entry>
</feed>"#;

    fn found() -> Vec<Found> {
        let Ok(found) = found_in(ANSWER);

        found
    }

    #[test]
    fn a_book_is_an_entry_whose_page_is_on_standard_ebooks() {
        let titles: Vec<String> = found().into_iter().map(|found| found.title).collect();

        assert_eq!(titles, vec!["Meditations"]);
    }

    #[test]
    fn the_feed_author_is_not_taken_for_a_book_author() {
        assert_eq!(found()[0].by, "Marcus Aurelius");
    }

    #[test]
    fn the_picture_is_the_thumbnail_rather_than_the_whole_cover() {
        assert_eq!(
            found()[0].picture,
            "https://standardebooks.org/ebooks/marcus-aurelius/meditations/george-long/downloads/cover-thumbnail.jpg"
        );
    }

    #[test]
    fn a_book_is_fetched_from_its_page_whatever_link_the_answer_carried() {
        assert_eq!(found()[0].id, "marcus-aurelius_meditations_george-long");
        assert_eq!(
            publication(&found()[0].url),
            Ok(Some(
                "https://standardebooks.org/ebooks/marcus-aurelius/meditations/george-long/downloads/marcus-aurelius_meditations_george-long.epub?source=feed"
                    .to_string()
            ))
        );
        assert_eq!(publication("https://www.gutenberg.org/ebooks/2680"), Ok(None));
    }

    #[test]
    fn what_is_typed_is_one_encoded_value_and_never_a_flag() {
        let Ok(arguments) = search("--output /etc/passwd");

        assert!(arguments.contains(&"query=--output /etc/passwd".to_string()));
        assert_eq!(arguments.last().map(String::as_str), Some(SEARCH));
    }
}
