//! What an EPUB says about itself: its title, whose it is, its cover, and the
//! order its chapters are read in.
//!
//! Three files answer that, one after the other. `META-INF/container.xml`
//! names the package file; the package file lists everything in the book under
//! an identifier (the manifest), the identifiers a reader walks through in order (the spine),
//! and the title and author (the metadata). Where the cover is has been said
//! three ways over the format's life -- a `cover-image` property on the item in
//! EPUB 3, a `<meta name="cover">` pointing at an identifier in EPUB 2, and nothing at
//! all but an item called `cover` in books nobody told -- and all three are
//! asked, newest first, because a library of covers with holes in it is one
//! somebody stops trusting.
//!
//! A spine item that is not a page -- an image the spine names directly, which
//! some comics do -- is still read in order, as a picture on its own.

use console_core_never::Never;

use std::collections::BTreeMap;

use crate::flow::{self, Relative};
use crate::markup::{self, Token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicationError {
    NoContainer,
    NoPackage(String),
    NoChapters,
}

impl std::fmt::Display for PublicationError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublicationError::NoContainer => write!(to, "this book does not say where its contents are"),
            PublicationError::NoPackage(at) => write!(to, "this book names {at} as its contents and it is not there"),
            PublicationError::NoChapters => write!(to, "this book lists no chapters"),
        }
    }
}

impl std::error::Error for PublicationError {}

pub const CONTAINER: &str = "META-INF/container.xml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    pub title: Option<String>,
    pub author: Option<String>,
    pub cover: Option<String>,
    pub chapters: Vec<String>,
}

pub fn package_path(container: &str) -> Result<Option<String>, Never> {
    let Ok(tokens) = markup::tokenize(container);

    Ok(tokens.iter().find_map(|piece| match piece {
        Token::StartTag { name, attributes } => match name.as_str() {
            "rootfile" => {
                let Ok(path) = markup::attribute(attributes, "full-path");

                path.map(str::to_string)
            },
            _ => None,
        },
        Token::EndTag { .. } | Token::Text(_) => None,
    }))
}

#[derive(Debug, Clone, Default)]
struct Item {
    identifier: String,
    link: String,
    kind: String,
    properties: String,
}

#[derive(Debug, Default)]
struct Manifest {
    items: Vec<Item>,
    spine: Vec<String>,
    title: Option<String>,
    author: Option<String>,
    cover_identifier: Option<String>,
    current_element: Option<String>,
}

fn attribute(attributes: &[(String, String)], name: &str) -> Result<String, Never> {
    let Ok(found) = markup::attribute(attributes, name);

    Ok(match found {
        Some(found) => found.to_string(),
        None => String::new(),
    })
}

impl Manifest {
    fn start_tag(&mut self, name: &str, attributes: &[(String, String)]) -> Result<(), Never> {
        match name {
            "item" => {
                let Ok(identifier) = attribute(attributes, "id");
                let Ok(link) = attribute(attributes, "href");
                let Ok(kind) = attribute(attributes, "media-type");
                let Ok(properties) = attribute(attributes, "properties");

                self.items.push(Item { identifier, link, kind, properties });
            },
            "itemref" => {
                let Ok(identifier) = attribute(attributes, "idref");

                self.spine.push(identifier);
            },
            "meta" => {
                let Ok(named) = attribute(attributes, "name");
                let Ok(content) = attribute(attributes, "content");

                match named.as_str() {
                    "cover" => self.cover_identifier = Some(content),
                    _ => {},
                }
            },
            "title" | "creator" => self.current_element = Some(name.to_string()),
            _ => {},
        }

        Ok(())
    }

    fn text(&mut self, words: &str) -> Result<(), Never> {
        let words = words.trim();

        match (self.current_element.as_deref(), self.title.is_none(), self.author.is_none(), words.is_empty()) {
            (_, _, _, true) => {},
            (Some("title"), true, _, false) => self.title = Some(words.to_string()),
            (Some("creator"), _, true, false) => self.author = Some(words.to_string()),
            (Some(_), _, _, false) | (None, _, _, false) => {},
        }

        Ok(())
    }

