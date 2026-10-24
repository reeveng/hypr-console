//! A portable document, read in this process.
//!
//! poppler did this as three programs, each started for every question and
//! each answering in a format of its own: `pdfinfo` for how many pages, read
//! off a line of prose; `pdftoppm` to draw one, as a portable pixmap down a
//! pipe; and `pdftotext -bbox-layout` for the words and their boxes, as markup
//! somebody then had to parse back. The renderer in `forks/` draws a page into
//! pixels here and the text reader beside it hands over every glyph with the
//! box it is drawn in, so a page is opened once and asked all three.
//!
//! A word is the glyphs between two breaks the text reader saw -- a space it
//! measured, or the end of a line -- and its box is the union of theirs, kept
//! as a share of the page like every other box in [`crate::live_text`].
//! poppler grouped lines into blocks; here a line further below the one before
//! it than that line is tall starts a new paragraph, which is the same block a
//! reader sees and not always the one poppler found.
//!
//! The page is drawn on white. A document draws its own paper as often as it
//! leaves it out, and a page with no paper is a transparent one, which the
//! book draws over whatever colour the room is.

use std::path::Path;
use std::sync::Arc;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_portable_document_rasterizer::console_core_portable_document_interpreter::InterpreterSettings;
use console_core_portable_document_rasterizer::console_core_portable_document_syntax::{LoadPdfError, Pdf};
use console_core_portable_document_rasterizer::console_core_portable_document_syntax::page::Page;
use console_core_portable_document_rasterizer::vello_cpu::color::{AlphaColor, Srgb};
use console_core_portable_document_rasterizer::{RenderCache, RenderSettings, render};
use console_core_portable_document_text::{PageText, extract_page};
use console_core_shapes::Pixels;

use crate::live_text::{Area, Paragraph};

const POINTS_AN_INCH: f32 = 72.0;

const BYTES_A_PIXEL: u32 = 4;

#[derive(Debug)]
pub enum DocumentError {
    Unread(std::io::Error),
    Encrypted,
    Unparsed,
    Unencoded(console_core_png_files::PngError),
}

impl std::fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocumentError::Unread(fault) => write!(formatter, "{fault}"),
            DocumentError::Encrypted => write!(formatter, "it is locked, and nothing here knows the password"),
            DocumentError::Unparsed => write!(formatter, "it is not a document this desktop can read"),
            DocumentError::Unencoded(fault) => write!(formatter, "making the page a PNG: {fault}"),
        }
    }
}

impl std::error::Error for DocumentError {}

impl From<LoadPdfError> for DocumentError {
    fn from(fault: LoadPdfError) -> DocumentError {
        match fault {
            LoadPdfError::Decryption(_unopened) => DocumentError::Encrypted,
            LoadPdfError::Invalid => DocumentError::Unparsed,
        }
    }
}

pub struct Document {
    pdf: Pdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Tall(u32),
    DotsPerInch(u32),
}

pub fn opened(at: &Path) -> Result<Document, DocumentError> {
    let bytes = std::fs::read(at).map_err(DocumentError::Unread)?;
    let pdf = Pdf::new(bytes)?;

    Ok(Document { pdf })
}

impl Document {
    pub fn page_count(&self) -> Result<u32, Never> {
        fitted(self.pdf.pages().len())
    }

    fn page(&self, number: u32) -> Result<Option<&Page<'_>>, Never> {
        let Ok(at) = index(number);

        Ok(self.pdf.pages().get(at))
    }

    pub fn drawn(&self, number: u32, scale: Scale) -> Result<Option<Pixels>, Never> {
        let Ok(page) = self.page(number);

        let page = match page {
            Some(page) => page,
            None => return Ok(None),
        };

        let (_wide, tall) = page.render_dimensions();
        let Ok(factor) = factor(scale, tall);
        let settings = RenderSettings { x_scale: factor, y_scale: factor, bg_color: AlphaColor::<Srgb>::WHITE, ..RenderSettings::default() };
        let pixmap = render(page, &RenderCache::new(), &InterpreterSettings::default(), &settings);
        let width = u32::from(pixmap.width());
        let height = u32::from(pixmap.height());
        let stride = width.saturating_mul(BYTES_A_PIXEL);

        Ok(Some(Pixels { width, height, stride, bytes: Arc::new(pixmap.data_as_u8_slice().to_vec()) }))
    }

    pub fn png(&self, number: u32, scale: Scale) -> Result<Option<Vec<u8>>, DocumentError> {
        let Ok(drawn) = self.drawn(number, scale);

        let pixels = match drawn {
            Some(pixels) => pixels,
            None => return Ok(None),
        };

        let size = console_core_geometry::Size { width: pixels.width, height: pixels.height };
        let picture = console_core_png_files::Picture { size, rgba: pixels.bytes.to_vec() };
        let png = console_core_png_files::encoded(&picture).map_err(DocumentError::Unencoded)?;

        Ok(Some(png))
    }

    pub fn text(&self, number: u32) -> Result<Vec<Paragraph>, Never> {
        let Ok(page) = self.page(number);

        let page = match page {
            Some(page) => page,
            None => return Ok(Vec::new()),
        };

        let (wide, tall) = page.render_dimensions();
        let Ok(at) = index(number);

        match extract_page(&self.pdf, at, &InterpreterSettings::default()) {
            Some(text) => paragraphs(&text, Sheet { wide, tall }),
            None => Ok(Vec::new()),
        }
    }
}

