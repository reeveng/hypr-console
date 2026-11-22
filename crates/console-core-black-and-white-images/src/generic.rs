//! Generic regions: a bitmap decoded pixel by pixel from the ones around it, or in fax's coding.

use console_core_fax_images::Color;
use console_core_geometry::Point;
use console_core_mq_decoding::{Context, Decoder as Arithmetic};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::arithmetic;
use crate::bitmap::Bitmap;
use crate::error::DecodeError;
use crate::neighbourhood::{Neighbourhood, Source, Tap};
use crate::reading::Reader;
use crate::regions::RegionInfo;
use crate::segments::Length;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Template {
  Zero,
  One,
  Two,
  Three,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prediction {
  Typical,
  Plain,
}

#[derive(Debug, Clone, Copy)]
enum Slot {
  Fixed(i8, i8),
  Adaptive(u8),
}

const ZERO: [Slot; 16] = [
  Slot::Fixed(-1, 0),
  Slot::Fixed(-2, 0),
  Slot::Fixed(-3, 0),
  Slot::Fixed(-4, 0),
  Slot::Adaptive(0),
  Slot::Fixed(2, -1),
  Slot::Fixed(1, -1),
  Slot::Fixed(0, -1),
  Slot::Fixed(-1, -1),
  Slot::Fixed(-2, -1),
  Slot::Adaptive(1),
  Slot::Adaptive(2),
  Slot::Fixed(1, -2),
  Slot::Fixed(0, -2),
  Slot::Fixed(-1, -2),
  Slot::Adaptive(3),
];

const ONE: [Slot; 13] = [
  Slot::Fixed(-1, 0),
  Slot::Fixed(-2, 0),
  Slot::Fixed(-3, 0),
  Slot::Adaptive(0),
  Slot::Fixed(2, -1),
  Slot::Fixed(1, -1),
  Slot::Fixed(0, -1),
  Slot::Fixed(-1, -1),
  Slot::Fixed(-2, -1),
  Slot::Fixed(2, -2),
  Slot::Fixed(1, -2),
  Slot::Fixed(0, -2),
  Slot::Fixed(-1, -2),
];

const TWO: [Slot; 10] = [
  Slot::Fixed(-1, 0),
  Slot::Fixed(-2, 0),
  Slot::Adaptive(0),
  Slot::Fixed(1, -1),
  Slot::Fixed(0, -1),
  Slot::Fixed(-1, -1),
  Slot::Fixed(-2, -1),
  Slot::Fixed(1, -2),
  Slot::Fixed(0, -2),
  Slot::Fixed(-1, -2),
];

const THREE: [Slot; 10] = [
  Slot::Fixed(-1, 0),
  Slot::Fixed(-2, 0),
  Slot::Fixed(-3, 0),
  Slot::Fixed(-4, 0),
  Slot::Adaptive(0),
  Slot::Fixed(1, -1),
  Slot::Fixed(0, -1),
  Slot::Fixed(-1, -1),
  Slot::Fixed(-2, -1),
  Slot::Fixed(-3, -1),
];

impl Template {
  pub(crate) fn from_bits(value: u8) -> Result<Template, Never> {
    Ok(match value & 0x03 {
      0 => Template::Zero,
      1 => Template::One,
      2 => Template::Two,
      _ => Template::Three,
    })
  }

  pub(crate) fn context_bits(self) -> Result<u32, Never> {
    Ok(match self {
      Template::Zero => 16,
      Template::One => 13,
      Template::Two | Template::Three => 10,
    })
  }

  pub(crate) fn adaptive_count(self) -> Result<u32, Never> {
    Ok(match self {
      Template::Zero => 4,
      Template::One | Template::Two | Template::Three => 1,
    })
  }

  pub(crate) fn usual_pixels(self) -> Result<[Point<i8>; 4], Never> {
    let unused = Point { x: 0, y: 0 };

    Ok(match self {
      Template::Zero => [
        Point { x: 3, y: -1 },
        Point { x: -3, y: -1 },
        Point { x: 2, y: -2 },
        Point { x: -2, y: -2 },
      ],
      Template::One => [Point { x: 3, y: -1 }, unused, unused, unused],
      Template::Two | Template::Three => [Point { x: 2, y: -1 }, unused, unused, unused],
    })
  }

  fn slots(self) -> Result<&'static [Slot], Never> {
    Ok(match self {
      Template::Zero => &ZERO,
      Template::One => &ONE,
      Template::Two => &TWO,
      Template::Three => &THREE,
    })
  }

  fn typical_context(self) -> Result<u32, Never> {
    Ok(match self {
      Template::Zero => 0b1001_1011_0010_0101,
      Template::One => 0b0_0111_1001_0101,
      Template::Two => 0b00_1110_0101,
      Template::Three => 0b01_1001_0101,
    })
  }

  pub(crate) fn neighbourhood(
    self,
    adaptive: &[Point<i8>; 4],
  ) -> Result<Neighbourhood, DecodeError> {
    let Ok(slots) = self.slots();
    let mut taps = Vec::new();

    for slot in slots {
      let at = match slot {
        Slot::Fixed(x, y) => Point {
          x: i32::from(*x),
          y: i32::from(*y),
        },
        Slot::Adaptive(which) => {
          let Ok(which_2) = index(*which);
          let found = adaptive
            .get(which_2)
            .ok_or(DecodeError::InvalidAdaptivePixel)?;

          Point {
            x: i32::from(found.x),
            y: i32::from(found.y),
          }
        }
      };

      taps.push(Tap {
        source: Source::Decoded,
        at,
      });
    }

    let Ok(hood) = Neighbourhood::new(taps);

    Ok(hood)
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings<'a> {
  pub(crate) template: Template,
  pub(crate) adaptive: [Point<i8>; 4],
  pub(crate) prediction: Prediction,
  pub(crate) skip: Option<&'a Bitmap>,
}

pub(crate) fn decode(
  bitmap: &mut Bitmap,
  arithmetic: &mut Arithmetic<'_>,
  contexts: &mut [Context],
  settings: &Settings<'_>,
) -> Result<(), DecodeError> {
  let hood = settings.template.neighbourhood(&settings.adaptive)?;
  let Ok(typical) = settings.template.typical_context();
  let mut repeated = 0u32;

  for y in 0..bitmap.size.height {
    repeated = match settings.prediction {
      Prediction::Typical => {
        let flip = arithmetic::bit(arithmetic, contexts, typical)?;

        repeated ^ flip
      }
      Prediction::Plain => repeated,
    };

    match repeated {
      0 => decode_row(bitmap, arithmetic, contexts, (&hood, settings), y)?,
      _ => {
        let Ok(()) = bitmap.repeat_above(y);
      }
    }
  }

  Ok(())
}

fn decode_row(
  bitmap: &mut Bitmap,
  arithmetic: &mut Arithmetic<'_>,
  contexts: &mut [Context],
  with: (&Neighbourhood, &Settings<'_>),
  y: u32,
) -> Result<(), DecodeError> {
  let (hood, settings) = with;
  let Ok(row) = fitted::<_, i32>(y);
  let Ok(mut context) = hood.first(Point { x: 0, y: row }, bitmap, None);

  for x in 0..bitmap.size.width {
    let Ok(column) = fitted::<_, i32>(x);
    let at = Point { x: column, y: row };

    context = match x {
      0 => context,
      _ => {
        let Ok(next) = hood.next(context, at, bitmap, None);

        next
      }
    };

    let skipped = match settings.skip {
      Some(mask) => {
        let Ok(pixel) = mask.get(at);

        pixel
      }
      None => 0,
    };

    match skipped {
      0 => {
        let pixel = arithmetic::bit(arithmetic, contexts, context)?;
        let Ok(()) = bitmap.put(Point { x, y }, pixel);
      }
      _ => {}
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Coding {
  Mmr,
  Arithmetic,
}

#[derive(Debug, Clone)]
pub(crate) struct Header<'a> {
  pub(crate) info: RegionInfo,
  pub(crate) coding: Coding,
  pub(crate) settings: Settings<'static>,
  pub(crate) data: &'a [u8],
}

pub(crate) fn parse_adaptive(
  reader: &mut Reader<'_>,
  count: u32,
) -> Result<[Point<i8>; 4], DecodeError> {
  let mut pixels = [Point { x: 0i8, y: 0i8 }; 4];

  let Ok(taken) = index(count);

  for pixel in pixels.iter_mut().take(taken) {
    let x = reader.signed_byte()?;
    let y = reader.signed_byte()?;

    match (y > 0, y == 0 && x >= 0) {
      (false, false) => *pixel = Point { x, y },
      (true, _) | (_, true) => return Err(DecodeError::InvalidAdaptivePixel),
    }
  }

  Ok(pixels)
}

pub(crate) fn parse<'a>(
  reader: &mut Reader<'a>,
  length: Length,
) -> Result<Header<'a>, DecodeError> {
  let mut info = RegionInfo::parse(reader)?;
  let flags = reader.byte()?;
  let coding = match flags & 1 {
    0 => Coding::Arithmetic,
    _ => Coding::Mmr,
  };
  let Ok(template) = Template::from_bits(flags.wrapping_shr(1));
  let prediction = match flags & 0x08 {
    0 => Prediction::Plain,
    _ => Prediction::Typical,
  };
  let adaptive = match coding {
    Coding::Mmr => [Point { x: 0, y: 0 }; 4],
    Coding::Arithmetic => {
      let Ok(count) = template.adaptive_count();

      parse_adaptive(reader, count)?
    }
  };
  let mut data = reader.tail()?;

  match length {
    Length::Known(_) => {}
    Length::Unknown => {
      let (head, tail) = data.split_last_chunk::<4>().ok_or(DecodeError::EndOfData)?;
      let rows = u32::from_be_bytes(*tail);

      match ::core::primitive::bool::then_some(rows > info.size.height, ()) {
        ::core::option::Option::Some(()) => return Err(DecodeError::InvalidDimension),
        ::core::option::Option::None => info.size.height = rows,
      }

      data = head;
    }
  }

  Ok(Header {
    info,
    coding,
    settings: Settings {
      template,
      adaptive,
      prediction,
      skip: None,
    },
    data,
  })
}

pub(crate) fn decode_region(header: &Header<'_>) -> Result<Bitmap, DecodeError> {
  let mut bitmap = Bitmap::new(header.info.size, header.info.origin, Color::White)?;

  match header.coding {
    Coding::Mmr => {
      let _read = crate::mmr::decode(&mut bitmap, header.data)?;
    }
    Coding::Arithmetic => {
      let Ok(bits) = header.settings.template.context_bits();
      let mut contexts = arithmetic::contexts(bits)?;
      let Ok(mut decoder) = Arithmetic::new(header.data);

      decode(&mut bitmap, &mut decoder, &mut contexts, &header.settings)?;
    }
  }

  Ok(bitmap)
}
