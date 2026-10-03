//! The words on a page that is a picture, and where each one is.
//!
//! An EPUB's words are laid out here, so a highlight on one knows its letters
//! for free. A PDF page and a comic page arrive as pictures, and to highlight
//! a line of one the words and their boxes have to come from somewhere else.
//! Apple calls this Live Text. For a PDF they come from the PDF itself, which
//! usually carries its text and where each word is drawn: poppler's
//! `pdftotext -bbox-layout` hands both over as markup of blocks, lines and
//! words. A scanned PDF has no such layer, and a comic never has, so for those
//! the words are recognized off the picture by tesseract, whose TSV says the
//! same things one word to a row.
//!
//! Both are read into the one shape: paragraphs, each its words joined by a
//! space, and every word the letters it spans in that text and the box it
//! covers on the page. The box is kept as a share of the page rather than in
//! pixels, because the page is drawn at whatever size the screen gives it and
//! the text layer and the recognizer measure in two different units. A
//! paragraph being text with letters in it is what lets a highlight on a
//! picture be the same note as one on an EPUB: a block, and the two letters it
//! runs between.
//!
//! Japanese and Thai are written without a space between words, so two words
//! recognized side by side in either are joined with nothing between them, and
//! a highlight on a picture is widened to the words the recognizer found rather
//! than to the spaces around them. Korean is written with spaces and is joined
//! like English.
//!
//! Which languages a picture is read in is decided by its script. tesseract is
//! asked that first, a cheap question its osd data answers, and the page is
//! then read in that script's languages alone, because every language handed
//! to one reading makes it slower and no better at any of them. A script it
//! cannot name is read in every language there is. Only languages whose data is
//! installed are asked for, since tesseract refuses a whole reading over one it
//! has not got.