    fn cover(&self) -> Result<Option<&Item>, Never> {
        let told = self.items.iter().find(|item| item.properties.split_whitespace().any(|word| word == "cover-image"));
        let named = self.cover_identifier.as_deref().and_then(|identifier| self.items.iter().find(|item| item.identifier == identifier));
        let guessed = self
            .items
            .iter()
            .find(|item| item.kind.starts_with("image/") && item.identifier.to_lowercase().contains("cover"));

        Ok(told.or(named).or(guessed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Package<'a> {
    pub path: &'a str,
    pub contents: &'a str,
}

pub fn parse(package: Package<'_>) -> Result<Publication, PublicationError> {
    let Package { path: package_at, contents: package } = package;
    let Ok(tokens) = markup::tokenize(package);
    let mut manifest = Manifest::default();

    for piece in &tokens {
        let Ok(()) = match piece {
            Token::StartTag { name, attributes } => manifest.start_tag(name, attributes),
            Token::EndTag { .. } => {
                manifest.current_element = None;

                Ok(())
            },
            Token::Text(words) => manifest.text(words),
        };
    }

    let Ok(cover) = manifest.cover();
    let cover = match cover {
        Some(item) => {
            let Ok(joined) = flow::resolved(Relative { base: package_at, path: &item.link });

            Some(joined)
        },
        None => None,
    };

    let mut chapters = Vec::new();
    let by_identifier: BTreeMap<&str, &Item> =
        manifest.items.iter().map(|item| (item.identifier.as_str(), item)).collect();

    for identifier in &manifest.spine {
        match by_identifier.get(identifier.as_str()) {
            Some(item) => {
                let Ok(joined) = flow::resolved(Relative { base: package_at, path: &item.link });

                chapters.push(joined);
            },
            None => {},
        }
    }

    match chapters.is_empty() {
        true => Err(PublicationError::NoChapters),
        false => Ok(Publication { title: manifest.title, author: manifest.author, cover, chapters }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKAGE: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Moby Dick; Or, The Whale</dc:title>
    <dc:creator opf:role="aut">Herman Melville</dc:creator>
    <meta name="cover" content="the-cover"/>
  </metadata>
  <manifest>
    <item id="the-cover" href="images/cover.jpg" media-type="image/jpeg"/>
    <item id="one" href="text/one.xhtml" media-type="application/xhtml+xml"/>
    <item id="two" href="text/two.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="two"/><itemref idref="one"/><itemref idref="missing"/></spine>
</package>"#;

    #[test]
    fn the_container_names_the_package() {
        let container = r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;

        assert_eq!(package_path(container), Ok(Some("OEBPS/content.opf".to_string())));
    }

    #[test]
    fn a_package_says_its_title_author_cover_and_the_order_of_its_chapters() {
        assert_eq!(
            parse(Package { path: "OEBPS/content.opf", contents: PACKAGE }),
            Ok(Publication {
                title: Some("Moby Dick; Or, The Whale".to_string()),
                author: Some("Herman Melville".to_string()),
                cover: Some("OEBPS/images/cover.jpg".to_string()),
                chapters: vec!["OEBPS/text/two.xhtml".to_string(), "OEBPS/text/one.xhtml".to_string()],
            })
        );
    }

    #[test]
    fn an_epub_three_cover_is_found_by_what_it_is_rather_than_what_it_is_called() {
        let package = r#"<package><manifest>
            <item id="img1" href="c.png" media-type="image/png" properties="cover-image"/>
            <item id="p" href="p.xhtml" media-type="application/xhtml+xml"/>
            </manifest><spine><itemref idref="p"/></spine></package>"#;

        let parsed = parse(Package { path: "content.opf", contents: package }).map(|epub| epub.cover);

        assert_eq!(parsed, Ok(Some("c.png".to_string())));
    }

    #[test]
    fn a_book_with_no_chapters_says_so() {
        assert_eq!(parse(Package { path: "a.opf", contents: "<package></package>" }), Err(PublicationError::NoChapters));
    }
}
