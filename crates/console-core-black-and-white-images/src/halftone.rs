//! Halftone regions: a grid of gray values, each one a pattern from a dictionary laid on the page.

use console_core_fax_images::Color;
use console_core_geometry::{Point, Size};
use console_core_mq_decoding::Decoder as Arithmetic;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::arithmetic;
use crate::bitmap::{Bitmap, MAX_DIMENSION, MAX_PIXELS, Operator};
use crate::error::DecodeError;
use crate::generic::{self, Coding, Prediction, Settings, Template};
use crate::mmr;
use crate::reading::Reader;
use crate::regions::RegionInfo;

#[derive(Debug, Clone)]
pub(crate) struct Patterns {
  bitmaps: Vec<Bitmap>,
  size: Size<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct PatternHeader<'a> {
  coding: Coding,
  template: Template,
  size: Size<u8>,
  count: u32,
  data: &'a [u8],
}

pub(crate) fn parse_patterns<'a>(
  reader: &mut Reader<'a>,
) -> Result<PatternHeader<'a>, DecodeError> {
  let flags = reader.byte()?;
  let coding = match flags & 1 {
    0 => Coding::Arithmetic,
    _ => Coding::Mmr,
  };
  let Ok(template) = Template::from_bits(flags.wrapping_shr(1));
  let width = reader.nonzero_byte()?;
  let height = reader.nonzero_byte()?;
  let count = reader.u32()?;
  let data = reader.tail()?;

  Ok(PatternHeader {
    coding,
    template,
    size: Size { width, height },
    count,
    data,
  })
}

pub(crate) fn decode_patterns(header: &PatternHeader<'_>) -> Result<Patterns, DecodeError> {
  let size = Size {
    width: u32::from(header.size.width),
    height: u32::from(header.size.height),
  };
  let total = header.count.checked_add(1).ok_or(DecodeError::Overflow)?;
  let wide = total.checked_mul(size.width).ok_or(DecodeError::Overflow)?;
  let mut collective = Bitmap::white(Size {
    width: wide,
    height: size.height,
  })?;

  match header.coding {
    Coding::Mmr => {
      let _read = mmr::decode(&mut collective, header.data)?;
    }
    Coding::Arithmetic => decode_collective(&mut collective, header, size.width)?,
  }

  let mut bitmaps = Vec::new();

  for gray in 0..total {
    let mut pattern = Bitmap::white(size)?;
    let start =
      i32::try_from(gray.saturating_mul(size.width)).map_err(|_| DecodeError::Overflow)?;
    let Ok(()) = pattern.combine(
      &collective,
      Point {
        x: start.saturating_neg(),
        y: 0,
      },
      Operator::Replace,
    );

    bitmaps.push(pattern);
  }

  Ok(Patterns { bitmaps, size })
}

fn decode_collective(
  collective: &mut Bitmap,
  header: &PatternHeader<'_>,
  width: u32,
) -> Result<(), DecodeError> {
  let Ok(mut adaptive) = header.template.usual_pixels();
  let first = i8::try_from(width).map_err(|_| DecodeError::InvalidAdaptivePixel)?;

  match adaptive.first_mut() {
    Some(slot) => {
      *slot = Point {
        x: first.saturating_neg(),
        y: 0,
      }
    }
    None => return Err(DecodeError::InvalidAdaptivePixel),
  }

  let Ok(bits) = header.template.context_bits();
  let mut contexts = arithmetic::contexts(bits)?;
  let Ok(mut decoder) = Arithmetic::new(header.data);
  let settings = Settings {
    template: header.template,
    adaptive,
    prediction: Prediction::Plain,
    skip: None,
  };

  generic::decode(collective, &mut decoder, &mut contexts, &settings)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skipping {
  Enabled,
  Disabled,
}

#[derive(Debug, Clone)]
pub(crate) struct Header<'a> {
  pub(crate) info: RegionInfo,
  coding: Coding,
  template: Template,
  skipping: Skipping,
  operator: Operator,
  initial: Color,
  grid: Size<u32>,
  offset: Point<i32>,
  vector: Point<u16>,
  data: &'a [u8],
}

pub(crate) fn parse<'a>(reader: &mut Reader<'a>) -> Result<Header<'a>, DecodeError> {
  let info = RegionInfo::parse(reader)?;
  let flags = reader.byte()?;
  let coding = match flags & 1 {
    0 => Coding::Arithmetic,
    _ => Coding::Mmr,
  };
  let Ok(template) = Template::from_bits(flags.wrapping_shr(1));
  let skipping = match flags & 0x08 {
    0 => Skipping::Disabled,
    _ => Skipping::Enabled,
  };
  let operator = Operator::from_bits(flags.wrapping_shr(4))?;
  let initial = match flags & 0x80 {
    0 => Color::White,
    _ => Color::Black,
  };
  let width = reader.u32()?;
  let height = reader.u32()?;
  let area = u64::from(width).saturating_mul(u64::from(height));

  match (
    width > MAX_DIMENSION,
    height > MAX_DIMENSION,
    area > MAX_PIXELS,
  ) {
    (false, false, false) => {}
    (true, _, _) | (_, true, _) | (_, _, true) => return Err(DecodeError::Overflow),
  }

  let across = reader.i32()?;
  let down = reader.i32()?;
  let x = reader.u16()?;
  let y = reader.u16()?;
  let data = reader.tail()?;

  Ok(Header {
    info,
    coding,
    template,
    skipping,
    operator,
    initial,
    grid: Size { width, height },
    offset: Point { x: across, y: down },
    vector: Point { x, y },
    data,
  })
}

struct Grid {
  row: Point<i64>,
  at: Point<i64>,
  vector: Point<i64>,
}

