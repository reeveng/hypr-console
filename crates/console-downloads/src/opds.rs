//! An OPDS feed, which is how both libraries answer a search.
//!
//! Gutenberg and Standard Ebooks each had a curl line and a walk over the
//! feed's tags of their own, and the two differed in one tag: Gutenberg says
//! who wrote a book in the entry's `content`, Standard Ebooks in the author's
//! `name`. The feed is read here and the difference is a `Byline`. What an
//! entry becomes -- which of them is a book, and at what address -- is still
//! each library's own.

use console_books::markup::{self, Token};

use console_core_external_programs::Program;
use console_core_never::Never;

use crate::looking::{Found, MANY};

pub const PATIENCE: &str = "20";

const THUMBNAIL: &str = "http://opds-spec.org/image/thumbnail";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Feed(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Byline {
    Content,
    Name,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub identifier: String,
    pub title: String,
    pub by: String,
    pub picture: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Identifier,
    Title,
    Author,
    Elsewhere,
}

pub fn search(Feed(at): Feed, asked: &str) -> Result<Vec<String>, Never> {
    let Ok(curl) = Program::Curl.name();

    Ok(vec![
        curl.to_string(),
        "--silent".to_string(),
        "--show-error".to_string(),
        "--fail".to_string(),
        "--location".to_string(),
        "--max-time".to_string(),
        PATIENCE.to_string(),
        "--get".to_string(),
        "--data-urlencode".to_string(),
        format!("query={}", asked.trim()),
        "--".to_string(),
        at.to_string(),
    ])
}

fn tag(name: &str, byline: Byline) -> Result<Field, Never> {
    Ok(match (name, byline) {
        ("id", _) => Field::Identifier,
        ("title", _) => Field::Title,
        ("content", Byline::Content) | ("name", Byline::Name) => Field::Author,
        (_, Byline::Content | Byline::Name) => Field::Elsewhere,
    })
}

pub fn found_in(text: &str, byline: Byline, book: fn(Entry) -> Result<Option<Found>, Never>) -> Result<Vec<Found>, Never> {
    let Ok(tokens) = markup::tokenize(text);
    let Ok(many) = console_core_number_conversion::index(MANY);
    let mut found: Vec<Found> = Vec::new();
    let mut entry: Option<Entry> = None;
    let mut field = Field::Elsewhere;

    for token in tokens {
        match token {
            Token::StartTag { name, attributes } => {
                field = match name.as_str() {
                    "entry" => {
                        entry = Some(Entry::default());

                        Field::Elsewhere
                    },
                    "link" => {
                        let Ok(rel) = markup::attribute(&attributes, "rel");
                        let Ok(href) = markup::attribute(&attributes, "href");

                        match (&mut entry, rel, href) {
                            (Some(open), Some(THUMBNAIL), Some(href)) => open.picture = href.to_string(),
                            (Some(_) | None, _, _) => {},
                        }

                        Field::Elsewhere
                    },
                    other => {
                        let Ok(field) = tag(other, byline);

                        field
                    },
                };
            },
            Token::EndTag { name } => {
                field = Field::Elsewhere;

                match (name.as_str(), entry.take()) {
                    ("entry", Some(ended)) => {
                        let Ok(one) = book(ended);

                        found.extend(one);
                    },
                    (_, still) => entry = still,
                }
            },
            Token::Text(words) => match (&mut entry, field) {
                (Some(open), Field::Identifier) => open.identifier.push_str(words.trim()),
                (Some(open), Field::Title) => open.title.push_str(words.trim()),
                (Some(open), Field::Author) => open.by.push_str(words.trim()),
                (Some(_), Field::Elsewhere) | (None, _) => {},
            },
        }
    }

    found.truncate(many);

    Ok(found)
}
