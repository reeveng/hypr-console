//! A cover, read as characters.
//!
//! This is kew's renderer, in Rust: the same ramp, the same luminance, the same
//! rescale. `src/utils/img_utils.c` in the fork is the original, and a change
//! there is a change here.


use console_number::{Float, fitted, toward_zero_u8, whole_usize};
use std::path::Path;

use gtk4::gdk_pixbuf::{InterpType, Pixbuf};

/// The ramp a pixel's brightness is read off, densest first.
pub const RAMP: &str = "$@&B%8WM#ZO0QoahkbdpqwmLCJUYXIjft/\\|()1{}[]l?zcvunxr!<>i;:*-+~_,\"^`'.";

/// How many steps of brightness the ramp is cut into.
///
/// One short of its length, so the darkest pixel lands on the last character
/// rather than past it.
fn levels() -> usize {
    RAMP.chars().count() - 1
}

/// How tall a character cell is against its width.
///
/// A monospace cell is taller than it is wide, so a square sleeve drawn one
/// character to a pixel comes out standing up. The number is the font the
/// stylesheet names for a cover, at the size it names: twenty points of line
/// against eight of advance.
///
/// It was five thirds, which is the advance against the *size* -- and the line
/// is not the size. `line-height: 1` is the font's own line, which at fourteen
/// points is twenty of them and not fourteen, so the grid was a third too
/// narrow for what was drawn on it. The size is pinned in the stylesheet and
/// the family is named there rather than left to whichever monospace font
/// answers first, because both halves of this number are somebody else's font
/// otherwise.
pub const CELL_ASPECT: f64 = 20.0 / 8.0;

/// One cell: the character, and the colour the pixel under it was.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub rgb: (u8, u8, u8),
}

/// A cover, drawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cover {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
}

impl Cover {
    /// The cover as Pango markup, one span to a run of one colour.
    ///
    /// Runs rather than a span for every character, because a sleeve is mostly
    /// flat colour and a span apiece is thousands of them for one picture.
    pub fn markup(&self) -> String {
        let mut out = String::new();

        for line in self.cells.chunks(self.cols) {
            let mut colour = None;
            let mut run = String::new();

            for cell in line {
                if colour != Some(cell.rgb) {
                    close(&mut out, &run, colour);
                    (colour, run) = (Some(cell.rgb), String::new());
                }

                run.push_str(&escaped(cell.ch));
            }

            close(&mut out, &run, colour);
            out.push('\n');
        }

        out.trim_end().to_string()
    }

