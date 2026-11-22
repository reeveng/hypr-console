//! Generic refinement regions: a bitmap decoded against a reference one.

use console_core_fax_images::Color;
use console_core_geometry::Point;
use console_core_mq_decoding::{Context, Decoder as Arithmetic};
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::arithmetic;
use crate::bitmap::Bitmap;
use crate::error::DecodeError;
use crate::generic::Prediction;
use crate::neighbourhood::{Neighbourhood, Reference, Source, Tap};
use crate::reading::Reader;
use crate::regions::RegionInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Template {
  Zero,
  One,
}

#[derive(Debug, Clone, Copy)]
enum Slot {
  Decoded(i8, i8),
  Reference(i8, i8),
  DecodedAdaptive,
  ReferenceAdaptive,
}

const ZERO: [Slot; 13] = [
  Slot::Reference(1, 1),
  Slot::Reference(0, 1),
  Slot::Reference(-1, 1),
  Slot::Reference(1, 0),
  Slot::Reference(0, 0),
  Slot::Reference(-1, 0),
  Slot::Reference(1, -1),
  Slot::Reference(0, -1),
  Slot::ReferenceAdaptive,
  Slot::Decoded(-1, 0),
  Slot::Decoded(1, -1),
  Slot::Decoded(0, -1),
  Slot::DecodedAdaptive,
];

const ONE: [Slot; 10] = [
  Slot::Reference(1, 1),
  Slot::Reference(0, 1),
  Slot::Reference(1, 0),
  Slot::Reference(0, 0),
  Slot::Reference(-1, 0),
  Slot::Reference(0, -1),
  Slot::Decoded(-1, 0),
  Slot::Decoded(1, -1),
  Slot::Decoded(0, -1),
  Slot::Decoded(-1, -1),
];

impl Template {
  pub(crate) fn from_bits(value: u8) -> Result<Template, Never> {
    Ok(match value & 1 {
      0 => Template::Zero,
      _ => Template::One,
    })
  }

  pub(crate) fn context_bits(self) -> Result<u32, Never> {
    Ok(match self {
      Template::Zero => 13,
      Template::One => 10,
    })
  }

  fn typical_context(self) -> Result<u32, Never> {
    Ok(match self {
      Template::Zero => 0b0_0000_0001_0000,
      Template::One => 0b00_0000_1000,
    })
  }

  fn neighbourhood(self, adaptive: &[Point<i8>; 2]) -> Result<Neighbourhood, Never> {
    let slots: &[Slot] = match self {
      Template::Zero => &ZERO,
      Template::One => &ONE,
    };
    let [decoded, reference] = *adaptive;
    let taps = slots
      .iter()
      .map(|slot| {
        let (source, at) = match slot {
          Slot::Decoded(x, y) => (Source::Decoded, Point { x: *x, y: *y }),
          Slot::Reference(x, y) => (Source::Reference, Point { x: *x, y: *y }),
          Slot::DecodedAdaptive => (Source::Decoded, decoded),
          Slot::ReferenceAdaptive => (Source::Reference, reference),
        };
        let Ok(made) = tap(source, at);

        made
      })
      .collect();

    Neighbourhood::new(taps)
  }
}

fn tap(source: Source, at: Point<i8>) -> Result<Tap, Never> {
  Ok(Tap {
    source,
    at: Point {
      x: i32::from(at.x),
      y: i32::from(at.y),
    },
  })
}

#[derive(Debug, Clone)]
pub(crate) struct Header<'a> {
  pub(crate) info: RegionInfo,
  pub(crate) template: Template,
  pub(crate) prediction: Prediction,
  pub(crate) adaptive: [Point<i8>; 2],
  pub(crate) data: &'a [u8],
}

pub(crate) fn parse_adaptive(reader: &mut Reader<'_>) -> Result<[Point<i8>; 2], DecodeError> {
  let x1 = reader.signed_byte()?;
  let y1 = reader.signed_byte()?;
  let x2 = reader.signed_byte()?;
  let y2 = reader.signed_byte()?;

  Ok([Point { x: x1, y: y1 }, Point { x: x2, y: y2 }])
}

