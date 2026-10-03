//! What is written on a book, and the one file it is kept in.
//!
//! A note is a highlight, handwriting or something typed. A highlight is a run
//! of letters in one paragraph, so it is kept as the paragraph and the two
//! letters it runs between: laid out again at another size those letters are
//! on another line or another page, and the highlight goes with them. A typed
//! note is kept the same way, against the run of letters it was written about,
//! with its text and whether its popup was left open or closed -- the way a
//! PDF keeps a text annotation, because a note somebody closed is one they did
//! not want over the page the next time the book opens. Handwriting is strokes,
//! and strokes are drawn where a hand was, so they are kept against the word
//! they were started beside: relative to the top left of its first letter, and
//! measured against the size the text was when they were written. Laid out in
//! another face or at another size the word is somewhere else, and the strokes
//! are drawn from wherever it is now and grown or shrunk by as much as the text
//! was, so a note written beside a word stays beside that word. Whether the
//! note is about the page or the paragraph decides what is quoted with it, not
//! where it is drawn. On a picture the page is the thing that does not move,
//! so there the strokes are kept against its top left and its height.
//!
//! Every note of a book is in one Markdown file, and the file is the only place
//! it is kept, because the point of it is to be read: a heading for where the
//! note is, the paragraph or the page it is about as a quote with what was
//! highlighted marked, handwriting as the picture it is, and what was typed as
//! the paragraph under the quote. Under each is a line a Markdown reader does
//! not show, saying where the note belongs, which is the part this reader
//! parses back. The quote is written once, when the note is made, so the file
//! reads the same with the book closed. A typed line that would read as one of
//! those -- a heading, a quote, the hidden line -- is escaped the way Markdown
//! escapes it, so nothing typed can be mistaken for where a note belongs.
//!
//! A PDF page or a comic page is a picture rather than words, so the block and
//! the letter of handwriting there are both nought and the strokes are relative
//! to the page; a highlight or a typed note there is kept against the words
//! `live_text` found on it.

use std::path::{Path, PathBuf};

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_words::Words;

use crate::library;

const FOLDER: &str = "Notes";

const ANCHOR: &str = "<!-- books ";

const ANCHOR_END: &str = " -->";

const PATH: &str = "<path d=\"";

const HIGHLIGHT: &str = "highlight";

const HANDWRITING: &str = "handwriting";

const TEXT: &str = "text";

const PICTURE: &str = "<svg";

const ESCAPED: [char; 4] = ['#', '>', '<', '\\'];

const PEN: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Words)]
pub enum Scope {
    #[words(word = "page", says = "Page")]
    Page,
    #[words(word = "paragraph", says = "Paragraph")]
    Paragraph,
}

