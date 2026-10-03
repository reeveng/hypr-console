//! A chapter cut into the pages a screen holds.
//!
//! How much of a chapter fits on a page is a question about the face, the
//! size and the width, and only Pango can answer the part of it that is
//! where a paragraph breaks into lines. So the breaking is handed in, as
//! `wrap`, and what is left here is stacking: a line goes on the page if it
//! fits under the last one, and starts the next page if it does not. A
//! paragraph is split across two pages at a line, the way a printed book is,
//! rather than moved whole to the next page and leaving a hole.
//!
//! A picture is a page of its own. Fitting one between two paragraphs at
//! whatever height was left over is how a figure ends up the size of a stamp,
//! and on a screen this size a picture worth putting in a book is worth the
//! screen.
//!
//! Every line says which block of the chapter it came out of and where in that
//! block's text it starts, because a note is kept against the words rather
//! than against the screen: the same letters are on another line, or another
//! page, once the size changes, and a highlight has to follow them there.
//!
//! Nothing here has heard of a font. A test hands in a `wrap` that breaks
//! every so many letters, and the arithmetic is the same arithmetic.

use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::flow::Block;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Body,
    Heading,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub style: Style,
    pub top: u32,
    pub block: u32,
    pub start: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Text(Vec<Line>),
    Picture(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub height: u32,
    pub line_height: u32,
    pub heading_height: u32,
}

pub fn letters(page: &Page) -> Result<u64, Never> {
    Ok(match page {
        Page::Text(lines) => lines.iter().fold(0_u64, |sum, line| {
            let Ok(count) = fitted::<_, u64>(line.text.chars().count());

            sum.saturating_add(count)
        }),
        Page::Picture(_) => 1,
    })
}

pub type Wrap<'a> = &'a dyn Fn(&str, Style) -> Vec<String>;

struct Paginator {
    pages: Vec<Page>,
    lines: Vec<Line>,
    y: u32,
    layout: Layout,
}

impl Paginator {
    fn next_page(&mut self) -> Result<(), Never> {
        match self.lines.is_empty() {
            true => {},
            false => {
                let lines = std::mem::take(&mut self.lines);

                self.pages.push(Page::Text(lines));
            },
        }

        self.y = 0;

        Ok(())
    }

    fn add(&mut self, text: &str, style: Style, block: u32, wrap: Wrap<'_>) -> Result<(), Never> {
        let tall = match style {
            Style::Body => self.layout.line_height,
            Style::Heading => self.layout.heading_height,
        };

        let gap = match (self.lines.is_empty(), style) {
            (true, _) => 0,
            (false, Style::Body) => self.layout.line_height.saturating_div(2),
            (false, Style::Heading) => self.layout.line_height,
        };

        self.y = self.y.saturating_add(gap);

        let mut from = 0_u32;

        for line in wrap(text, style) {
            let Ok(after) = console_core_number_conversion::index(from);
            let Ok(start) = match text.get(after..).and_then(|rest| rest.find(line.as_str())).map(fitted::<_, u32>) {
                Some(Ok(found)) => Ok::<u32, Never>(from.saturating_add(found)),
                None => Ok(from),
            };
            let Ok(wide) = fitted::<_, u32>(line.len());

            from = start.saturating_add(wide);

            let past = self.y.saturating_add(tall);

            match (past > self.layout.height, self.lines.is_empty()) {
                (true, false) => {
                    let Ok(()) = self.next_page();
                },
                (true, true) | (false, _) => {},
            }

            self.lines.push(Line { text: line, style, top: self.y, block, start });
            self.y = self.y.saturating_add(tall);
        }

        Ok(())
    }
}

