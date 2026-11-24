//! A cover, read as characters.
//!
//! This is kew's renderer, in Rust: the same ramp, the same luminance, the same
//! rescale. `src/utils/img_utils.c` in the fork is the original, and a change
//! there is a change here.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u8, whole_u32};
use console_core_shapes::Pixels;
use console_core_size::Size;
use std::num::NonZero;
use std::path::Path;

const DARKEST: char = ' ';

pub const RAMP: &str = "$@&B%8WM#ZO0QoahkbdpqwmLCJUYXIjft/\\|()1{}[]l?zcvunxr!<>i;:*-+~_,\"^`'.";

fn levels() -> Result<u32, Never> {
  let Ok(many) = fitted::<_, u32>(RAMP.chars().count());

  Ok(many.saturating_sub(1))
}

pub const CELL_ASPECT: f64 = 20.0 / 8.0;

const CHANNELS: u32 = 4;
const MAX_ROWS: u32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cell {
  pub character: char,
  pub rgb: (u8, u8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grid {
  pub columns: u32,
  pub rows: u32,
}

impl Grid {
  pub fn of(rows: u32) -> Result<Self, Never> {
    let bounded_rows = rows.min(MAX_ROWS);
    let Ok(columns) = whole_u32(f64::from(bounded_rows) * CELL_ASPECT);

    Ok(Self {
      columns,
      rows: bounded_rows,
    })
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cover {
  pub columns: u32,
  pub rows: u32,
  pub cells: Vec<Cell>,
}

impl Cover {
  pub fn lines(&self) -> Result<impl Iterator<Item = &[Cell]>, Never> {
    let Ok(columns) = index(self.columns);

    Ok(
      NonZero::new(columns)
        .into_iter()
        .flat_map(|width| self.cells.chunks(width.get())),
    )
  }

  pub fn markup(&self) -> Result<String, Never> {
    let mut out = String::new();
    let lines = self.lines()?;

    for line in lines {
      let mut color = None;
      let mut run = String::new();

      for cell in line {
        match ::core::primitive::bool::then_some(color == Some(cell.rgb), ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => {
            close(&mut out, &run, color)?;
            (color, run) = (Some(cell.rgb), String::new());
          }
        }

        let letter = escaped(cell.character)?;

        run.push_str(&letter);
      }

      close(&mut out, &run, color)?;
      out.push('\n');
    }

    Ok(out.trim_end().to_string())
  }

  pub fn plain(&self) -> Result<String, Never> {
    let lines = self.lines()?;

    Ok(
      lines
        .map(|line| line.iter().map(|cell| cell.character).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n"),
    )
  }
}

fn close(out: &mut String, run: &str, color: Option<(u8, u8, u8)>) -> Result<(), Never> {
  let (red, green, blue) = match color {
    Some((red, green, blue)) => (red, green, blue),
    None => return Ok(()),
  };

  match ::core::primitive::bool::then_some(run.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      out.push_str(&format!(
        "<span foreground=\"#{red:02x}{green:02x}{blue:02x}\">{run}</span>"
      ));
    }
  }

  Ok(())
}

fn escaped(character: char) -> Result<String, Never> {
  Ok(match character {
    '&' => "&amp;".to_string(),
    '<' => "&lt;".to_string(),
    '>' => "&gt;".to_string(),
    '\'' => "&apos;".to_string(),
    '"' => "&quot;".to_string(),
    _ => character.to_string(),
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

pub fn luminance((red, green, blue): (u8, u8, u8)) -> Result<u8, Never> {
  toward_zero_u8(0.2126 * f64::from(red) + 0.7152 * f64::from(green) + 0.0722 * f64::from(blue))
}

pub fn read(path: &Path, rows: u32) -> Result<Option<Cover>, Never> {
  let grid = Grid::of(rows)?;

  let read = console_pictures::square(
    path,
    Size {
      width: grid.columns,
      height: grid.rows,
    },
  );

  let picture = match read {
    Ok(Some(picture)) => picture,
    Ok(None) => return Ok(None),
    Err(why) => {
      console_core_logging::warn!("console-music: {}: {why}", path.display());

      return Ok(None);
    }
  };

  let cover = laid_out(&picture, grid)?;

  Ok(Some(cover))
}

pub fn room(rows: u32) -> Result<Cover, Never> {
  let grid = Grid::of(rows)?;
  let blank = Cell {
    character: '\u{a0}',
    rgb: (0, 0, 0),
  };
  let cells = blank_cells(grid, blank)?;

  Ok(Cover {
    columns: grid.columns,
    rows: grid.rows,
    cells,
  })
}

fn blank_cells(grid: Grid, blank: Cell) -> Result<Vec<Cell>, Never> {
  let Ok(many) = index(grid.columns.saturating_mul(grid.rows));

  Ok(vec![blank; many.min(const { 256 * 640 })])
}

fn laid_out(picture: &Pixels, grid: Grid) -> Result<Cover, Never> {
  let Grid { columns, rows } = grid;
  let blank = Cell {
    character: ' ',
    rgb: (0, 0, 0),
  };
  let mut cells = blank_cells(grid, blank)?;
  let (stride, wide, tall) = (picture.stride, picture.width, picture.height);

  let bytes = &picture.bytes;
  let (left, top) = (
    columns.saturating_sub(wide).saturating_div(2),
    rows.saturating_sub(tall).saturating_div(2),
  );

  for down in 0..tall.min(rows) {
    for across in 0..wide.min(columns) {
      let Ok(at) = index(
        down
          .saturating_mul(stride)
          .saturating_add(across.saturating_mul(CHANNELS)),
      );
      let rgb = match bytes.get(at..at.saturating_add(3)) {
        Some([red, green, blue]) => (*red, *green, *blue),
        Some(_) | None => (0, 0, 0),
      };
      let cell = top
        .saturating_add(down)
        .saturating_mul(columns)
        .saturating_add(left)
        .saturating_add(across);
      let character = character(rgb)?;
      let Ok(cell_2) = index(cell);

      match cells.get_mut(cell_2) {
        Some(cell_3) => *cell_3 = Cell { character, rgb },
        None => {}
      }
    }
  }

  Ok(Cover {
    columns,
    rows,
    cells,
  })
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
    let white = Cell {
      character: '@',
      rgb: (255, 255, 255),
    };
    let cover = Cover {
      columns: 3,
      rows: 1,
      cells: vec![white; 3],
    };

    assert_eq!(
      cover.markup(),
      Ok("<span foreground=\"#ffffff\">@@@</span>".to_string())
    );
  }

  #[test]
  fn a_character_the_markup_would_choke_on_is_escaped() {
    let cell = |character| Cell {
      character,
      rgb: (1, 2, 3),
    };
    let cover = Cover {
      columns: 3,
      rows: 1,
      cells: vec![cell('<'), cell('&'), cell('>')],
    };

    assert_eq!(
      cover.markup(),
      Ok("<span foreground=\"#010203\">&lt;&amp;&gt;</span>".to_string())
    );
  }

  #[test]
  fn every_pixel_lands_on_the_ramp() {
    for lit in 0..=255u8 {
      let Ok(character) = character((lit, lit, lit));

      assert!(RAMP.contains(character));
    }
  }

  #[test]
  fn an_empty_cover_renders_and_an_oversized_cover_stays_bounded() {
    let Ok(empty) = room(0);
    let Ok(large) = room(u32::MAX);

    assert_eq!(empty.markup(), Ok(String::new()));
    assert_eq!(empty.plain(), Ok(String::new()));
    assert_eq!(large.rows, MAX_ROWS);
    assert_eq!(large.cells.len(), 163_840);
  }
}
