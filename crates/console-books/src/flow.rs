//! A chapter, as the paragraphs a page is made of.
//!
//! What a chapter's markup says about how it looks is a stylesheet's worth of
//! decisions -- a face, a margin, an indent, a drop capital -- and on a screen
//! held at arm's length every one of them is either what this reader already
//! does or something it does not want. What the markup says about what the
//! chapter *is* comes to three things: a heading, a paragraph, and a picture
//! standing on its own. So that is what is kept.
//!
//! A paragraph ends where a block ends, whichever block it was: a `p`, a `div`,
//! a list item, a heading. Words outside any block -- which a chapter written
//! by hand has plenty of -- are a paragraph of their own rather than lost. Runs
//! of white space are one space, because in markup a line break is where the
//! author's editor wrapped rather than where the author meant one, and a `br`
//! is the line break that was meant. What is inside the head, a script or a
//! style is not the chapter at all.
//!
//! A picture is named by the path the markup gives, which is relative to the
//! chapter; [`resolved`] turns it into the name the book's zip knows it by.

use console_core_never::Never;

use crate::markup::{self, Token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading(String),
    Paragraph(String),
    Picture(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Element {
    Block,
    Heading,
    Hidden,
    Break,
    Picture,
    Inline,
}

fn element(name: &str) -> Result<Element, Never> {
    Ok(match name {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Element::Heading,
        "p" | "div" | "li" | "blockquote" | "section" | "article" | "tr" | "dt" | "dd" | "pre"
        | "figure" | "figcaption" | "td" | "th" | "aside" | "header" | "footer" | "hr" | "table"
        | "ul" | "ol" | "body" => Element::Block,
        "head" | "script" | "style" | "title" => Element::Hidden,
        "br" => Element::Break,
        "img" | "image" => Element::Picture,
        _ => Element::Inline,
    })
}

#[derive(Debug, Default)]
struct BlockBuilder {
    contents: String,
    heading: u32,
    hidden: u32,
    blocks: Vec<Block>,
}

impl BlockBuilder {
    fn text(&mut self, words: &str) -> Result<(), Never> {
        match self.hidden {
            0 => {},
            _ => return Ok(()),
        }

        for letter in words.chars() {
            match (letter.is_whitespace() && letter != '\u{a0}', self.contents.ends_with([' ', '\n']) || self.contents.is_empty()) {
                (true, true) => {},
                (true, false) => self.contents.push(' '),
                (false, _) => self.contents.push(letter),
            }
        }

        Ok(())
    }

    fn line_break(&mut self) -> Result<(), Never> {
        let kept = self.contents.trim_end_matches(' ').to_string();

        self.contents = kept;

        match self.contents.is_empty() {
            true => {},
            false => self.contents.push('\n'),
        }

        Ok(())
    }

    fn end_block(&mut self) -> Result<(), Never> {
        let text = self.contents.trim().to_string();

        self.contents.clear();

        match (text.is_empty(), self.heading) {
            (true, _) => {},
            (false, 0) => self.blocks.push(Block::Paragraph(text)),
            (false, _) => self.blocks.push(Block::Heading(text)),
        }

        Ok(())
    }

    fn start_tag(&mut self, name: &str, attributes: &[(String, String)]) -> Result<(), Never> {
        let Ok(element) = element(name);

        match element {
            Element::Heading => {
                let Ok(()) = self.end_block();

                self.heading = self.heading.saturating_add(1);
            },
            Element::Block => {
                let Ok(()) = self.end_block();
            },
            Element::Hidden => self.hidden = self.hidden.saturating_add(1),
            Element::Break => {
                let Ok(()) = self.line_break();
            },
            Element::Picture => {
                let Ok(source) = markup::attribute(attributes, "src");
                let Ok(linked) = markup::attribute(attributes, "href");

                match (source.or(linked), self.hidden) {
                    (Some(named), 0) => {
                        let Ok(()) = self.end_block();

                        self.blocks.push(Block::Picture(named.to_string()));
                    },
                    (Some(_), _) | (None, _) => {},
                }
            },
            Element::Inline => {},
        }

        Ok(())
    }

    fn end_tag(&mut self, name: &str) -> Result<(), Never> {
        let Ok(element) = element(name);

        match element {
            Element::Heading => {
                let Ok(()) = self.end_block();

                self.heading = self.heading.saturating_sub(1);
            },
            Element::Block => {
                let Ok(()) = self.end_block();
            },
            Element::Hidden => self.hidden = self.hidden.saturating_sub(1),
            Element::Break | Element::Picture | Element::Inline => {},
        }

        Ok(())
    }
}

pub fn blocks(page: &str) -> Result<Vec<Block>, Never> {
    let Ok(tokens) = markup::tokenize(page);
    let mut builder = BlockBuilder::default();

    for piece in &tokens {
        let Ok(()) = match piece {
            Token::StartTag { name, attributes } => builder.start_tag(name, attributes),
            Token::EndTag { name } => builder.end_tag(name),
            Token::Text(words) => builder.text(words),
        };
    }

    let Ok(()) = builder.end_block();

    Ok(builder.blocks)
}

fn percent_decode(text: &str) -> Result<String, Never> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut rest = text.as_bytes();

    while let Some((first, after)) = rest.split_first() {
        let byte = match after.get(..2).map(std::str::from_utf8) {
            Some(Ok(hexadecimal)) => match u8::from_str_radix(hexadecimal, 16) {
                Ok(byte) => Some(byte),
                Err(_not_hex) => None,
            },
            Some(Err(_)) | None => None,
        };

        rest = match (*first, byte, after.get(2..)) {
            (b'%', Some(byte), Some(past)) => {
                bytes.push(byte);

                past
            },
            (other, _, _) => {
                bytes.push(other);

                after
            },
        };
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relative<'a> {
    pub base: &'a str,
    pub path: &'a str,
}

pub fn resolved(link: Relative<'_>) -> Result<String, Never> {
    let Relative { base: from, path: link } = link;
    let link = match link.split_once('#') {
        Some((link, _fragment)) => link,
        None => link,
    };

    let Ok(link) = percent_decode(link);

    let folder = match from.rsplit_once('/') {
        Some((folder, _file)) => folder,
        None => "",
    };

    let mut parts: Vec<&str> = match link.starts_with('/') {
        true => Vec::new(),
        false => folder.split('/').filter(|part| !part.is_empty()).collect(),
    };

    for part in link.split('/') {
        match part {
            "" | "." => {},
            ".." => {
                let _ = parts.pop();
            },
            part => parts.push(part),
        }
    }

    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(text: &str) -> Block {
        Block::Paragraph(text.to_string())
    }

    #[test]
    fn a_chapter_is_its_headings_and_paragraphs_with_the_head_left_out() {
        let page = "<html><head><title>One</title><style>p { x }</style></head><body>\
            <h1>Chapter <i>One</i></h1>\n<p>It was the best\n   of times,</p><p>it was the <b>worst</b>.</p></body></html>";

        assert_eq!(
            blocks(page),
            Ok(vec![
                Block::Heading("Chapter One".to_string()),
                paragraph("It was the best of times,"),
                paragraph("it was the worst."),
            ])
        );
    }

    #[test]
    fn a_break_is_a_line_and_a_picture_stands_between_paragraphs() {
        let page = "<p>Roses are red,<br/>violets are blue</p><div><img src=\"../img/rose.jpg\" alt=\"\"/></div><p>End</p>";

        assert_eq!(
            blocks(page),
            Ok(vec![
                paragraph("Roses are red,\nviolets are blue"),
                Block::Picture("../img/rose.jpg".to_string()),
                paragraph("End"),
            ])
        );
    }

    #[test]
    fn words_outside_any_block_are_still_read() {
        assert_eq!(blocks("<body>Just words</body>"), Ok(vec![paragraph("Just words")]));
    }

    #[test]
    fn a_link_is_named_the_way_the_zip_names_it() {
        assert_eq!(resolved(Relative { base: "OEBPS/text/one.xhtml", path: "../images/a%20b.jpg#top" }), Ok("OEBPS/images/a b.jpg".to_string()));
        assert_eq!(resolved(Relative { base: "content.opf", path: "chapter.xhtml" }), Ok("chapter.xhtml".to_string()));
        assert_eq!(resolved(Relative { base: "OEBPS/content.opf", path: "./text/one.xhtml" }), Ok("OEBPS/text/one.xhtml".to_string()));
    }
}