impl Grid {
  fn new(offset: Point<i32>, vector: Point<u16>) -> Result<Grid, Never> {
    let start = Point {
      x: i64::from(offset.x),
      y: i64::from(offset.y),
    };

    Ok(Grid {
      row: start,
      at: start,
      vector: Point {
        x: i64::from(vector.x),
        y: i64::from(vector.y),
      },
    })
  }

  fn position(&self) -> Result<Point<i32>, Never> {
    let Ok(x) = fitted::<_, i32>(self.at.x.div_euclid(256));
    let Ok(y) = fitted::<_, i32>(self.at.y.div_euclid(256));

    Ok(Point { x, y })
  }

  fn next_column(&mut self) -> Result<(), Never> {
    self.at = Point {
      x: self.at.x.saturating_add(self.vector.x),
      y: self.at.y.saturating_sub(self.vector.y),
    };

    Ok(())
  }

  fn next_row(&mut self) -> Result<(), Never> {
    self.row = Point {
      x: self.row.x.saturating_add(self.vector.y),
      y: self.row.y.saturating_add(self.vector.x),
    };
    self.at = self.row;

    Ok(())
  }
}

fn skipped(
  header: &Header<'_>,
  patterns: &Patterns,
  region: Size<u32>,
) -> Result<Bitmap, DecodeError> {
  let mut mask = Bitmap::white(header.grid)?;
  let Ok(mut grid) = Grid::new(header.offset, header.vector);
  let (width, height) = (
    i64::from(patterns.size.width),
    i64::from(patterns.size.height),
  );

  for row in 0..header.grid.height {
    for column in 0..header.grid.width {
      let Ok(at) = grid.position();
      let (x, y) = (i64::from(at.x), i64::from(at.y));
      let outside = (
        x.saturating_add(width) <= 0,
        x >= i64::from(region.width),
        y.saturating_add(height) <= 0,
        y >= i64::from(region.height),
      );

      match outside {
        (false, false, false, false) => {}
        (true, _, _, _) | (_, true, _, _) | (_, _, true, _) | (_, _, _, true) => {
          let Ok(()) = mask.put(Point { x: column, y: row }, 1);
        }
      }

      let Ok(()) = grid.next_column();
    }

    let Ok(()) = grid.next_row();
  }

  Ok(mask)
}

fn bits_per_pixel(patterns: &Patterns) -> Result<u32, Never> {
  let Ok(count) = fitted::<_, u32>(patterns.bitmaps.len());

  Ok(match count.saturating_sub(1).checked_ilog2() {
    Some(log) => log.saturating_add(1),
    None => 1,
  })
}

fn gray_scale(
  header: &Header<'_>,
  patterns: &Patterns,
  mask: Option<&Bitmap>,
) -> Result<Vec<u32>, DecodeError> {
  let Ok(planes) = bits_per_pixel(patterns);
  let size = header.grid;
  let Ok(count) = index(u64::from(size.width).saturating_mul(u64::from(size.height)));
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the halftone header parser rejects grids whose dimensions or total cell count exceed MAX_DIMENSION or MAX_PIXELS"
    )
  )]
  let mut values = vec![0u32; count];
  let Ok(bits) = header.template.context_bits();
  let mut contexts = arithmetic::contexts(bits)?;
  let Ok(mut decoder) = Arithmetic::new(header.data);
  let Ok(adaptive) = header.template.usual_pixels();
  let settings = Settings {
    template: header.template,
    adaptive,
    prediction: Prediction::Plain,
    skip: mask,
  };
  let mut previous: Option<Bitmap> = None;
  let mut consumed = 0u32;

  for plane_bit in (0..planes).rev() {
    let mut plane = Bitmap::white(size)?;

    match header.coding {
      Coding::Mmr => {
        let Ok(from) = index(consumed);
        let rest = header.data.get(from..).ok_or(DecodeError::EndOfData)?;

        let used = mmr::decode(&mut plane, rest)?;

        consumed = consumed.saturating_add(used);
      }
      Coding::Arithmetic => generic::decode(&mut plane, &mut decoder, &mut contexts, &settings)?,
    }

    match &previous {
      Some(above) => {
        let Ok(()) = plane.combine(above, Point { x: 0, y: 0 }, Operator::Xor);
      }
      None => {}
    }

    let Ok(planes_2) = plane.pixels();

    for (value, pixel) in values.iter_mut().zip(planes_2) {
      *value |= u32::from(*pixel).wrapping_shl(plane_bit);
    }

    previous = Some(plane);
  }

  Ok(values)
}

pub(crate) fn decode_region(
  header: &Header<'_>,
  patterns: &Patterns,
) -> Result<Bitmap, DecodeError> {
  let mut region = Bitmap::new(header.info.size, header.info.origin, header.initial)?;
  let mask = match header.skipping {
    Skipping::Enabled => {
      let mask = skipped(header, patterns, header.info.size)?;

      Some(mask)
    }
    Skipping::Disabled => None,
  };
  let values = gray_scale(header, patterns, mask.as_ref())?;
  let Ok(mut grid) = Grid::new(header.offset, header.vector);
  let Ok(width) = index(header.grid.width);

  for row in values.chunks(width.max(1)) {
    for value in row {
      let Ok(at) = grid.position();
      let Ok(which) = index(*value);
      let pattern = patterns
        .bitmaps
        .get(which)
        .ok_or(DecodeError::GrayScaleOutOfRange)?;
      let Ok(()) = region.combine(pattern, at, header.operator);
      let Ok(()) = grid.next_column();
    }

    let Ok(()) = grid.next_row();
  }

  Ok(region)
}
