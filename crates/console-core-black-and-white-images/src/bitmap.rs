//! Pixels held one to a byte, with every question about a place outside them answered white.

use console_core_fax_images::{Color, Decoder};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::error::DecodeError;

pub(crate) const MAX_DIMENSION: u32 = 65_535;

pub(crate) const MAX_PIXELS: u64 = 1 << 28;

const CHUNK: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Operator {
  #[default]
  Or,
  And,
  Xor,
  Xnor,
  Replace,
}

impl Operator {
  pub(crate) fn from_bits(value: u8) -> Result<Operator, DecodeError> {
    match value & 0x07 {
      0 => Ok(Operator::Or),
      1 => Ok(Operator::And),
      2 => Ok(Operator::Xor),
      3 => Ok(Operator::Xnor),
      4 => Ok(Operator::Replace),
      _ => Err(DecodeError::InvalidCombinationOperator),
    }
  }

  fn apply(self, pixels: Pixels) -> Result<u8, Never> {
    Ok(match self {
      Operator::Or => pixels.destination | pixels.source,
      Operator::And => pixels.destination & pixels.source,
      Operator::Xor => pixels.destination ^ pixels.source,
      Operator::Xnor => 1 ^ pixels.destination ^ pixels.source,
      Operator::Replace => pixels.source,
    })
  }
}

#[derive(Debug, Clone)]
pub(crate) struct Bitmap {
  pub(crate) size: Size<u32>,
  pub(crate) origin: Point<u32>,
  pixels: Vec<u8>,
}

impl Bitmap {
  pub(crate) fn new(
    size: Size<u32>,
    origin: Point<u32>,
    fill: Color,
  ) -> Result<Bitmap, DecodeError> {
    let Ok(area) = size.area();
    let within = (
      size.width <= MAX_DIMENSION,
      size.height <= MAX_DIMENSION,
      area <= MAX_PIXELS,
    );

    match within {
      (true, true, true) => {}
      (false, _, _) | (_, false, _) | (_, _, false) => return Err(DecodeError::Overflow),
    }

    let value = match fill {
      Color::Black => 1u8,
      Color::White => 0u8,
    };
    let Ok(count) = index(area);

    #[cfg_attr(
      dylint_lib = "explicit088_no_arriving_length",
      expect(
        explicit088_no_arriving_length,
        reason = "both dimensions and the pixel count have been refused above MAX_DIMENSION and MAX_PIXELS before converting to an allocation index"
      )
    )]
    let pixels = vec![value; count];

    Ok(Bitmap {
      size,
      origin,
      pixels,
    })
  }

  pub(crate) fn white(size: Size<u32>) -> Result<Bitmap, DecodeError> {
    Bitmap::new(size, Point { x: 0, y: 0 }, Color::White)
  }

  pub(crate) fn shift_to(&self, region: Point<u32>) -> Result<Point<i32>, DecodeError> {
    let held_x = i32::try_from(self.origin.x).map_err(|_| DecodeError::Overflow)?;
    let held_y = i32::try_from(self.origin.y).map_err(|_| DecodeError::Overflow)?;
    let region_x = i32::try_from(region.x).map_err(|_| DecodeError::Overflow)?;
    let region_y = i32::try_from(region.y).map_err(|_| DecodeError::Overflow)?;
    let x = held_x.checked_sub(region_x).ok_or(DecodeError::Overflow)?;
    let y = held_y.checked_sub(region_y).ok_or(DecodeError::Overflow)?;

    Ok(Point { x, y })
  }

  pub(crate) fn pixels(&self) -> Result<&[u8], Never> {
    Ok(&self.pixels)
  }

  fn inside(&self, at: Point<u32>) -> Result<Option<u64>, Never> {
    let within = (at.x < self.size.width, at.y < self.size.height);

    Ok(match within {
      (true, true) => Some(
        u64::from(at.y)
          .saturating_mul(u64::from(self.size.width))
          .saturating_add(u64::from(at.x)),
      ),
      (false, _) | (_, false) => None,
    })
  }

  fn slot(&self, at: Point<i32>) -> Result<Option<u64>, Never> {
    match (u32::try_from(at.x), u32::try_from(at.y)) {
      (Ok(x), Ok(y)) => self.inside(Point { x, y }),
      (Err(_negative), _) | (_, Err(_negative)) => Ok(None),
    }
  }

  pub(crate) fn get(&self, at: Point<i32>) -> Result<u8, Never> {
    let Ok(place) = self.slot(at);

    Ok(match place {
      Some(place_2) => {
        let Ok(slot) = index(place_2);

        match self.pixels.get(slot) {
          Some(pixel) => *pixel,
          None => 0,
        }
      }
      None => 0,
    })
  }

  pub(crate) fn put(&mut self, at: Point<u32>, bit: u32) -> Result<(), Never> {
    let Ok(place) = self.inside(at);

    match place {
      Some(place_2) => {
        let Ok(slot) = index(place_2);

        match self.pixels.get_mut(slot) {
          Some(cell) => *cell = u8::from(bit != 0),
          None => {}
        }
      }
      None => {}
    }

    Ok(())
  }

  pub(crate) fn repeat_above(&mut self, y: u32) -> Result<(), Never> {
    match y.checked_sub(1) {
      Some(above) => {
        let Ok(from) = index(u64::from(above).saturating_mul(u64::from(self.size.width)));
        let Ok(width) = index(self.size.width);
        let copied = self
          .pixels
          .get(from..from.saturating_add(width))
          .map(<[u8]>::to_vec);
        let Ok(row) = self.row_mut(y);

        match (row, copied) {
          (Some(row_2), Some(copied_2)) => row_2
            .iter_mut()
            .zip(copied_2)
            .for_each(|(pixel, above_2)| *pixel = above_2),
          (None, _) | (_, None) => {}
        }

        Ok(())
      }
      None => Ok(()),
    }
  }

  pub(crate) fn combine(
    &mut self,
    other: &Bitmap,
    at: Point<i32>,
    operator: Operator,
  ) -> Result<(), Never> {
    let Ok(span) = Span::between(
      Widths {
        destination: self.size.width,
        source: other.size.width,
      },
      at.x,
    );
    let Ok(width) = index(other.size.width);

    for (source, source_y) in other.pixels.chunks_exact(width.max(1)).zip(0u32..) {
      let Ok(taken) = fitted::<_, i32>(source_y);
      let target = match u32::try_from(at.y.saturating_add(taken)) {
        Ok(row) => row,
        Err(_above) => continue,
      };
      let Ok(row) = self.row_mut(target);

      match row {
        Some(row_2) => span.blend(row_2, source, operator)?,
        None => {}
      }
    }

    Ok(())
  }

  fn row_mut(&mut self, y: u32) -> Result<Option<&mut [u8]>, Never> {
    let Ok(from) = index(u64::from(y).saturating_mul(u64::from(self.size.width)));
    let Ok(width) = index(self.size.width);

    Ok(self.pixels.get_mut(from..from.saturating_add(width)))
  }

  pub(crate) fn emit(&self, decoder: &mut impl Decoder) -> Result<(), Never> {
    let Ok(width) = index(self.size.width);
    let Ok(chunk) = index(CHUNK);

    for row in self.pixels.chunks_exact(width.max(1)) {
      let mut run: Option<(Color, u32)> = None;

      for pieces in row.chunks(chunk.max(1)) {
        let continued = emit_chunk(decoder, run, pieces)?;

        run = continued;
      }

      match run {
        Some((color, count)) => decoder.push_pixel_chunk(color, count)?,
        None => {}
      }

      decoder.next_line()?;
    }

    Ok(())
  }
}