const SCOPES: [Scope; 2] = [Scope::Page, Scope::Paragraph];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Words)]
pub enum Popup {
    #[words(word = "open")]
    Open,
    #[words(word = "closed")]
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spot {
    pub section: u32,
    pub block: u32,
    pub start: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stroke(pub Vec<Point<i32>>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    Highlight { end: u32 },
    Handwriting { scope: Scope, size: u32, strokes: Vec<Stroke> },
    Text { end: u32, text: String, popup: Popup },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scale {
    pub written: u32,
    pub now: u32,
}

pub fn scaled(at: Point<i32>, scale: Scale) -> Result<Point<i32>, Never> {
    let times = |along: i32| -> i32 {
        let grown = i64::from(along).saturating_mul(i64::from(scale.now));

        match grown.checked_div(i64::from(scale.written)).map(i32::try_from) {
            Some(Ok(along)) => along,
            Some(Err(_too_far)) => along,
            None => along,
        }
    };

    Ok(Point { x: times(at.x), y: times(at.y) })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub spot: Spot,
    pub mark: Mark,
    pub heading: String,
    pub context: String,
}

pub fn path(home: &Path, title: &str) -> Result<PathBuf, Never> {
    let Ok(books) = library::books_folder(home);
    let named: String = title.chars().map(|letter| match letter {
        '/' => '-',
        letter => letter,
    }).collect();

    Ok(books.join(FOLDER).join(format!("{named}.md")))
}

pub fn widen(text: &str, run: std::ops::Range<u32>) -> Result<std::ops::Range<u32>, Never> {
    let (start, end) = match run.start <= run.end {
        true => (run.start, run.end),
        false => (run.end, run.start),
    };

    let Ok(from) = index(start);
    let Ok(to) = index(end);

    let before = match text.get(..from) {
        Some(before) => before,
        None => "",
    };
    let after = match text.get(to..) {
        Some(after) => after,
        None => "",
    };

    let Ok(start) = match before.rfind(char::is_whitespace).map(fitted::<_, u32>) {
        Some(Ok(space)) => Ok::<u32, Never>(space.saturating_add(1)),
        None => Ok(0),
    };
    let Ok(end) = match after.find(char::is_whitespace).map(fitted::<_, u32>) {
        Some(Ok(space)) => Ok(end.saturating_add(space)),
        None => fitted::<_, u32>(text.len()),
    };

    Ok(start..end)
}

pub fn marked(text: &str, run: std::ops::Range<u32>) -> Result<String, Never> {
    let Ok(start) = index(run.start);
    let Ok(end) = index(run.end);
    let parts = (text.get(..start), text.get(start..end), text.get(end..));

    Ok(match parts {
        (Some(before), Some(highlighted), Some(after)) => format!("{before}=={highlighted}=={after}"),
        (None, _, _) | (_, None, _) | (_, _, None) => text.to_string(),
    })
}

pub fn highlight_at(notes: &[Note], spot: Spot) -> Result<Option<u32>, Never> {
    let found = notes
        .iter()
        .position(|note| match note.mark {
            Mark::Highlight { end } => {
                note.spot.section == spot.section && note.spot.block == spot.block && note.spot.start <= spot.start && spot.start < end
            },
            Mark::Handwriting { .. } | Mark::Text { .. } => false,
        })
        .map(fitted::<_, u32>);

    Ok(found.map(|Ok(at)| at))
}

pub fn text_at(notes: &[Note], spot: Spot, end: u32) -> Result<Option<u32>, Never> {
    let through = end.max(spot.start.saturating_add(1));
    let found = notes
        .iter()
        .position(|note| match note.mark {
            Mark::Text { end: last, .. } => {
                note.spot.section == spot.section && note.spot.block == spot.block && note.spot.start < through && spot.start < last
            },
            Mark::Highlight { .. } | Mark::Handwriting { .. } => false,
        })
        .map(fitted::<_, u32>);

    Ok(found.map(|Ok(at)| at))
}

fn quoted(context: &str) -> Result<String, Never> {
    Ok(context.lines().map(|line| format!("> {line}\n")).collect())
}

fn drawn(strokes: &[Stroke]) -> Result<String, Never> {
    let points: Vec<Point<i32>> = strokes.iter().flat_map(|stroke| stroke.0.iter().copied()).collect();
    let across = (points.iter().map(|at| at.x).min(), points.iter().map(|at| at.x).max());
    let down = (points.iter().map(|at| at.y).min(), points.iter().map(|at| at.y).max());

    let (low, high) = match (across, down) {
        ((Some(left), Some(right)), (Some(top), Some(bottom))) => (Point { x: left, y: top }, Point { x: right, y: bottom }),
        ((None, _) | (_, None), _) | (_, (None, _) | (_, None)) => (Point::default(), Point::default()),
    };
    let wide = high.x.saturating_sub(low.x).saturating_add(8);
    let tall = high.y.saturating_sub(low.y).saturating_add(8);
    let left = low.x.saturating_sub(4);
    let top = low.y.saturating_sub(4);

    let path: String = strokes
        .iter()
        .map(|stroke| {
            stroke
                .0
                .iter()
                .zip(std::iter::once("M").chain(std::iter::repeat("L")))
                .map(|(point, pen)| format!("{pen}{} {}", point.x, point.y))
                .collect::<String>()
        })
        .collect();

    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{wide}\" height=\"{tall}\" viewBox=\"{left} {top} {wide} {tall}\">\
         <path d=\"{path}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"{PEN}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>\n"
    ))
}

fn anchor(note: &Note) -> Result<String, Never> {
    let Spot { section, block, start } = note.spot;

    Ok(match &note.mark {
        Mark::Highlight { end } => format!("{ANCHOR}{HIGHLIGHT} {section} {block} {start} {end}{ANCHOR_END}\n"),
        Mark::Handwriting { scope, size, .. } => {
            let Ok(scope) = scope.word();

            format!("{ANCHOR}{HANDWRITING} {scope} {section} {block} {start} {size}{ANCHOR_END}\n")
        },
        Mark::Text { end, popup, .. } => {
            let Ok(popup) = popup.word();

            format!("{ANCHOR}{TEXT} {popup} {section} {block} {start} {end}{ANCHOR_END}\n")
        },
    })
}

fn escaped(text: &str) -> Result<String, Never> {
    Ok(text
        .lines()
        .map(|line| match line.starts_with(ESCAPED) {
            true => format!("\\{line}\n"),
            false => format!("{line}\n"),
        })
        .collect())
}

pub fn serialize(title: &str, notes: &[Note]) -> Result<String, Never> {
    let mut written = format!("# {title}\n");

    for note in notes {
        let Ok(quote) = quoted(&note.context);
        let Ok(anchor) = anchor(note);
        let Ok(body) = match &note.mark {
            Mark::Highlight { .. } => Ok(String::new()),
            Mark::Handwriting { strokes, .. } => drawn(strokes).map(|svg| format!("{svg}\n")),
            Mark::Text { text, .. } => escaped(text.trim_matches('\n')).map(|typed| match typed.is_empty() {
                true => typed,
                false => format!("{typed}\n"),
            }),
        };

        written.push_str(&format!("\n## {}\n\n{quote}\n{body}{anchor}", note.heading));
    }

    Ok(written)
}

fn strokes(path: &str) -> Result<Vec<Stroke>, Never> {
    let spaced = path.replace('M', " M ").replace('L', " L ");
    let words: Vec<&str> = spaced.split_whitespace().collect();
    let mut read: Vec<Stroke> = Vec::new();

    for step in words.chunks(3) {
        let point = match step {
            [word, x, y] => match (x.parse::<i32>(), y.parse::<i32>()) {
                (Ok(x), Ok(y)) => (*word, Point { x, y }),
                (Err(_not_a_number), _) | (_, Err(_not_a_number)) => return Ok(read),
            },
            _ => return Ok(read),
        };

        match (point.0, read.last_mut()) {
            ("L", Some(stroke)) => stroke.0.push(point.1),
            ("M", _) | ("L", None) => read.push(Stroke(vec![point.1])),
            (_, _) => return Ok(read),
        }
    }

    Ok(read)
}

fn numbers(words: &[&str]) -> Result<Option<Vec<u32>>, Never> {
    Ok(match words.iter().map(|word| word.parse::<u32>()).collect::<Result<Vec<u32>, _>>() {
        Ok(read) => Some(read),
        Err(_not_a_number) => None,
    })
}

struct AnchorLine<'a> {
    said: &'a str,
    picture: &'a str,
    typed: &'a str,
}