fn factor(scale: Scale, tall: f32) -> Result<f32, Never> {
    let wanted = |count: u32| match fitted::<u32, u16>(count) {
        Ok(count) => f32::from(count),
    };

    Ok(match (scale, tall > 0.0) {
        (Scale::Tall(pixels), true) => wanted(pixels) / tall,
        (Scale::Tall(_pixels), false) => 1.0,
        (Scale::DotsPerInch(dots), _) => wanted(dots) / POINTS_AN_INCH,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Sheet {
    wide: f32,
    tall: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Corners {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Corners {
    fn of(rect: [f32; 4]) -> Result<Corners, Never> {
        let [left, top, right, bottom] = rect;

        Ok(Corners { left, top, right, bottom })
    }

    fn with(self, other: Corners) -> Result<Corners, Never> {
        Ok(Corners { left: self.left.min(other.left), top: self.top.min(other.top), right: self.right.max(other.right), bottom: self.bottom.max(other.bottom) })
    }

    fn area(self, sheet: Sheet) -> Result<Area, Never> {
        let share = |length: f32, of: f32| match of > 0.0 {
            true => f64::from(length / of),
            false => f64::from(length),
        };

        Ok(Area {
            left: share(self.left, sheet.wide),
            top: share(self.top, sheet.tall),
            width: share(self.right - self.left, sheet.wide),
            height: share(self.bottom - self.top, sheet.tall),
        })
    }
}

#[derive(Default)]
struct Reading {
    paragraphs: Vec<Paragraph>,
    word: Option<(String, Corners)>,
    line: Option<(u32, Corners)>,
}

impl Reading {
    fn finish_word(&mut self, sheet: Sheet) -> Result<(), Never> {
        let finished = self.word.take();

        match (finished, self.paragraphs.last_mut()) {
            (Some((said, corners)), Some(paragraph)) => {
                let Ok(area) = corners.area(sheet);

                paragraph.push(&said, area)
            },
            (None, _) | (_, None) => Ok(()),
        }
    }

    fn start_line(&mut self, line: u32, corners: Corners) -> Result<(), Never> {
        let apart = match self.line {
            Some((_before, above)) => {
                let gap = corners.top - above.bottom;
                let tall = above.bottom - above.top;

                gap > tall
            },
            None => true,
        };

        match apart {
            true => self.paragraphs.push(Paragraph::default()),
            false => {},
        }

        self.line = Some((line, corners));

        Ok(())
    }
}

fn paragraphs(text: &PageText, sheet: Sheet) -> Result<Vec<Paragraph>, Never> {
    let mut reading = Reading::default();
    let placed = text.glyphs.iter().zip(text.line_of.iter()).zip(text.space_before.iter());

    for ((glyph, line), space_before) in placed {
        let Ok(corners) = Corners::of(glyph.rect);
        let new_line = reading.line.map(|(at, _corners)| at) != Some(*line);
        let blank = glyph.text.trim().is_empty();

        let Ok(()) = match new_line || *space_before || blank {
            true => reading.finish_word(sheet),
            false => Ok(()),
        };

        let Ok(()) = match new_line {
            true => reading.start_line(*line, corners),
            false => Ok(()),
        };

        let Ok(along) = reading.line.map_or(Ok(corners), |(_at, along)| along.with(corners));

        reading.line = reading.line.map(|(at, _corners)| (at, along));

        match (blank, reading.word.take()) {
            (true, held) => reading.word = held,
            (false, Some((mut said, held))) => {
                let Ok(grown) = held.with(corners);

                said.push_str(&glyph.text);
                reading.word = Some((said, grown));
            },
            (false, None) => reading.word = Some((glyph.text.clone(), corners)),
        }
    }

    let Ok(()) = reading.finish_word(sheet);

    Ok(reading.paragraphs.into_iter().filter(|paragraph| !paragraph.words.is_empty()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_page_saying(said: &str) -> Result<Vec<u8>, Never> {
        let content = format!("BT /F1 24 Tf 20 50 Td ({said}) Tj ET");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];
        let mut document = b"%PDF-1.7\n".to_vec();
        let mut offsets: Vec<u64> = Vec::new();

        for (number, object) in (1_u32..).zip(objects.iter()) {
            let Ok(offset) = fitted::<_, u64>(document.len());

            offsets.push(offset);
            document.extend(format!("{number} 0 obj\n{object}\nendobj\n").bytes());
        }

        let Ok(table) = fitted::<_, u64>(document.len());

        document.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len().saturating_add(1)).bytes());

        for offset in offsets {
            document.extend(format!("{offset:010} 00000 n \n").bytes());
        }

        document.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{table}\n%%EOF\n", objects.len().saturating_add(1)).bytes());

        Ok(document)
    }

    fn opened_saying(said: &str, folder: &Path) -> Result<Document, Box<dyn std::error::Error>> {
        let at = folder.join("page.pdf");
        let Ok(bytes) = a_page_saying(said);

        console_core_atomic_writes::whole(&at, &bytes)?;

        let document = opened(&at)?;

        Ok(document)
    }

    #[test]
    fn a_page_is_counted_and_drawn_at_the_height_asked_with_its_words_dark_on_white() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("books-page-drawn")?;
        let document = opened_saying("Hello world", &folder)?;
        let Ok(count) = document.page_count();
        let Ok(drawn) = document.drawn(0, Scale::Tall(100));
        let pixels = drawn.ok_or("the page was not drawn")?;
        let Ok(dark) = fitted::<_, u32>(pixels.bytes.as_chunks::<4>().0.iter().filter(|[red, ..]| *red < 128).count());
        let Ok(white) = fitted::<_, u32>(pixels.bytes.as_chunks::<4>().0.iter().filter(|pixel| pixel.iter().all(|channel| *channel == 255)).count());

        assert_eq!(count, 1);
        assert_eq!((pixels.width, pixels.height), (200, 100));
        assert!(dark > 0, "no ink on the page");
        assert!(white > dark, "the paper is not white");
        assert_eq!(document.drawn(1, Scale::Tall(100)), Ok(None));

        Ok(())
    }

    #[test]
    fn a_text_layer_is_read_as_its_paragraphs_and_each_word_as_its_share_of_the_page() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("books-a-page")?;
        let at = folder.join("a-page.pdf");

        console_core_atomic_writes::whole(&at, include_bytes!("../tests/a-page.pdf"))?;

        let document = opened(&at)?;
        let Ok(paragraphs) = document.text(0);
        let texts: Vec<&str> = paragraphs.iter().map(|paragraph| paragraph.text.as_str()).collect();
        let second = paragraphs.first().and_then(|paragraph| paragraph.words.get(1)).ok_or("a second word")?;

        assert_eq!(texts, vec!["Call me Ishmael.", "Some years ago, never mind how long precisely, having little."]);
        assert_eq!(second.run, 5..7, "me");
        assert!((second.area.left - 91.2825 / 450.0).abs() < 1e-4, "me starts at {}", second.area.left);

        Ok(())
    }

    #[test]
    fn the_words_on_a_page_are_read_with_the_share_of_the_page_each_covers() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("books-page-read")?;
        let document = opened_saying("Hello world", &folder)?;
        let Ok(paragraphs) = document.text(0);
        let paragraph = paragraphs.first().ok_or("no paragraph")?;
        let first = paragraph.words.first().ok_or("no word")?;

        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraph.text, "Hello world");
        assert_eq!(paragraph.words.len(), 2);
        assert!((first.area.left - 0.1).abs() < 0.02, "the first word starts at {}", first.area.left);
        assert!(first.area.top > 0.2 && first.area.top < 0.5, "the first word's top is at {}", first.area.top);

        Ok(())
    }

    #[test]
    fn a_file_that_is_not_a_document_is_said_to_be_one() -> Result<(), Box<dyn std::error::Error>> {
        let folder = console_core_temporary_directories::fresh("books-not-a-document")?;
        let at = folder.join("page.pdf");

        console_core_atomic_writes::whole(&at, b"<html>not a document</html>")?;

        assert!(matches!(opened(&at), Err(DocumentError::Unparsed)));
        assert!(matches!(opened(&folder.join("none.pdf")), Err(DocumentError::Unread(_))));

        Ok(())
    }
}