fn color_of(pixel: u8) -> Result<Color, Never> {
  Ok(match pixel {
    0 => Color::White,
    _ => Color::Black,
  })
}

fn emit_chunk(
  decoder: &mut impl Decoder,
  run: Option<(Color, u32)>,
  chunk: &[u8],
) -> Result<Option<(Color, u32)>, Never> {
  let Ok(full) = index(CHUNK);
  let uniform = match chunk.first() {
    Some(first) => chunk.len() == full && chunk.iter().all(|pixel| pixel == first),
    None => false,
  };

  match (uniform, chunk.first(), run) {
    (true, Some(first), Some((color, count))) => {
      let Ok(this) = color_of(*first);

      match ::core::primitive::bool::then_some(this == color, ()) {
        ::core::option::Option::Some(()) => Ok(Some((color, count.saturating_add(1)))),
        ::core::option::Option::None => {
          decoder.push_pixel_chunk(color, count)?;

          Ok(Some((this, 1)))
        }
      }
    }
    (true, Some(first), None) => {
      let Ok(this) = color_of(*first);

      Ok(Some((this, 1)))
    }
    (false, _, _) | (true, None, _) => {
      match run {
        Some((color, count)) => decoder.push_pixel_chunk(color, count)?,
        None => {}
      }

      for pixel in chunk {
        let Ok(color) = color_of(*pixel);

        decoder.push_pixel(color)?;
      }

      Ok(None)
    }
  }
}

#[derive(Debug, Clone, Copy)]
struct Pixels {
  destination: u8,
  source: u8,
}

#[derive(Debug, Clone, Copy)]
struct Widths {
  destination: u32,
  source: u32,
}

#[derive(Debug, Clone, Copy)]
struct Span {
  destination: u32,
  source: u32,
  length: u32,
}

impl Span {
  fn between(widths: Widths, at: i32) -> Result<Span, Never> {
    let start = i64::from(at).max(0);
    let from = start.saturating_sub(i64::from(at));
    let room = i64::from(widths.destination).saturating_sub(start);
    let left = i64::from(widths.source).saturating_sub(from);
    let Ok(destination) = fitted::<_, u32>(start);
    let Ok(source) = fitted::<_, u32>(from);
    let Ok(length) = fitted::<_, u32>(room.min(left).max(0));

    Ok(Span {
      destination,
      source,
      length,
    })
  }

  fn blend(self, row: &mut [u8], source: &[u8], operator: Operator) -> Result<(), Never> {
    let Ok(destination) = index(self.destination);
    let Ok(from) = index(self.source);
    let Ok(length) = index(self.length);
    let targets = row.iter_mut().skip(destination).take(length);

    for (target, pixel) in targets.zip(source.iter().skip(from)) {
      let Ok(blended) = operator.apply(Pixels {
        destination: *target,
        source: *pixel,
      });

      *target = blended;
    }

    Ok(())
  }
}