fn read_anchor(line: AnchorLine<'_>) -> Result<Option<(Spot, Mark)>, Never> {
    let words: Vec<&str> = line.said.split_whitespace().collect();
    let Ok(scope) = match words.get(1) {
        Some(word) => Ok(SCOPES.iter().copied().find(|scope| scope.word() == Ok(*word))),
        None => Ok::<Option<Scope>, Never>(None),
    };
    let Ok(popup) = match words.get(1) {
        Some(word) => Popup::from_word(word),
        None => Ok(None),
    };

    let (kind, rest) = match words.split_first() {
        Some((kind, rest)) => (*kind, rest),
        None => return Ok(None),
    };

    let Ok(read) = match (kind, scope) {
        (HIGHLIGHT, _) => numbers(rest),
        (HANDWRITING, Some(_)) | (TEXT, _) => match rest.get(1..) {
            Some(after) => numbers(after),
            None => Ok(None),
        },
        (_, _) => Ok(None),
    };

    let Ok(strokes) = strokes(line.picture);

    Ok(match (kind, scope, read.as_deref()) {
        (HIGHLIGHT, _, Some(&[section, block, start, end])) => Some((Spot { section, block, start }, Mark::Highlight { end })),
        (HANDWRITING, Some(scope), Some(&[section, block, start, size])) => {
            Some((Spot { section, block, start }, Mark::Handwriting { scope, size, strokes }))
        },
        (TEXT, _, Some(&[section, block, start, end])) => popup.map(|popup| {
            (Spot { section, block, start }, Mark::Text { end, text: line.typed.to_string(), popup })
        }),
        (_, _, _) => None,
    })
}

