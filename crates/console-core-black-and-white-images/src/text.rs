//! Text regions: symbols from dictionaries set down along strips, each one possibly refined as it goes.

use console_core_fax_images::Color;
use console_core_geometry::{Point, Size};
use console_core_mq_decoding::{Context, Decoder as Arithmetic};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::arithmetic;
use crate::bitmap::{Bitmap, Operator};
use crate::error::DecodeError;
use crate::huffman::{HuffmanTable, symbol_table};
use crate::integers::{Identifier, Integer};
use crate::neighbourhood::Reference;
use crate::reading::Reader;
use crate::refinement;
use crate::regions::RegionInfo;
use crate::standard::Standard;

const MAX_INSTANCES: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Corner {
  BottomLeft,
  TopLeft,
  BottomRight,
  TopRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
  Upright,
  Transposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refining {
  Allowed,
  Forbidden,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings {
  pub(crate) refining: Refining,
  pub(crate) log_strip_size: u8,
  pub(crate) corner: Corner,
  pub(crate) orientation: Orientation,
  pub(crate) operator: Operator,
  pub(crate) default_pixel: Color,
  pub(crate) delta_s_offset: i8,
  pub(crate) refinement: refinement::Settings,
  pub(crate) instances: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selection {
  Standard(Standard),
  Custom,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Selections {
  pub(crate) first_s: Selection,
  pub(crate) delta_s: Selection,
  pub(crate) delta_t: Selection,
  pub(crate) width: Selection,
  pub(crate) height: Selection,
  pub(crate) y: Selection,
  pub(crate) x: Selection,
  pub(crate) size: Selection,
}

#[derive(Debug, Clone)]
pub(crate) struct Tables {
  first_s: HuffmanTable,
  delta_s: HuffmanTable,
  delta_t: HuffmanTable,
  width: HuffmanTable,
  height: HuffmanTable,
  y: HuffmanTable,
  x: HuffmanTable,
  size: HuffmanTable,
}

#[derive(Debug)]
pub(crate) struct Integers {
  strip: Integer,
  first: Integer,
  delta: Integer,
  within: Integer,
  pub(crate) identifier: Identifier,
  flag: Integer,
  width: Integer,
  height: Integer,
  pub(crate) x: Integer,
  pub(crate) y: Integer,
}

impl Integers {
  pub(crate) fn new(code_length: u32) -> Result<Integers, DecodeError> {
    let strip = Integer::new()?;
    let first = Integer::new()?;
    let delta = Integer::new()?;
    let within = Integer::new()?;
    let identifier = Identifier::new(code_length)?;
    let flag = Integer::new()?;
    let width = Integer::new()?;
    let height = Integer::new()?;
    let x = Integer::new()?;
    let y = Integer::new()?;

    Ok(Integers {
      strip,
      first,
      delta,
      within,
      identifier,
      flag,
      width,
      height,
      x,
      y,
    })
  }
}

pub(crate) enum Source<'a, 'b> {
  Huffman {
    reader: &'a mut Reader<'b>,
    tables: Tables,
    codes: &'a HuffmanTable,
    contexts: &'a mut [Context],
  },
  Arithmetic {
    decoder: &'a mut Arithmetic<'b>,
    integers: &'a mut Integers,
    contexts: &'a mut [Context],
  },
}

#[derive(Debug, Clone, Copy)]
struct Offsets {
  width: i32,
  height: i32,
  x: i32,
  y: i32,
}

fn in_band(decoded: Result<Option<i32>, DecodeError>) -> Result<i32, DecodeError> {
  match decoded {
    Ok(Some(value)) => Ok(value),
    Ok(None) => Err(DecodeError::UnexpectedOutOfBand),
    Err(fault) => Err(fault),
  }
}

impl Source<'_, '_> {
  fn strip_delta(&mut self, strip: i32) -> Result<i32, DecodeError> {
    let value = match self {
      Source::Huffman { reader, tables, .. } => tables.delta_t.decode_in_band(reader),
      Source::Arithmetic {
        decoder, integers, ..
      } => in_band(integers.strip.decode(decoder)),
    };
    let value_2 = value?;

    value_2
      .checked_mul(strip)
      .ok_or(DecodeError::InvalidStripDelta)
  }

  fn first_s(&mut self) -> Result<i32, DecodeError> {
    match self {
      Source::Huffman { reader, tables, .. } => tables.first_s.decode_in_band(reader),
      Source::Arithmetic {
        decoder, integers, ..
      } => in_band(integers.first.decode(decoder)),
    }
  }

  fn delta_s(&mut self) -> Result<Option<i32>, DecodeError> {
    match self {
      Source::Huffman { reader, tables, .. } => tables.delta_s.decode(reader),
      Source::Arithmetic {
        decoder, integers, ..
      } => integers.delta.decode(decoder),
    }
  }

  fn symbol_t(&mut self, log_strip_size: u8) -> Result<i32, DecodeError> {
    match (log_strip_size, self) {
      (0, _) => Ok(0),
      (log, Source::Huffman { reader, .. }) => {
        let bits = reader.bits(u32::from(log))?;

        i32::try_from(bits).map_err(|_| DecodeError::Overflow)
      }
      (
        _,
        Source::Arithmetic {
          decoder, integers, ..
        },
      ) => in_band(integers.within.decode(decoder)),
    }
  }

  pub(crate) fn symbol_id(&mut self) -> Result<u32, DecodeError> {
    match self {
      Source::Huffman { reader, codes, .. } => {
        let id = codes.decode_in_band(reader)?;

        u32::try_from(id).map_err(|_| DecodeError::SymbolOutOfRange)
      }
      Source::Arithmetic {
        decoder, integers, ..
      } => integers.identifier.decode(decoder),
    }
  }

  fn refinement_flag(&mut self) -> Result<u32, DecodeError> {
    match self {
      Source::Huffman { reader, .. } => reader.bit(),
      Source::Arithmetic {
        decoder, integers, ..
      } => {
        let flag = in_band(integers.flag.decode(decoder))?;

        Ok(u32::from_be_bytes(flag.to_be_bytes()) & 0xFF)
      }
    }
  }

  fn offsets(&mut self) -> Result<Offsets, DecodeError> {
    match self {
      Source::Huffman { reader, tables, .. } => {
        let width = tables.width.decode_in_band(reader)?;
        let height = tables.height.decode_in_band(reader)?;
        let x = tables.x.decode_in_band(reader)?;
        let y = tables.y.decode_in_band(reader)?;

        Ok(Offsets {
          width,
          height,
          x,
          y,
        })
      }
      Source::Arithmetic {
        decoder, integers, ..
      } => {
        let width = in_band(integers.width.decode(decoder))?;
        let height = in_band(integers.height.decode(decoder))?;
        let x = in_band(integers.x.decode(decoder))?;
        let y = in_band(integers.y.decode(decoder))?;

        Ok(Offsets {
          width,
          height,
          x,
          y,
        })
      }
    }
  }

  pub(crate) fn refine(
    &mut self,
    refined: &mut Bitmap,
    against: &Reference<'_>,
    settings: &refinement::Settings,
  ) -> Result<(), DecodeError> {
    match self {
      Source::Huffman {
        reader,
        tables,
        contexts,
        ..
      } => {
        let size = tables.size.decode_in_band(reader)?;
        let size_2 = u32::try_from(size).map_err(|_| DecodeError::Overflow)?;
        let Ok(()) = reader.align();
        let data = reader.bytes(size_2)?;
        let Ok(mut decoder) = Arithmetic::new(data);

        refinement::decode(refined, &mut decoder, contexts, against, settings)
      }
      Source::Arithmetic {
        decoder, contexts, ..
      } => refinement::decode(refined, decoder, contexts, against, settings),
    }
  }
}

pub(crate) fn pick(
  selection: Selection,
  custom: &mut std::slice::Iter<'_, &HuffmanTable>,
) -> Result<HuffmanTable, DecodeError> {
  match selection {
    Selection::Standard(which) => HuffmanTable::standard(which),
    Selection::Custom => match custom.next() {
      Some(table) => Ok((*table).clone()),
      None => Err(DecodeError::MissingTables),
    },
  }
}

impl Selections {
  pub(crate) fn resolve(&self, custom: &[&HuffmanTable]) -> Result<Tables, DecodeError> {
    let mut next = custom.iter();

    let first_s = pick(self.first_s, &mut next)?;
    let delta_s = pick(self.delta_s, &mut next)?;
    let delta_t = pick(self.delta_t, &mut next)?;
    let width = pick(self.width, &mut next)?;
    let height = pick(self.height, &mut next)?;
    let y = pick(self.y, &mut next)?;
    let x = pick(self.x, &mut next)?;
    let size = pick(self.size, &mut next)?;

    Ok(Tables {
      first_s,
      delta_s,
      delta_t,
      width,
      height,
      y,
      x,
      size,
    })
  }

  pub(crate) fn aggregating() -> Result<Selections, Never> {
    Ok(Selections {
      first_s: Selection::Standard(Standard::B6),
      delta_s: Selection::Standard(Standard::B8),
      delta_t: Selection::Standard(Standard::B11),
      width: Selection::Standard(Standard::B15),
      height: Selection::Standard(Standard::B15),
      y: Selection::Standard(Standard::B15),
      x: Selection::Standard(Standard::B15),
      size: Selection::Standard(Standard::B1),
    })
  }

  fn parse(reader: &mut Reader<'_>) -> Result<Selections, DecodeError> {
    let word = reader.u16()?;
    let field = |shift: u32| u16::wrapping_shr(word, shift) & 0x03;
    let refinement = |shift: u32| match field(shift) {
      0 => Ok(Selection::Standard(Standard::B14)),
      1 => Ok(Selection::Standard(Standard::B15)),
      3 => Ok(Selection::Custom),
      _ => Err(DecodeError::InvalidTableSelection),
    };

    let width = refinement(6)?;
    let height = refinement(8)?;
    let y = refinement(10)?;
    let x = refinement(12)?;

    Ok(Selections {
      first_s: match field(0) {
        0 => Selection::Standard(Standard::B6),
        1 => Selection::Standard(Standard::B7),
        3 => Selection::Custom,
        _ => return Err(DecodeError::InvalidTableSelection),
      },
      delta_s: match field(2) {
        0 => Selection::Standard(Standard::B8),
        1 => Selection::Standard(Standard::B9),
        2 => Selection::Standard(Standard::B10),
        _ => Selection::Custom,
      },
      delta_t: match field(4) {
        0 => Selection::Standard(Standard::B11),
        1 => Selection::Standard(Standard::B12),
        2 => Selection::Standard(Standard::B13),
        _ => Selection::Custom,
      },
      width,
      height,
      y,
      x,
      size: match word.wrapping_shr(14) & 1 {
        0 => Selection::Standard(Standard::B1),
        _ => Selection::Custom,
      },
    })
  }
}

#[derive(Debug, Clone, Copy)]
struct Strip {
  t: i32,
  first_s: i32,
}

enum Instance<'a> {
  Shared(&'a Bitmap),
  Refined(Bitmap),
}

fn resized(held: u32, delta: i32) -> Result<u32, DecodeError> {
  let held_2 = i32::try_from(held).map_err(|_| DecodeError::Overflow)?;
  let sum = held_2.checked_add(delta).ok_or(DecodeError::Overflow)?;

  u32::try_from(sum).map_err(|_| DecodeError::Overflow)
}

fn instance<'a>(
  source: &mut Source<'_, '_>,
  symbols: &[&'a Bitmap],
  settings: &Settings,
  id: u32,
) -> Result<Instance<'a>, DecodeError> {
  let Ok(at) = index(id);
  let found = symbols.get(at).ok_or(DecodeError::SymbolOutOfRange)?;
  let reference = *found;
  let refined = match settings.refining {
    Refining::Allowed => source.refinement_flag()?,
    Refining::Forbidden => 0,
  };

  match refined {
    0 => Ok(Instance::Shared(reference)),
    _ => {
      let offsets = source.offsets()?;
      let width = resized(reference.size.width, offsets.width)?;
      let height = resized(reference.size.height, offsets.height)?;
      let shift_x = offsets
        .width
        .div_euclid(2)
        .checked_add(offsets.x)
        .ok_or(DecodeError::Overflow)?;
      let shift_y = offsets
        .height
        .div_euclid(2)
        .checked_add(offsets.y)
        .ok_or(DecodeError::Overflow)?;
      let size = Size { width, height };
      let shift = Point {
        x: shift_x,
        y: shift_y,
      };
      let mut bitmap = Bitmap::white(size)?;
      let against = Reference {
        bitmap: reference,
        shift,
      };

      source.refine(&mut bitmap, &against, &settings.refinement)?;

      Ok(Instance::Refined(bitmap))
    }
  }
}

fn corners(settings: &Settings, size: Size<i32>) -> Result<(i32, i32), Never> {
  let wide = size.width.saturating_sub(1);
  let tall = size.height.saturating_sub(1);

  Ok(match (settings.orientation, settings.corner) {
    (Orientation::Upright, Corner::TopRight | Corner::BottomRight) => (wide, 0),
    (Orientation::Upright, Corner::TopLeft | Corner::BottomLeft) => (0, wide),
    (Orientation::Transposed, Corner::BottomLeft | Corner::BottomRight) => (tall, 0),
    (Orientation::Transposed, Corner::TopLeft | Corner::TopRight) => (0, tall),
  })
}

fn placed(
  settings: &Settings,
  size: Size<i32>,
  along_strip: Point<i32>,
) -> Result<Point<i32>, Never> {
  let wide = size.width.saturating_sub(1);
  let tall = size.height.saturating_sub(1);
  let (along, across) = match settings.orientation {
    Orientation::Upright => (along_strip.x, along_strip.y),
    Orientation::Transposed => (along_strip.y, along_strip.x),
  };

  Ok(match (settings.orientation, settings.corner) {
    (_, Corner::TopLeft) => Point {
      x: along,
      y: across,
    },
    (_, Corner::TopRight) => Point {
      x: along.saturating_sub(wide),
      y: across,
    },
    (_, Corner::BottomLeft) => Point {
      x: along,
      y: across.saturating_sub(tall),
    },
    (_, Corner::BottomRight) => Point {
      x: along.saturating_sub(wide),
      y: across.saturating_sub(tall),
    },
  })
}

fn place(
  source: &mut Source<'_, '_>,
  symbols: &[&Bitmap],
  settings: &Settings,
  region: &mut Bitmap,
  at: (i32, &mut i32),
) -> Result<(), DecodeError> {
  let (strip_t, current_s) = at;
  let within = source.symbol_t(settings.log_strip_size)?;
  let symbol_t = strip_t.checked_add(within).ok_or(DecodeError::Overflow)?;
  let id = source.symbol_id()?;
  let chosen = instance(source, symbols, settings, id)?;
  let bitmap = match &chosen {
    Instance::Shared(shared) => shared,
    Instance::Refined(refined) => refined,
  };
  let width = i32::try_from(bitmap.size.width).map_err(|_| DecodeError::Overflow)?;
  let height = i32::try_from(bitmap.size.height).map_err(|_| DecodeError::Overflow)?;
  let size = Size { width, height };
  let Ok((before, after)) = corners(settings, size);
  let advanced = current_s.checked_add(before).ok_or(DecodeError::Overflow)?;

  *current_s = advanced;

  let Ok(at_2) = placed(
    settings,
    size,
    Point {
      x: *current_s,
      y: symbol_t,
    },
  );
  let Ok(()) = region.combine(bitmap, at_2, settings.operator);
  let finished = current_s.checked_add(after).ok_or(DecodeError::Overflow)?;

  *current_s = finished;

  Ok(())
}

fn strip(
  source: &mut Source<'_, '_>,
  symbols: &[&Bitmap],
  settings: &Settings,
  region: &mut Bitmap,
  state: (&mut Strip, i32, u32),
) -> Result<u32, DecodeError> {
  let (strip, size, mut count) = state;
  let delta = source.strip_delta(size)?;
  let offset = i32::from(settings.delta_s_offset);
  let mut current_s = 0i32;

  let top = strip.t.checked_add(delta).ok_or(DecodeError::Overflow)?;

  strip.t = top;

  for step in 0..=settings.instances.saturating_add(1) {
    match ::core::primitive::bool::then_some(count > settings.instances, ()) {
      ::core::option::Option::Some(()) => return Err(DecodeError::TooManySymbols),
      ::core::option::Option::None => {}
    }

    let next = match step {
      0 => {
        let delta_2 = source.first_s()?;

        let first = strip
          .first_s
          .checked_add(delta_2)
          .ok_or(DecodeError::Overflow)?;

        strip.first_s = first;

        Some(first)
      }
      _ => {
        let delta_3 = source.delta_s()?;

        match delta_3 {
          Some(delta_4) => {
            let moved = current_s
              .checked_add(delta_4)
              .and_then(|held| held.checked_add(offset))
              .ok_or(DecodeError::Overflow)?;

            Some(moved)
          }
          None => None,
        }
      }
    };

    current_s = match next {
      Some(next_2) => next_2,
      None => break,
    };

    place(source, symbols, settings, region, (strip.t, &mut current_s))?;

    count = count.saturating_add(1);
  }

  Ok(count)
}

pub(crate) fn decode_with(
  mut source: Source<'_, '_>,
  symbols: &[&Bitmap],
  settings: &Settings,
  region: &mut Bitmap,
) -> Result<(), DecodeError> {
  match ::core::primitive::bool::then_some(settings.instances > MAX_INSTANCES, ()) {
    ::core::option::Option::Some(()) => return Err(DecodeError::TooManyInstances),
    ::core::option::Option::None => {}
  }

  let size = 1i32
    .checked_shl(u32::from(settings.log_strip_size))
    .ok_or(DecodeError::Overflow)?;
  let first = source.strip_delta(size)?;
  let top = first.checked_neg().ok_or(DecodeError::Overflow)?;
  let mut state = Strip { t: top, first_s: 0 };
  let mut count = 0u32;

  for _strip in 0..settings.instances {
    match ::core::primitive::bool::then_some(count >= settings.instances, ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }

    let counted = strip(
      &mut source,
      symbols,
      settings,
      region,
      (&mut state, size, count),
    )?;

    count = counted;
  }

  Ok(())
}

#[derive(Debug, Clone)]
pub(crate) enum Coding {
  Huffman {
    selections: Selections,
    codes: HuffmanTable,
  },
  Arithmetic,
}

#[derive(Debug, Clone)]
pub(crate) struct Header<'a> {
  pub(crate) info: RegionInfo,
  settings: Settings,
  coding: Coding,
  data: &'a [u8],
}

fn corner_of(bits: u16) -> Result<Corner, Never> {
  Ok(match bits & 0x03 {
    0 => Corner::BottomLeft,
    1 => Corner::TopLeft,
    2 => Corner::BottomRight,
    _ => Corner::TopRight,
  })
}

fn offset_of(bits: u16) -> Result<i8, Never> {
  let Ok(raw) = fitted::<u16, u8>(bits & 0x1F);
  let extended = match raw & 0x10 {
    0 => raw,
    _ => raw | 0xE0,
  };

  Ok(i8::from_be_bytes([extended]))
}

fn parse_flags(reader: &mut Reader<'_>) -> Result<(Settings, bool), DecodeError> {
  let word = reader.u16()?;
  let huffman = word & 0x0001 != 0;
  let refining = match word & 0x0002 {
    0 => Refining::Forbidden,
    _ => Refining::Allowed,
  };
  let log_strip_size =
    u8::try_from(word.wrapping_shr(2) & 0x03).map_err(|_| DecodeError::Overflow)?;
  let Ok(corner) = corner_of(word.wrapping_shr(4));
  let orientation = match word & 0x0040 {
    0 => Orientation::Upright,
    _ => Orientation::Transposed,
  };
  let operator_bits =
    u8::try_from(word.wrapping_shr(7) & 0x03).map_err(|_| DecodeError::Overflow)?;
  let operator = Operator::from_bits(operator_bits)?;
  let default_pixel = match word & 0x0200 {
    0 => Color::White,
    _ => Color::Black,
  };
  let Ok(delta_s_offset) = offset_of(word.wrapping_shr(10));
  let template_bit = u8::try_from(word.wrapping_shr(15)).map_err(|_| DecodeError::Overflow)?;
  let Ok(template) = refinement::Template::from_bits(template_bit);
  let refinement = refinement::Settings {
    template,
    adaptive: [Point { x: 0, y: 0 }; 2],
    prediction: crate::generic::Prediction::Plain,
  };
  let settings = Settings {
    refining,
    log_strip_size,
    corner,
    orientation,
    operator,
    default_pixel,
    delta_s_offset,
    refinement,
    instances: 0,
  };

  Ok((settings, huffman))
}

pub(crate) fn parse<'a>(
  reader: &mut Reader<'a>,
  symbols: u32,
) -> Result<Header<'a>, DecodeError> {
  let info = RegionInfo::parse(reader)?;
  let (mut settings, huffman) = parse_flags(reader)?;
  let selections = match ::core::primitive::bool::then_some(huffman, ()) {
    ::core::option::Option::Some(()) => {
      let parsed = Selections::parse(reader)?;

      Some(parsed)
    }
    ::core::option::Option::None => None,
  };

  match (settings.refining, settings.refinement.template) {
    (Refining::Allowed, refinement::Template::Zero) => {
      let adaptive = refinement::parse_adaptive(reader)?;

      settings.refinement.adaptive = adaptive;
    }
    (Refining::Allowed, refinement::Template::One) | (Refining::Forbidden, _) => {}
  }

  let instances = reader.u32()?;

  settings.instances = instances;

  let coding = match selections {
    Some(selections_2) => {
      let codes = symbol_table(reader, symbols)?;

      Coding::Huffman {
        selections: selections_2,
        codes,
      }
    }
    None => Coding::Arithmetic,
  };
  let data = reader.tail()?;

  Ok(Header {
    info,
    settings,
    coding,
    data,
  })
}