pub fn paginate(blocks: &[Block], layout: Layout, wrap: Wrap<'_>) -> Result<Vec<Page>, Never> {
    let mut paginator = Paginator { pages: Vec::new(), lines: Vec::new(), y: 0, layout };

    for (at, block) in (0_u32..).zip(blocks) {
        let Ok(()) = match block {
            Block::Picture(named) => {
                let Ok(()) = paginator.next_page();

                paginator.pages.push(Page::Picture(named.clone()));

                Ok(())
            },
            Block::Heading(text) => paginator.add(text, Style::Heading, at, wrap),
            Block::Paragraph(text) => paginator.add(text, Style::Body, at, wrap),
        };
    }

    let Ok(()) = paginator.next_page();

    Ok(match paginator.pages.is_empty() {
        true => vec![Page::Text(Vec::new())],
        false => paginator.pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_TEN_LETTERS: fn(&str, Style) -> Vec<String> = |text, _style| {
        let letters: Vec<char> = text.chars().collect();

        letters.chunks(10).map(|line| line.iter().collect()).collect()
    };

    const LAYOUT: Layout = Layout { height: 100, line_height: 20, heading_height: 30 };

    fn lines(page: &Page) -> Result<Vec<(String, u32)>, Never> {
        match page {
            Page::Text(lines) => Ok(lines.iter().map(|line| (line.text.clone(), line.top)).collect()),
            Page::Picture(named) => Ok(vec![(named.clone(), 0)]),
        }
    }

    #[test]
    fn lines_stack_until_the_page_is_full_and_the_rest_starts_the_next() {
        let blocks = vec![Block::Paragraph("a".repeat(70))];
        let Ok(pages) = paginate(&blocks, LAYOUT, &EVERY_TEN_LETTERS);
        let Ok(drawn) = pages.iter().map(lines).collect::<Result<Vec<Vec<(String, u32)>>, Never>>();
        let line = "a".repeat(10);

        assert_eq!(drawn.len(), 2, "five lines of twenty fit in a hundred and seven do not");
        assert_eq!(drawn.first().map(Vec::len), Some(5));
        assert_eq!(drawn.get(1), Some(&vec![(line.clone(), 0), (line, 20)]));
    }

    #[test]
    fn a_paragraph_after_another_is_half_a_line_further_down() {
        let blocks = vec![Block::Paragraph("one".to_string()), Block::Paragraph("two".to_string())];
        let Ok(pages) = paginate(&blocks, LAYOUT, &EVERY_TEN_LETTERS);
        let Ok(drawn) = pages.iter().map(lines).collect::<Result<Vec<Vec<(String, u32)>>, Never>>();

        assert_eq!(drawn, vec![vec![("one".to_string(), 0), ("two".to_string(), 30)]]);
    }

    #[test]
    fn a_picture_is_a_page_of_its_own() {
        let blocks = vec![
            Block::Paragraph("before".to_string()),
            Block::Picture("map.png".to_string()),
            Block::Paragraph("after".to_string()),
        ];
        let Ok(pages) = paginate(&blocks, LAYOUT, &EVERY_TEN_LETTERS);

        assert_eq!(pages.get(1), Some(&Page::Picture("map.png".to_string())));
        assert_eq!(pages.len(), 3);
    }

    #[test]
    fn a_line_knows_the_block_and_the_letter_it_starts_at() {
        let blocks = vec![Block::Heading("One".to_string()), Block::Paragraph("abcdefghij klmno".to_string())];
        let Ok(pages) = paginate(&blocks, LAYOUT, &|text: &str, _style: Style| text.split(' ').map(str::to_string).collect());
        let starts: Vec<(u32, u32)> = pages
            .iter()
            .flat_map(|page| match page {
                Page::Text(lines) => lines.iter().map(|line| (line.block, line.start)).collect(),
                Page::Picture(_) => Vec::new(),
            })
            .collect();

        assert_eq!(starts, vec![(0, 0), (1, 0), (1, 11)], "the space the wrap dropped is not counted as a letter of the next line");
    }

    #[test]
    fn a_chapter_with_nothing_in_it_is_still_one_page() {
        assert_eq!(paginate(&[], LAYOUT, &EVERY_TEN_LETTERS), Ok(vec![Page::Text(Vec::new())]));
    }
}