pub(crate) fn parse<'a>(reader: &mut Reader<'a>) -> Result<Header<'a>, DecodeError> {
  let info = RegionInfo::parse(reader)?;
  let flags = reader.byte()?;
  let Ok(template) = Template::from_bits(flags);
  let prediction = match flags & 0x02 {
    0 => Prediction::Plain,
    _ => Prediction::Typical,
  };
  let adaptive = match template {
    Template::Zero => parse_adaptive(reader)?,
    Template::One => [Point { x: 0, y: 0 }; 2],
  };
  let data = reader.tail()?;

  Ok(Header {
    info,
    template,
    prediction,
    adaptive,
    data,
  })
}

pub(crate) fn decode_region(
  header: &Header<'_>,
  reference: &Bitmap,
) -> Result<Bitmap, DecodeError> {
  let info = &header.info;

  match (
    info.size.width > reference.size.width,
    info.size.height > reference.size.height,
  ) {
    (false, false) => {}
    (true, _) | (_, true) => return Err(DecodeError::InvalidDimension),
  }

  let shift = reference.shift_to(info.origin)?;
  let mut bitmap = Bitmap::new(info.size, info.origin, Color::White)?;
  let Ok(bits) = header.template.context_bits();
  let mut contexts = arithmetic::contexts(bits)?;
  let Ok(mut decoder) = Arithmetic::new(header.data);
  let against = Reference {
    bitmap: reference,
    shift,
  };
  let settings = Settings {
    template: header.template,
    adaptive: header.adaptive,
    prediction: header.prediction,
  };

  decode(
    &mut bitmap,
    &mut decoder,
    &mut contexts,
    &against,
    &settings,
  )?;

  Ok(bitmap)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings {
  pub(crate) template: Template,
  pub(crate) adaptive: [Point<i8>; 2],
  pub(crate) prediction: Prediction,
}

pub(crate) fn decode(
  bitmap: &mut Bitmap,
  arithmetic: &mut Arithmetic<'_>,
  contexts: &mut [Context],
  reference: &Reference<'_>,
  settings: &Settings,
) -> Result<(), DecodeError> {
  let Ok(hood) = settings.template.neighbourhood(&settings.adaptive);
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

    decode_row(
      bitmap,
      arithmetic,
      contexts,
      (&hood, reference),
      (y, repeated),
    )?;
  }

  Ok(())
}

fn decode_row(
  bitmap: &mut Bitmap,
  arithmetic: &mut Arithmetic<'_>,
  contexts: &mut [Context],
  with: (&Neighbourhood, &Reference<'_>),
  row: (u32, u32),
) -> Result<(), DecodeError> {
  let (hood, reference) = with;
  let (y, repeated) = row;
  let Ok(line) = fitted::<_, i32>(y);
  let Ok(mut context) = hood.first(Point { x: 0, y: line }, bitmap, Some(reference));

  for x in 0..bitmap.size.width {
    let Ok(column) = fitted::<_, i32>(x);
    let at = Point { x: column, y: line };

    context = match x {
      0 => context,
      _ => {
        let Ok(next) = hood.next(context, at, bitmap, Some(reference));

        next
      }
    };

    let Ok(same) = surroundings(reference, at);
    let pixel = match (repeated, same) {
      (1, Some(value)) => value,
      (_, _) => arithmetic::bit(arithmetic, contexts, context)?,
    };
    let Ok(()) = bitmap.put(Point { x, y }, pixel);
  }

  Ok(())
}

fn surroundings(reference: &Reference<'_>, at: Point<i32>) -> Result<Option<u32>, Never> {
  let centre = Point {
    x: at.x.saturating_sub(reference.shift.x),
    y: at.y.saturating_sub(reference.shift.y),
  };
  let Ok(middle) = reference.bitmap.get(centre);
  let mut agreeing = true;

  for across in -1i32..=1 {
    for down in -1i32..=1 {
      let place = Point {
        x: centre.x.saturating_add(across),
        y: centre.y.saturating_add(down),
      };
      let Ok(pixel) = reference.bitmap.get(place);

      agreeing = agreeing && pixel == middle;
    }
  }

  Ok(match ::core::primitive::bool::then_some(agreeing, ()) {
    ::core::option::Option::Some(()) => Some(u32::from(middle)),
    ::core::option::Option::None => None,
  })
}