pub(crate) fn code_length(symbols: u32) -> Result<u32, Never> {
  Ok(32u32.saturating_sub(symbols.saturating_sub(1).leading_zeros()))
}

pub(crate) fn decode_region(
  header: &Header<'_>,
  symbols: &[&Bitmap],
  custom: &[&HuffmanTable],
) -> Result<Bitmap, DecodeError> {
  let settings = &header.settings;
  let mut bitmap = Bitmap::new(header.info.size, header.info.origin, settings.default_pixel)?;

  match &header.coding {
    Coding::Huffman { selections, codes } => {
      let tables = selections.resolve(custom)?;
      let Ok(mut reader) = Reader::new(header.data);
      let Ok(bits) = settings.refinement.template.context_bits();
      let mut contexts = arithmetic::contexts(bits)?;

      decode_with(
        Source::Huffman {
          reader: &mut reader,
          tables,
          codes,
          contexts: &mut contexts,
        },
        symbols,
        settings,
        &mut bitmap,
      )?;
    }
    Coding::Arithmetic => {
      let Ok(count) = fitted::<_, u32>(symbols.len());
      let Ok(length) = code_length(count);
      let mut integers = Integers::new(length)?;
      let Ok(bits) = settings.refinement.template.context_bits();
      let mut contexts = arithmetic::contexts(bits)?;
      let Ok(mut decoder) = Arithmetic::new(header.data);
      let source = Source::Arithmetic {
        decoder: &mut decoder,
        integers: &mut integers,
        contexts: &mut contexts,
      };

      decode_with(source, symbols, settings, &mut bitmap)?;
    }
  }

  Ok(bitmap)
}