use std::ops::Range;

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::markup::{self, Token};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub run: Range<u32>,
    pub area: Area,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paragraph {
    pub text: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spacing {
    Spaced,
    Unspaced,
}

fn spacing(letter: Option<char>) -> Result<Spacing, Never> {
    Ok(match letter.map(u32::from) {
        Some(0x0E00..=0x0E7F | 0x3000..=0x30FF | 0x3400..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF) => Spacing::Unspaced,
        Some(_) | None => Spacing::Spaced,
    })
}

impl Paragraph {
    fn push(&mut self, said: &str, area: Area) -> Result<(), Never> {
        let Ok(before) = spacing(self.text.chars().last());
        let Ok(after) = spacing(said.chars().next());

        match (self.text.is_empty(), before, after) {
            (true, _, _) | (false, Spacing::Unspaced, Spacing::Unspaced) => {},
            (false, Spacing::Spaced, _) | (false, _, Spacing::Spaced) => self.text.push(' '),
        }

        let Ok(start) = fitted::<_, u32>(self.text.len());

        self.text.push_str(said);

        let Ok(end) = fitted::<_, u32>(self.text.len());

        self.words.push(Word { run: start..end, area });

        Ok(())
    }
}

fn number(attributes: &[(String, String)], name: &str) -> Result<Option<f64>, Never> {
    let Ok(said) = markup::attribute(attributes, name);

    Ok(match said.map(str::parse::<f64>) {
        Some(Ok(read)) => Some(read),
        Some(Err(_not_a_number)) => None,
        None => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Page {
    width: f64,
    height: f64,
}

#[derive(Default)]
struct Layout {
    page: Option<Page>,
    paragraphs: Vec<Paragraph>,
    word: Option<Area>,
    said: String,
}

impl Layout {
    fn start(&mut self, name: &str, attributes: &[(String, String)]) -> Result<(), Never> {
        let Ok(corners) = corners(attributes);

        match (name, self.page, corners) {
            ("page", _, _) => {
                let Ok(width) = number(attributes, "width");
                let Ok(height) = number(attributes, "height");

                self.page = width.zip(height).map(|(width, height)| Page { width, height });
            },
            ("block", _, _) => self.paragraphs.push(Paragraph::default()),
            ("word", Some(page), Some(Corners { low, high })) => {
                let Ok(area) = share(Area { left: low.x, top: low.y, width: high.x - low.x, height: high.y - low.y }, page);

                self.said.clear();
                self.word = Some(area);
            },
            (_, _, _) => {},
        }

        Ok(())
    }

    fn end(&mut self, name: &str) -> Result<(), Never> {
        match (name, self.word.take(), self.paragraphs.last_mut()) {
            ("word", Some(area), Some(paragraph)) => paragraph.push(self.said.trim(), area),
            (_, _, _) => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Corners {
    low: Point<f64>,
    high: Point<f64>,
}

fn corners(attributes: &[(String, String)]) -> Result<Option<Corners>, Never> {
    let Ok(left) = number(attributes, "xmin");
    let Ok(top) = number(attributes, "ymin");
    let Ok(right) = number(attributes, "xmax");
    let Ok(bottom) = number(attributes, "ymax");

    Ok(match (left, top, right, bottom) {
        (Some(left), Some(top), Some(right), Some(bottom)) => Some(Corners { low: Point { x: left, y: top }, high: Point { x: right, y: bottom } }),
        (None, _, _, _) | (_, None, _, _) | (_, _, None, _) | (_, _, _, None) => None,
    })
}

fn share(area: Area, page: Page) -> Result<Area, Never> {
    Ok(match page.width > 0.0 && page.height > 0.0 {
        true => Area { left: area.left / page.width, top: area.top / page.height, width: area.width / page.width, height: area.height / page.height },
        false => area,
    })
}

pub fn from_layout(markup: &str) -> Result<Vec<Paragraph>, Never> {
    let Ok(tokens) = markup::tokenize(markup);
    let mut layout = Layout::default();

    for token in &tokens {
        let Ok(()) = match token {
            Token::StartTag { name, attributes } => layout.start(name, attributes),
            Token::EndTag { name } => layout.end(name),
            Token::Text(said) => {
                layout.said.push_str(said);

                Ok(())
            },
        };
    }

    Ok(layout.paragraphs.into_iter().filter(|paragraph| !paragraph.words.is_empty()).collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row {
    level: u32,
    block: u32,
    paragraph: u32,
}

fn row(fields: &[&str]) -> Result<Option<(Row, Area, String)>, Never> {
    let numbers: Vec<Option<f64>> = fields.iter().take(10).map(|field| match field.parse::<f64>() {
        Ok(read) => Some(read),
        Err(_not_a_number) => None,
    }).collect();

    let whole = |at: u32| -> Option<u32> {
        let Ok(at) = console_core_number_conversion::index(at);

        fields.get(at).and_then(|field| match field.parse::<u32>() {
            Ok(read) => Some(read),
            Err(_not_a_number) => None,
        })
    };

    let area = match (numbers.get(6), numbers.get(7), numbers.get(8), numbers.get(9)) {
        (Some(Some(left)), Some(Some(top)), Some(Some(width)), Some(Some(height))) => Area { left: *left, top: *top, width: *width, height: *height },
        (_, _, _, _) => return Ok(None),
    };

    let said = match fields.get(11) {
        Some(said) => said.trim().to_string(),
        None => String::new(),
    };

    Ok(match (whole(0), whole(2), whole(3)) {
        (Some(level), Some(block), Some(paragraph)) => Some((Row { level, block, paragraph }, area, said)),
        (None, _, _) | (_, None, _) | (_, _, None) => None,
    })
}

const PAGE_ROW: u32 = 1;

const WORD_ROW: u32 = 5;

pub fn from_recognition(table: &str) -> Result<Vec<Paragraph>, Never> {
    let mut page: Option<Page> = None;
    let mut paragraphs: Vec<((u32, u32), Paragraph)> = Vec::new();

    for line in table.lines().skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        let Ok(read) = row(&fields);

        match (read, page) {
            (Some((Row { level: PAGE_ROW, .. }, area, _)), _) => page = Some(Page { width: area.width, height: area.height }),
            (Some((Row { level: WORD_ROW, block, paragraph }, area, said)), Some(page)) => match said.is_empty() {
                true => {},
                false => {
                    let key = (block, paragraph);
                    let Ok(area) = share(area, page);

                    match paragraphs.last_mut() {
                        Some((last, paragraph)) => match *last == key {
                            true => {
                                let Ok(()) = paragraph.push(&said, area);
                            },
                            false => {
                                let mut fresh = Paragraph::default();
                                let Ok(()) = fresh.push(&said, area);

                                paragraphs.push((key, fresh));
                            },
                        },
                        None => {
                            let mut fresh = Paragraph::default();
                            let Ok(()) = fresh.push(&said, area);

                            paragraphs.push((key, fresh));
                        },
                    }
                },
            },
            (Some(_), _) | (None, _) => {},
        }
    }

    Ok(paragraphs.into_iter().map(|(_, paragraph)| paragraph).collect())
}

fn distance(area: Area, at: Point<f64>) -> Result<f64, Never> {
    let across = (area.left - at.x).max(at.x - (area.left + area.width)).max(0.0);
    let down = (area.top - at.y).max(at.y - (area.top + area.height)).max(0.0);

    Ok(across.hypot(down * 2.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub paragraph: u32,
    pub start: u32,
}

pub fn word_at(paragraphs: &[Paragraph], at: Point<f64>) -> Result<Option<Found>, Never> {
    let nearest = (0_u32..)
        .zip(paragraphs)
        .flat_map(|(paragraph, each)| each.words.iter().map(move |word| (paragraph, word)))
        .map(|(paragraph, word)| {
            let Ok(far) = distance(word.area, at);

            (far, Found { paragraph, start: word.run.start })
        })
        .min_by(|(one, _), (other, _)| one.total_cmp(other));

    Ok(nearest.map(|(_, found)| found))
}

pub fn widen(paragraph: &Paragraph, run: Range<u32>) -> Result<Range<u32>, Never> {
    let (low, high) = match run.start <= run.end {
        true => (run.start, run.end),
        false => (run.end, run.start),
    };

    let mut touched = paragraph.words.iter().filter(|word| low < word.run.end && word.run.start <= high);
    let first = touched.next();
    let last = touched.next_back().or(first);

    Ok(match (first, last) {
        (Some(first), Some(last)) => first.run.start..last.run.end,
        (None, _) | (_, None) => low..high,
    })
}

pub fn word_under(paragraphs: &[Paragraph], at: Point<f64>) -> Result<Option<Found>, Never> {
    let under = (0_u32..)
        .zip(paragraphs)
        .flat_map(|(paragraph, each)| each.words.iter().map(move |word| (paragraph, word)))
        .find(|(_, word)| {
            let Ok(far) = distance(word.area, at);

            far <= word.area.height / 2.0
        });

    Ok(under.map(|(paragraph, word)| Found { paragraph, start: word.run.start }))
}

pub const SCRIPTS: [(&str, &[&str]); 8] = [
    ("Latin", &["eng", "nld", "fra"]),
    ("Japanese", &["jpn", "jpn_vert"]),
    ("Han", &["jpn", "jpn_vert"]),
    ("Hiragana", &["jpn", "jpn_vert"]),
    ("Katakana", &["jpn", "jpn_vert"]),
    ("Hangul", &["kor", "kor_vert"]),
    ("Korean", &["kor", "kor_vert"]),
    ("Thai", &["tha"]),
];

pub fn installed(listed: &str) -> Result<Vec<String>, Never> {
    Ok(listed.lines().skip(1).map(str::trim).filter(|language| !language.is_empty() && *language != "osd").map(str::to_string).collect())
}

pub fn script(answered: &str) -> Result<Option<String>, Never> {
    Ok(answered.lines().find_map(|line| line.strip_prefix("Script:")).map(|named| named.trim().to_string()))
}

pub fn languages(script: Option<&str>, installed: &[String]) -> Result<String, Never> {
    let have: std::collections::BTreeSet<&str> = installed.iter().map(String::as_str).collect();
    let wanted = SCRIPTS.iter().find(|(named, _)| Some(*named) == script).map(|(_, languages)| *languages);

    let chosen: Vec<&str> = match wanted {
        Some(languages) => languages.iter().copied().filter(|language| have.contains(language)).collect(),
        None => Vec::new(),
    };

    let chosen = match chosen.is_empty() {
        true => {
            let (kept, _) = SCRIPTS
                .iter()
                .flat_map(|(_, languages)| languages.iter().copied())
                .filter(|language| have.contains(language))
                .fold((Vec::new(), std::collections::BTreeSet::new()), |(mut kept, mut seen), language| {
                    match seen.insert(language) {
                        true => kept.push(language),
                        false => {},
                    }

                    (kept, seen)
                });

            kept
        },
        false => chosen,
    };

    Ok(chosen.join("+"))
}

pub fn areas(paragraph: &Paragraph, run: &Range<u32>) -> Result<Vec<Area>, Never> {
    Ok(paragraph.words.iter().filter(|word| word.run.start < run.end && run.start < word.run.end).map(|word| word.area).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: &str = include_str!("../tests/a-page.layout.html");

    const RECOGNIZED: &str = include_str!("../tests/a-page.tsv");

    fn texts(paragraphs: &[Paragraph]) -> Result<Vec<&str>, Never> {
        Ok(paragraphs.iter().map(|paragraph| paragraph.text.as_str()).collect())
    }

    #[test]
    fn a_pdf_text_layer_is_read_as_its_paragraphs() {
        let Ok(read) = from_layout(LAYOUT);

        assert_eq!(texts(&read), Ok(vec!["Call me Ishmael.", "Some years ago, never mind how long precisely, having little."]));
    }

    #[test]
    fn what_was_recognized_off_a_picture_is_read_the_same_way() {
        let Ok(read) = from_recognition(RECOGNIZED);

        assert_eq!(texts(&read), Ok(vec!["Call me Ishmael.", "Some years ago, never mind how long precisely, having little."]));
    }

    #[test]
    fn a_word_knows_its_letters_and_its_share_of_the_page() -> Result<(), &'static str> {
        let Ok(read) = from_layout(LAYOUT);
        let first = read.first().and_then(|paragraph| paragraph.words.get(1)).ok_or("a second word")?;

        assert_eq!(first.run, 5..7, "me");
        assert!((first.area.left - 91.2825 / 450.0).abs() < 1e-6);
        assert!((first.area.top - 65.945547 / 600.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn a_touch_finds_the_nearest_word() -> Result<(), &'static str> {
        let Ok(read) = from_recognition(RECOGNIZED);
        let Ok(found) = word_at(&read, Point { x: 0.43, y: 0.26 });
        let found = found.ok_or("a word near the middle of the second line")?;
        let paragraph = read.get(1).ok_or("a second paragraph")?;
        let Ok(start) = console_core_number_conversion::index(found.start);

        assert_eq!(found.paragraph, 1);
        assert!(paragraph.text.get(start..).is_some_and(|rest| rest.starts_with("ago,")), "{:?}", paragraph.text.get(start..));

        Ok(())
    }

    #[test]
    fn japanese_and_thai_words_are_joined_with_nothing_between_them() {
        let mut japanese = Paragraph::default();
        let area = Area { left: 0.0, top: 0.0, width: 0.1, height: 0.1 };

        for said in ["\u{79c1}\u{306f}", "\u{732b}", "Tokyo", "\u{0e41}\u{0e21}\u{0e27}", "\u{0e14}\u{0e33}"] {
            let Ok(()) = japanese.push(said, area);
        }

        assert_eq!(japanese.text, "\u{79c1}\u{306f}\u{732b} Tokyo \u{0e41}\u{0e21}\u{0e27}\u{0e14}\u{0e33}");

        let mut korean = Paragraph::default();

        for said in ["\u{c548}\u{b155}", "\u{d558}\u{c138}\u{c694}"] {
            let Ok(()) = korean.push(said, area);
        }

        assert_eq!(korean.text, "\u{c548}\u{b155} \u{d558}\u{c138}\u{c694}", "Korean keeps its spaces");
    }

    #[test]
    fn a_highlight_on_a_picture_is_widened_to_the_words_that_were_found() -> Result<(), &'static str> {
        let mut paragraph = Paragraph::default();
        let area = Area { left: 0.0, top: 0.0, width: 0.1, height: 0.1 };

        for said in ["\u{79c1}\u{306f}", "\u{732b}", "\u{304c}\u{597d}\u{304d}"] {
            let Ok(()) = paragraph.push(said, area);
        }

        let middle = paragraph.words.get(1).map(|word| word.run.clone()).ok_or("a second word")?;

        assert_eq!(widen(&paragraph, middle.start..middle.start), Ok(middle.clone()), "a tap takes the word it lands on and no more");
        assert_eq!(widen(&paragraph, 1..middle.start), Ok(0..middle.end));

        Ok(())
    }

    #[test]
    fn a_page_is_read_in_the_languages_of_its_script_that_are_installed() {
        let every: Vec<String> = ["eng", "nld", "fra", "jpn", "jpn_vert", "kor", "kor_vert", "tha"].iter().map(|language| language.to_string()).collect();
        let only_english = vec!["eng".to_string()];

        assert_eq!(script("Page number: 0\nScript: Japanese\nScript confidence: 3.1\n"), Ok(Some("Japanese".to_string())));
        assert_eq!(languages(Some("Japanese"), &every), Ok("jpn+jpn_vert".to_string()));
        assert_eq!(languages(Some("Hangul"), &every), Ok("kor+kor_vert".to_string()));
        assert_eq!(languages(Some("Latin"), &only_english), Ok("eng".to_string()), "a language that is not installed is not asked for");
        assert_eq!(languages(None, &every), Ok("eng+nld+fra+jpn+jpn_vert+kor+kor_vert+tha".to_string()), "no script is every language");
        assert_eq!(languages(Some("Thai"), &only_english), Ok("eng".to_string()), "a script with none of its languages installed is read with what there is");
        assert_eq!(installed("List of available languages in \"/usr/share/tessdata/\" (3):\nafr\neng\nosd\n"), Ok(vec!["afr".to_string(), "eng".to_string()]));
    }

    #[test]
    fn a_highlight_covers_the_boxes_of_the_words_it_runs_over() -> Result<(), &'static str> {
        let Ok(read) = from_layout(LAYOUT);
        let paragraph = read.get(1).ok_or("a second paragraph")?;
        let Ok(covered) = areas(paragraph, &(5..14));

        assert_eq!(covered.len(), 2, "years and ago");

        Ok(())
    }
}