fn unescaped(line: &str) -> Result<&str, Never> {
    let escaped = line.strip_prefix('\\').filter(|rest| rest.starts_with(ESCAPED));

    Ok(match escaped {
        Some(rest) => rest,
        None => line,
    })
}

#[derive(Default)]
struct Reading {
    heading: String,
    context: Vec<String>,
    picture: String,
    typed: Vec<String>,
    notes: Vec<Note>,
}

pub fn parse(text: &str) -> Result<Vec<Note>, Never> {
    let mut reading = Reading::default();

    for line in text.lines() {
        let anchor = line.strip_prefix(ANCHOR).and_then(|rest| rest.strip_suffix(ANCHOR_END));
        let picture = match line.starts_with(PICTURE) {
            true => line.split_once(PATH).and_then(|(_, rest)| rest.split_once('"')).map(|(path, _)| path),
            false => None,
        };

        match (line.strip_prefix("## "), line.strip_prefix("> "), picture, anchor) {
            (Some(heading), _, _, _) => {
                reading.heading = heading.to_string();
                reading.context.clear();
                reading.picture.clear();
                reading.typed.clear();
            },
            (None, Some(quote), _, _) => reading.context.push(quote.to_string()),
            (None, None, Some(path), _) => reading.picture = path.to_string(),
            (None, None, None, Some(said)) => {
                let typed = reading.typed.join("\n");
                let Ok(read) = read_anchor(AnchorLine { said, picture: &reading.picture, typed: typed.trim_matches('\n') });

                reading.typed.clear();

                match read {
                    Some((spot, mark)) => reading.notes.push(Note {
                        spot,
                        mark,
                        heading: reading.heading.clone(),
                        context: reading.context.join("\n"),
                    }),
                    None => {},
                }
            },
            (None, None, None, None) => {
                let Ok(line) = unescaped(line);

                reading.typed.push(line.to_string());
            },
        }
    }

    Ok(reading.notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_highlight() -> Result<Note, Never> {
        Ok(Note {
            spot: Spot { section: 2, block: 7, start: 5 },
            mark: Mark::Highlight { end: 12 },
            heading: "Loomings".to_string(),
            context: "Call ==me Ishm==ael.\nSome years ago".to_string(),
        })
    }

    fn some_handwriting() -> Result<Note, Never> {
        Ok(Note {
            spot: Spot { section: 3, block: 0, start: 40 },
            mark: Mark::Handwriting {
                scope: Scope::Paragraph,
                size: 22,
                strokes: vec![Stroke(vec![Point { x: 1, y: -2 }, Point { x: 30, y: 4 }]), Stroke(vec![Point { x: 5, y: 5 }])],
            },
            heading: "The Carpet-Bag".to_string(),
            context: "I stuffed a shirt or two".to_string(),
        })
    }

    fn something_typed() -> Result<Note, Never> {
        Ok(Note {
            spot: Spot { section: 2, block: 9, start: 0 },
            mark: Mark::Text {
                end: 4,
                text: "Who calls himself that?\n\n> not a quote\n## nor a heading\n<!-- books highlight 1 1 1 1 -->\n\\ and a slash".to_string(),
                popup: Popup::Closed,
            },
            heading: "Loomings".to_string(),
            context: "==Call== me Ishmael.".to_string(),
        })
    }

    #[test]
    fn every_note_comes_back_out_of_the_file_it_was_written_into() {
        let Ok(highlight) = a_highlight();
        let Ok(handwriting) = some_handwriting();
        let Ok(typed) = something_typed();
        let notes = vec![highlight, typed, handwriting];
        let Ok(written) = serialize("Moby-Dick", &notes);

        assert_eq!(parse(&written), Ok(notes), "{written}");
    }

    #[test]
    fn the_file_reads_as_the_book_with_what_was_highlighted_marked() {
        let Ok(highlight) = a_highlight();
        let Ok(written) = serialize("Moby-Dick", &[highlight]);

        assert!(written.starts_with("# Moby-Dick\n"));
        assert!(written.contains("## Loomings\n\n> Call ==me Ishm==ael.\n> Some years ago\n"), "{written}");
    }

    #[test]
    fn what_was_typed_is_read_under_its_quote_and_cannot_pass_for_a_note() {
        let Ok(typed) = something_typed();
        let Ok(written) = serialize("Moby-Dick", &[typed]);

        assert!(written.contains("> ==Call== me Ishmael.\n\nWho calls himself that?\n\n\\> not a quote\n"), "{written}");
        assert!(written.contains("\\<!-- books highlight"), "{written}");
        assert_eq!(parse(&written).map(|read| read.len()), Ok(1), "the typed anchor was taken for a second note: {written}");
    }

    #[test]
    fn a_typed_note_is_found_by_a_tap_on_its_words_or_a_run_over_them() {
        let Ok(typed) = something_typed();
        let Ok(highlight) = a_highlight();
        let notes = vec![highlight, typed];

        assert_eq!(text_at(&notes, Spot { section: 2, block: 9, start: 3 }, 3), Ok(Some(1)), "a tap on its last letter");
        assert_eq!(text_at(&notes, Spot { section: 2, block: 9, start: 4 }, 4), Ok(None), "a tap past it");
        assert_eq!(text_at(&notes, Spot { section: 2, block: 9, start: 3 }, 20), Ok(Some(1)), "a run that starts inside it");
        assert_eq!(highlight_at(&notes, Spot { section: 2, block: 9, start: 1 }), Ok(None), "a typed note is not a highlight");
    }

    #[test]
    fn handwriting_is_a_picture_a_markdown_reader_can_show() {
        let Ok(handwriting) = some_handwriting();
        let Ok(written) = serialize("Moby-Dick", &[handwriting]);

        assert!(written.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""), "{written}");
        assert!(written.contains("<path d=\"M1 -2L30 4M5 5\""), "{written}");
    }

    #[test]
    fn a_highlight_is_widened_to_whole_words() {
        let text = "Call me Ishmael. Some years ago";

        assert_eq!(widen(text, 6..10), Ok(5..16));
        assert_eq!(widen(text, std::ops::Range { start: 10, end: 6 }), Ok(5..16), "dragged backwards is the same run");
        assert_eq!(widen(text, 0..1), Ok(0..4));
        assert_eq!(widen(text, 23..25), Ok(22..27));
        assert_eq!(widen(text, 29..30), Ok(28..31));
    }

    #[test]
    fn a_highlight_is_found_by_any_letter_inside_it() {
        let Ok(highlight) = a_highlight();
        let Ok(handwriting) = some_handwriting();
        let notes = vec![handwriting, highlight];

        assert_eq!(highlight_at(&notes, Spot { section: 2, block: 7, start: 8 }), Ok(Some(1)));
        assert_eq!(highlight_at(&notes, Spot { section: 2, block: 7, start: 12 }), Ok(None), "the end is past the last letter");
        assert_eq!(highlight_at(&notes, Spot { section: 2, block: 8, start: 8 }), Ok(None));
    }

    #[test]
    fn handwriting_grows_with_the_text_it_was_written_beside() {
        assert_eq!(scaled(Point { x: 10, y: -4 }, Scale { written: 20, now: 30 }), Ok(Point { x: 15, y: -6 }));
        assert_eq!(scaled(Point { x: 10, y: -4 }, Scale { written: 20, now: 20 }), Ok(Point { x: 10, y: -4 }));
        assert_eq!(scaled(Point { x: 10, y: 4 }, Scale { written: 0, now: 20 }), Ok(Point { x: 10, y: 4 }), "a size of nothing leaves it alone");
    }

    #[test]
    fn what_a_person_wrote_in_the_file_by_hand_is_not_a_note() {
        assert_eq!(parse("# Mine\n\nsome thoughts\n<!-- books nonsense 1 2 -->\n"), Ok(Vec::new()));
    }
}
