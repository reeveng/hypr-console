//! A cover, read as characters.
//!
//! This is kew's renderer, in Rust: the same ramp, the same luminance, the same
//! rescale. `src/utils/img_utils.c` in the fork is the original, and a change
//! there is a change here.


use console_never::Never;
use console_number_conversion::{Float, fitted, toward_zero_u8, whole_usize};
use std::path::Path;

use gtk4::gdk_pixbuf::{InterpType, Pixbuf};

pub const RAMP: &str = "$@&B%8WM#ZO0QoahkbdpqwmLCJUYXIjft/\\|()1{}[]l?zcvunxr!<>i;:*-+~_,\"^`'.";

fn levels() -> Result<usize, Never> {
    Ok(RAMP.chars().count().saturating_sub(1))
}

pub const CELL_ASPECT: f64 = 20.0 / 8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub rgb: (u8, u8, u8),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cover {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<Cell>,
}

impl Cover {
    pub fn markup(&self) -> Result<String, Never> {
        let mut out = String::new();

        for line in self.cells.chunks(self.cols) {
            let mut colour = None;
            let mut run = String::new();

            for cell in line {
                match colour == Some(cell.rgb) {
                    true => {},
                    false => {
                        close(&mut out, &run, colour)?;
                        (colour, run) = (Some(cell.rgb), String::new());
                    }
                }

                let letter = escaped(cell.ch)?;

                run.push_str(&letter);
            }

            close(&mut out, &run, colour)?;
            out.push('\n');
        }

        Ok(out.trim_end().to_string())
    }

    pub fn plain(&self) -> Result<String, Never> {
        Ok(self
            .cells
            .chunks(self.cols)
            .map(|line| line.iter().map(|cell| cell.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

fn close(out: &mut String, run: &str, colour: Option<(u8, u8, u8)>) -> Result<(), Never> {
    let Some((r, g, b)) = colour else { return Ok(()) };

    match run.is_empty() {
        true => {},
        false => {
            out.push_str(&format!("<span foreground=\"#{r:02x}{g:02x}{b:02x}\">{run}</span>"));
        }
    }

    Ok(())
}

fn escaped(ch: char) -> Result<String, Never> {
    Ok(match ch {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '\'' => "&apos;".to_string(),
        '"' => "&quot;".to_string(),
        _ => ch.to_string(),
    })
}

pub fn character(rgb: (u8, u8, u8)) -> Result<char, Never> {
    let levels = levels()?;
    let luminance = luminance(rgb)?;
    let lit = usize::from(luminance);
    let step = lit.saturating_mul(levels).saturating_div(256);

    Ok(RAMP.chars().nth(levels.saturating_sub(step)).unwrap_or(' '))
}

pub fn luminance((r, g, b): (u8, u8, u8)) -> Result<u8, Never> {
    toward_zero_u8(0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b))
}

pub fn read(path: &Path, rows: usize) -> Result<Option<Cover>, Never> {
    let Ok(down) = rows.float();
    let Ok(cols) = whole_usize(down * CELL_ASPECT);

    let (Ok(wide), Ok(tall)) = (i32::try_from(cols), i32::try_from(rows)) else {
        return Ok(None);
    };

    let Ok(whole) = Pixbuf::from_file(path) else { return Ok(None) };

    let square = middle(&whole)?;

    let Some(picture) = square.scale_simple(wide, tall, InterpType::Bilinear) else {
        return Ok(None);
    };

    let cover = laid_out(&picture, cols, rows)?;

    Ok(Some(cover))
}

fn middle(whole: &Pixbuf) -> Result<Pixbuf, Never> {
    let side = whole.width().min(whole.height());

    Ok(whole.new_subpixbuf(
        whole.width().saturating_sub(side).saturating_div(2),
        whole.height().saturating_sub(side).saturating_div(2),
        side,
        side,
    ))
}

pub fn room(rows: usize) -> Result<Cover, Never> {
    let Ok(down) = rows.float();
    let Ok(cols) = whole_usize(down * CELL_ASPECT);

    let blank = Cell { ch: '\u{a0}', rgb: (0, 0, 0) };

    Ok(Cover { cols, rows, cells: vec![blank; cols.saturating_mul(rows)] })
}

fn laid_out(picture: &Pixbuf, cols: usize, rows: usize) -> Result<Cover, Never> {
    let blank = Cell { ch: ' ', rgb: (0, 0, 0) };
    let mut cells = vec![blank; cols.saturating_mul(rows)];
    let Ok(stride) = fitted::<i32, usize>(picture.rowstride());
    let Ok(channels) = fitted::<i32, usize>(picture.n_channels());
    let Ok(wide) = fitted::<i32, usize>(picture.width());
    let Ok(tall) = fitted::<i32, usize>(picture.height());

    let bytes = picture.read_pixel_bytes();
    let (left, top) =
        (cols.saturating_sub(wide).saturating_div(2), rows.saturating_sub(tall).saturating_div(2));

    for down in 0..tall.min(rows) {
        for across in 0..wide.min(cols) {
            let at = down.saturating_mul(stride).saturating_add(across.saturating_mul(channels));
            let rgb = match bytes.get(at..at.saturating_add(3)) {
                Some([r, g, b]) => (*r, *g, *b),
                Some(_) | None => (0, 0, 0),
            };
            let cell = top.saturating_add(down).saturating_mul(cols)
                .saturating_add(left)
                .saturating_add(across);
            let ch = character(rgb)?;

            match cells.get_mut(cell) {
                Some(cell) => *cell = Cell { ch, rgb },
                None => {},
            }
        }
    }

    Ok(Cover { cols, rows, cells })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brightest_pixel_is_the_densest_character_kew_can_reach() {
        assert_eq!(character((255, 255, 255)), Ok('@'));
        assert_eq!(RAMP.chars().next(), Some('$'));
    }

    #[test]
    fn the_darkest_pixel_is_the_faintest_character() {
        assert_eq!(character((0, 0, 0)), Ok('.'));
    }

    #[test]
    fn green_reads_brighter_than_blue() {
        let Ok(green) = luminance((0, 255, 0));

        let Ok(blue) = luminance((0, 0, 255));

        assert!(green > blue);
    }

    #[test]
    fn a_run_of_one_colour_is_one_span() {
        let white = Cell { ch: '@', rgb: (255, 255, 255) };
        let cover = Cover { cols: 3, rows: 1, cells: vec![white; 3] };

        assert_eq!(cover.markup(), Ok("<span foreground=\"#ffffff\">@@@</span>".to_string()));
    }

    #[test]
    fn a_character_the_markup_would_choke_on_is_escaped() {
        let cell = |ch| Cell { ch, rgb: (1, 2, 3) };
        let cover = Cover { cols: 3, rows: 1, cells: vec![cell('<'), cell('&'), cell('>')] };

        assert_eq!(cover.markup(), Ok("<span foreground=\"#010203\">&lt;&amp;&gt;</span>".to_string()));
    }

    #[test]
    fn every_pixel_lands_on_the_ramp() {
        for lit in 0..=255u8 {
            let Ok(ch) = character((lit, lit, lit));

            assert!(RAMP.contains(ch));
        }
    }
}
