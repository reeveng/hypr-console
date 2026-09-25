//! A cover, read as characters.
//!
//! This is kew's renderer, in Rust: the same ramp, the same luminance, the same
//! rescale. `src/utils/img_utils.c` in the fork is the original, and a change
//! there is a change here.


use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u8, whole_u32};
use console_core_shapes::Pixels;
use std::path::Path;

const DARKEST: char = ' ';


pub const RAMP: &str = "$@&B%8WM#ZO0QoahkbdpqwmLCJUYXIjft/\\|()1{}[]l?zcvunxr!<>i;:*-+~_,\"^`'.";

fn levels() -> Result<u32, Never> {
    let Ok(many) = fitted::<_, u32>(RAMP.chars().count());

    Ok(many.saturating_sub(1))
}

pub const CELL_ASPECT: f64 = 20.0 / 8.0;

const CHANNELS: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub rgb: (u8, u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
}

impl Grid {
    pub fn of(rows: u32) -> Result<Self, Never> {
        let Ok(cols) = whole_u32(f64::from(rows) * CELL_ASPECT);

        Ok(Self { cols, rows })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cover {
    pub cols: u32,
    pub rows: u32,
    pub cells: Vec<Cell>,
}

impl Cover {
    pub fn markup(&self) -> Result<String, Never> {
        let mut out = String::new();

        let Ok(cols) = index(self.cols);

        for line in self.cells.chunks(cols) {
            let mut color = None;
            let mut run = String::new();

            for cell in line {
                match color == Some(cell.rgb) {
                    true => {},
                    false => {
                        close(&mut out, &run, color)?;
                        (color, run) = (Some(cell.rgb), String::new());
                    }
                }

                let letter = escaped(cell.ch)?;

                run.push_str(&letter);
            }

            close(&mut out, &run, color)?;
            out.push('\n');
        }

        Ok(out.trim_end().to_string())
    }

    pub fn plain(&self) -> Result<String, Never> {
        let Ok(cols) = index(self.cols);

        Ok(self
            .cells
            .chunks(cols)
            .map(|line| line.iter().map(|cell| cell.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

fn close(out: &mut String, run: &str, color: Option<(u8, u8, u8)>) -> Result<(), Never> {
    let (r, g, b) = match color {
        Some((r, g, b)) => (r, g, b),
        None => return Ok(()),
    };

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
    let lit = u32::from(luminance);
    let step = lit.saturating_mul(levels).saturating_div(256);
    let Ok(at) = index(levels.saturating_sub(step));

    Ok(match RAMP.chars().nth(at) {
        Some(mark) => mark,
        None => DARKEST,
    })
}

pub fn luminance((r, g, b): (u8, u8, u8)) -> Result<u8, Never> {
    toward_zero_u8(0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b))
}

pub fn read(path: &Path, rows: u32) -> Result<Option<Cover>, Never> {
    let grid = Grid::of(rows)?;

    let read = console_pictures::square(path, Size { width: grid.cols, height: grid.rows });

    let picture = match read {
        Ok(Some(picture)) => picture,
        Ok(None) => return Ok(None),
        Err(why) => {
            eprintln!("console-music: {}: {why}", path.display());

            return Ok(None);
        }
    };

    let cover = laid_out(&picture, grid)?;

    Ok(Some(cover))
}

pub fn room(rows: u32) -> Result<Cover, Never> {
    let Grid { cols, rows } = Grid::of(rows)?;

    let blank = Cell { ch: '\u{a0}', rgb: (0, 0, 0) };
    let Ok(many) = index(cols.saturating_mul(rows));

    Ok(Cover { cols, rows, cells: vec![blank; many] })
}

fn laid_out(picture: &Pixels, grid: Grid) -> Result<Cover, Never> {
    let Grid { cols, rows } = grid;
    let blank = Cell { ch: ' ', rgb: (0, 0, 0) };
    let Ok(many) = index(cols.saturating_mul(rows));
    let mut cells = vec![blank; many];
    let (stride, wide, tall) = (picture.stride, picture.width, picture.height);

    let bytes = &picture.bytes;
    let (left, top) =
        (cols.saturating_sub(wide).saturating_div(2), rows.saturating_sub(tall).saturating_div(2));

    for down in 0..tall.min(rows) {
        for across in 0..wide.min(cols) {
            let Ok(at) = index(down.saturating_mul(stride).saturating_add(across.saturating_mul(CHANNELS)));
            let rgb = match bytes.get(at..at.saturating_add(3)) {
                Some([r, g, b]) => (*r, *g, *b),
                Some(_) | None => (0, 0, 0),
            };
            let cell = top.saturating_add(down).saturating_mul(cols)
                .saturating_add(left)
                .saturating_add(across);
            let ch = character(rgb)?;
            let Ok(cell) = index(cell);

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
    fn a_run_of_one_color_is_one_span() {
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