    /// The lines, as text, with the colours dropped.
    pub fn plain(&self) -> String {
        self.cells
            .chunks(self.cols)
            .map(|line| line.iter().map(|cell| cell.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One run of characters, in the colour they were.
fn close(out: &mut String, run: &str, colour: Option<(u8, u8, u8)>) {
    let Some((r, g, b)) = colour else { return };

    if !run.is_empty() {
        out.push_str(&format!("<span foreground=\"#{r:02x}{g:02x}{b:02x}\">{run}</span>"));
    }
}

/// A character of the ramp, as markup can hold it.
fn escaped(ch: char) -> String {
    match ch {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '\'' => "&apos;".to_string(),
        '"' => "&quot;".to_string(),
        _ => ch.to_string(),
    }
}

/// What a pixel is written as.
pub fn character(rgb: (u8, u8, u8)) -> char {
    let (levels, lit) = (levels(), usize::from(luminance(rgb)));
    let step = lit * levels / 256;
    RAMP.chars().nth(levels - step).unwrap_or(' ')
}

/// How bright a pixel is, by Rec. 709.
pub fn luminance((r, g, b): (u8, u8, u8)) -> u8 {
    toward_zero_u8(0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b))
}

/// A picture file, read as characters `rows` tall.
///
/// The middle square of it, drawn to fill the grid. What a player writes out
/// for a cover is not always a cover: kew hands over a wide picture with the
/// record in the middle of it and a margin either side, and drawn as it came
/// that is a letterbox with two bands of punctuation where a sleeve should be.
///
/// The grid is `CELL_ASPECT` wider than it is tall and the square is stretched
/// across it, which is the same correction kew makes in
/// `draw_square_bitmap_to_buf`: the cell does the standing up, so the picture
/// must not. Fitted into the grid on its own pixels instead -- which is what
/// this did -- a square sleeve took as many columns as it had rows and left
/// the rest of the grid empty either side, and what was drawn stood up by the
/// whole of the cell's own shape.
pub fn read(path: &Path, rows: usize) -> Option<Cover> {
    let cols = whole_usize(rows.float() * CELL_ASPECT);

    let (Ok(wide), Ok(tall)) = (i32::try_from(cols), i32::try_from(rows)) else {
        return None;
    };

    let Ok(whole) = Pixbuf::from_file(path) else { return None };

    let square = middle(&whole);
    let picture = square.scale_simple(wide, tall, InterpType::Bilinear)?;

    Some(laid_out(&picture, cols, rows))
}

/// The middle square of a picture, which is where the record is.
fn middle(whole: &Pixbuf) -> Pixbuf {
    let side = whole.width().min(whole.height());
    whole.new_subpixbuf((whole.width() - side) / 2, (whole.height() - side) / 2, side, side)
}

/// The room a cover takes, with nothing in it.
///
/// A player says what is playing before it says where the picture is, so the
/// square is held from the moment the song changes: a card that kept no room
/// until the picture arrived grew a sleeve's worth taller under a thumb that
/// had already started reading it.
///
/// Written in no-break spaces rather than ordinary ones. The room is the whole
/// of what this is for, and a run of ordinary spaces at the end of a line is a
/// run the layout is free not to measure -- an empty sleeve that measured
/// nothing would be no room at all.
pub fn room(rows: usize) -> Cover {
    let cols = whole_usize(rows.float() * CELL_ASPECT);
    let blank = Cell { ch: '\u{a0}', rgb: (0, 0, 0) };

    Cover { cols, rows, cells: vec![blank; cols * rows] }
}

/// The scaled picture, centred in a grid of that many columns and rows.
fn laid_out(picture: &Pixbuf, cols: usize, rows: usize) -> Cover {
    let blank = Cell { ch: ' ', rgb: (0, 0, 0) };
    let mut cells = vec![blank; cols * rows];
    let (bytes, stride) = (picture.read_pixel_bytes(), fitted::<i32, usize>(picture.rowstride()));
    let channels: usize = fitted(picture.n_channels());
    let (wide, tall) = (fitted::<i32, usize>(picture.width()), fitted::<i32, usize>(picture.height()));
    let (left, top) = ((cols.saturating_sub(wide)) / 2, (rows.saturating_sub(tall)) / 2);

    for down in 0..tall.min(rows) {
        for across in 0..wide.min(cols) {
            let at = down * stride + across * channels;
            let rgb = match bytes.get(at..at + 3) {
                Some([r, g, b]) => (*r, *g, *b),
                _ => (0, 0, 0),
            };
            cells[(top + down) * cols + left + across] = Cell { ch: character(rgb), rgb };
        }
    }

    Cover { cols, rows, cells }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// White comes out one short of the head of the ramp, because the
    /// luminance of white is 254 and not 255. kew rounds the same way, and the
    /// port is worth more than the missing character.
    #[test]
    fn the_brightest_pixel_is_the_densest_character_kew_can_reach() {
        assert_eq!(character((255, 255, 255)), '@');
        assert_eq!(RAMP.chars().next(), Some('$'));
    }

    #[test]
    fn the_darkest_pixel_is_the_faintest_character() {
        assert_eq!(character((0, 0, 0)), '.');
    }

    #[test]
    fn green_reads_brighter_than_blue() {
        assert!(luminance((0, 255, 0)) > luminance((0, 0, 255)));
    }

    #[test]
    fn a_run_of_one_colour_is_one_span() {
        let white = Cell { ch: '@', rgb: (255, 255, 255) };
        let cover = Cover { cols: 3, rows: 1, cells: vec![white; 3] };
        assert_eq!(cover.markup(), "<span foreground=\"#ffffff\">@@@</span>");
    }

    #[test]
    fn a_character_the_markup_would_choke_on_is_escaped() {
        let cell = |ch| Cell { ch, rgb: (1, 2, 3) };
        let cover = Cover { cols: 3, rows: 1, cells: vec![cell('<'), cell('&'), cell('>')] };
        assert_eq!(cover.markup(), "<span foreground=\"#010203\">&lt;&amp;&gt;</span>");
    }

    #[test]
    fn every_pixel_lands_on_the_ramp() {
        for lit in 0..=255u8 {
            assert!(RAMP.contains(character((lit, lit, lit))));
        }
    }
}
