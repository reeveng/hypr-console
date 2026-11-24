//! Symbol dictionaries: the bitmaps text regions are made of, decoded a height class at a time.

use console_core_fax_images::Color;
use console_core_geometry::{Point, Size};
use console_core_mq_decoding::{Context, Decoder as Arithmetic};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use std::cmp::Ordering;

use crate::arithmetic;
use crate::bitmap::{Bitmap, Operator};
use crate::error::DecodeError;
use crate::generic::{self, Prediction, Settings, Template, parse_adaptive};
use crate::huffman::HuffmanTable;
use crate::integers::Integer;
use crate::mmr;
use crate::neighbourhood::Reference;
use crate::reading::Reader;
use crate::refinement;
use crate::standard::Standard;
use crate::text::{
  self, CornerPosition, Integers, Orientation, Refining, Selection, Selections, Source,
  code_length,
};

const MAX_SYMBOLS: u32 = 65_535;

const MAX_DICTIONARY_PIXELS: u64 = 1 << 26;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Entropy {
  Huffman,
  Arithmetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aggregation {
  Refined,
  Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Contexts {
  Reused,
  Fresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retention {
  Preserve,
  Dropped,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Flags {
  entropy: Entropy,
  aggregation: Aggregation,
  delta_height: Selection,
  delta_width: Selection,
  collective_size: Selection,
  aggregate_count: Selection,
  contexts: Contexts,
  retention: Retention,
  template: Template,
  refinement_template: refinement::Template,
}

#[derive(Debug, Clone)]
pub(crate) struct Header<'a> {
  flags: Flags,
  adaptive: [Point<i8>; 4],
  refinement_adaptive: [Point<i8>; 2],
  exported: u32,
  new: u32,
  data: &'a [u8],
}

#[derive(Debug, Clone)]
pub(crate) struct Retained {
  generic: Vec<Context>,
  refinement: Vec<Context>,
}

#[derive(Debug, Clone)]
pub(crate) struct SymbolDictionary {
  pub(crate) exported: Vec<Bitmap>,
  pub(crate) retained: Option<Retained>,
}

fn parse_flags(word: u16) -> Result<Flags, DecodeError> {
  let entropy = match word & 0x0001 {
    0 => Entropy::Arithmetic,
    _ => Entropy::Huffman,
  };
  let aggregation = match word & 0x0002 {
    0 => Aggregation::Plain,
    _ => Aggregation::Refined,
  };
  let delta_height = match word.wrapping_shr(2) & 0x03 {
    0 => Selection::Standard(Standard::B4),
    1 => Selection::Standard(Standard::B5),
    3 => Selection::Custom,
    _ => return Err(DecodeError::InvalidTableSelection),
  };
  let delta_width = match word.wrapping_shr(4) & 0x03 {
    0 => Selection::Standard(Standard::B2),
    1 => Selection::Standard(Standard::B3),
    3 => Selection::Custom,
    _ => return Err(DecodeError::InvalidTableSelection),
  };
  let collective_size = match word & 0x0040 {
    0 => Selection::Standard(Standard::B1),
    _ => Selection::Custom,
  };
  let aggregate_count = match word & 0x0080 {
    0 => Selection::Standard(Standard::B1),
    _ => Selection::Custom,
  };
  let contexts = match word & 0x0100 {
    0 => Contexts::Fresh,
    _ => Contexts::Reused,
  };
  let retention = match word & 0x0200 {
    0 => Retention::Dropped,
    _ => Retention::Preserve,
  };
  let template = u8::try_from(word.wrapping_shr(10) & 0x03).map_err(|_| DecodeError::Overflow)?;
  let refinement_template =
    u8::try_from(word.wrapping_shr(12) & 0x01).map_err(|_| DecodeError::Overflow)?;
  let Ok(template_2) = Template::from_bits(template);
  let Ok(refinement_template_2) = refinement::Template::from_bits(refinement_template);

  Ok(Flags {
    entropy,
    aggregation,
    delta_height,
    delta_width,
    collective_size,
    aggregate_count,
    contexts,
    retention,
    template: template_2,
    refinement_template: refinement_template_2,
  })
}

pub(crate) fn parse<'a>(reader: &mut Reader<'a>) -> Result<Header<'a>, DecodeError> {
  let word = reader.u16()?;
  let flags = parse_flags(word)?;
  let adaptive = match flags.entropy {
    Entropy::Arithmetic => {
      let Ok(count) = flags.template.adaptive_count();

      parse_adaptive(reader, count)?
    }
    Entropy::Huffman => [Point { x: 0, y: 0 }; 4],
  };
  let refinement_adaptive = match (flags.aggregation, flags.refinement_template) {
    (Aggregation::Refined, refinement::Template::Zero) => refinement::parse_adaptive(reader)?,
    (Aggregation::Refined, refinement::Template::One) | (Aggregation::Plain, _) => {
      [Point { x: 0, y: 0 }; 2]
    }
  };
  let exported = reader.u32()?;
  let new = reader.u32()?;

  match ::core::primitive::bool::then_some(exported.saturating_add(new) > MAX_SYMBOLS, ()) {
    ::core::option::Option::Some(()) => return Err(DecodeError::Overflow),
    ::core::option::Option::None => {}
  }

  let data = reader.tail()?;

  Ok(Header {
    flags,
    adaptive,
    refinement_adaptive,
    exported,
    new,
    data,
  })
}

struct Huffman<'a> {
  height: HuffmanTable,
  width: HuffmanTable,
  collective: HuffmanTable,
  aggregate: HuffmanTable,
  export: HuffmanTable,
  offset: HuffmanTable,
  size: HuffmanTable,
  reader: Reader<'a>,
}

struct Arithmetical<'a> {
  decoder: Arithmetic<'a>,
  height: Integer,
  width: Integer,
  export: Integer,
  aggregate: Integer,
  text: Option<Integers>,
}

enum Coder<'a> {
  Huffman(Box<Huffman<'a>>),
  Arithmetic(Box<Arithmetical<'a>>),
}

fn coder<'a>(header: &Header<'a>, custom: &[&HuffmanTable]) -> Result<Coder<'a>, DecodeError> {
  match header.flags.entropy {
    Entropy::Huffman => {
      let mut next = custom.iter();
      let height = text::pick(header.flags.delta_height, &mut next)?;
      let width = text::pick(header.flags.delta_width, &mut next)?;
      let collective = text::pick(header.flags.collective_size, &mut next)?;
      let aggregate = text::pick(header.flags.aggregate_count, &mut next)?;
      let export = HuffmanTable::standard(Standard::B1)?;
      let offset = HuffmanTable::standard(Standard::B15)?;
      let size = HuffmanTable::standard(Standard::B1)?;
      let Ok(reader) = Reader::new(header.data);

      Ok(Coder::Huffman(Box::new(Huffman {
        height,
        width,
        collective,
        aggregate,
        export,
        offset,
        size,
        reader,
      })))
    }
    Entropy::Arithmetic => {
      let Ok(decoder) = Arithmetic::new(header.data);
      let height = Integer::new()?;
      let width = Integer::new()?;
      let export = Integer::new()?;
      let aggregate = Integer::new()?;

      Ok(Coder::Arithmetic(Box::new(Arithmetical {
        decoder,
        height,
        width,
        export,
        aggregate,
        text: None,
      })))
    }
  }
}

struct Dictionary<'a, 'h> {
  header: &'h Header<'a>,
  input: &'h [&'h Bitmap],
  coder: Coder<'a>,
  generic: Vec<Context>,
  refinement: Vec<Context>,
  new: Vec<Bitmap>,
  widths: Vec<u32>,
  height: u32,
  total_width: u32,
  pixels: u64,
  decoded: u32,
  class_start: u32,
}

fn in_band(decoded: Result<Option<i32>, DecodeError>) -> Result<i32, DecodeError> {
  match decoded {
    Ok(Some(value)) => Ok(value),
    Ok(None) => Err(DecodeError::UnexpectedOutOfBand),
    Err(fault) => Err(fault),
  }
}

pub(crate) fn decode(
  header: &Header<'_>,
  input: &[&Bitmap],
  custom: &[&HuffmanTable],
  retained: Option<&Retained>,
) -> Result<SymbolDictionary, DecodeError> {
  let (generic, refinement) = match (header.flags.contexts, retained) {
    (Contexts::Reused, Some(kept)) => (kept.generic.clone(), kept.refinement.clone()),
    (Contexts::Reused, None) => return Err(DecodeError::InvalidSymbol),
    (Contexts::Fresh, _) => {
      let Ok(generic_bits) = header.flags.template.context_bits();
      let Ok(refinement_bits) = header.flags.refinement_template.context_bits();

      let generic = arithmetic::contexts(generic_bits)?;
      let refinement = arithmetic::contexts(refinement_bits)?;

      (generic, refinement)
    }
  };
  let coder = coder(header, custom)?;
  let mut dictionary = Dictionary {
    header,
    input,
    coder,
    generic,
    refinement,
    new: Vec::new(),
    widths: Vec::new(),
    height: 0,
    total_width: 0,
    pixels: 0,
    decoded: 0,
    class_start: 0,
  };

  dictionary.classes()?;

  let exported = dictionary.export()?;
  let retained_2 = match header.flags.retention {
    Retention::Preserve => Some(Retained {
      generic: dictionary.generic,
      refinement: dictionary.refinement,
    }),
    Retention::Dropped => None,
  };

  Ok(SymbolDictionary {
    exported,
    retained: retained_2,
  })
}

impl Dictionary<'_, '_> {
  fn total(&self) -> Result<u32, DecodeError> {
    let Ok(held) = fitted::<_, u32>(self.input.len());

    held
      .checked_add(self.header.new)
      .ok_or(DecodeError::Overflow)
  }

  fn height_delta(&mut self) -> Result<i32, DecodeError> {
    match &mut self.coder {
      Coder::Huffman(huffman) => huffman.height.decode_in_band(&mut huffman.reader),
      Coder::Arithmetic(arithmetical) => {
        in_band(arithmetical.height.decode(&mut arithmetical.decoder))
      }
    }
  }

  fn width_delta(&mut self) -> Result<Option<i32>, DecodeError> {
    match &mut self.coder {
      Coder::Huffman(huffman) => huffman.width.decode(&mut huffman.reader),
      Coder::Arithmetic(arithmetical) => arithmetical.width.decode(&mut arithmetical.decoder),
    }
  }

  fn classes(&mut self) -> Result<(), DecodeError> {
    for _class in 0..MAX_SYMBOLS {
      match ::core::primitive::bool::then_some(self.decoded >= self.header.new, ()) {
        ::core::option::Option::Some(()) => return Ok(()),
        ::core::option::Option::None => {}
      }

      let delta = self.height_delta()?;

      let height = self
        .height
        .checked_add_signed(delta)
        .ok_or(DecodeError::Overflow)?;

      self.height = height;
      self.total_width = 0;
      self.class_start = self.decoded;
      self.widths_of_class()?;

      match (self.header.flags.entropy, self.header.flags.aggregation) {
        (Entropy::Huffman, Aggregation::Plain) => self.collective()?,
        (Entropy::Huffman, Aggregation::Refined) | (Entropy::Arithmetic, _) => {}
      }
    }

    match ::core::primitive::bool::then_some(self.decoded >= self.header.new, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(DecodeError::NoSymbols),
    }
  }

  fn widths_of_class(&mut self) -> Result<(), DecodeError> {
    let mut width = 0u32;

    for _symbol in 0..=self.header.new {
      let delta = self.width_delta()?;
      let delta_3 = match delta {
        Some(delta_2) => delta_2,
        None => return Ok(()),
      };

      match ::core::primitive::bool::then_some(self.decoded >= self.header.new, ()) {
        ::core::option::Option::Some(()) => return Err(DecodeError::TooManySymbols),
        ::core::option::Option::None => {}
      }

      let widened = width
        .checked_add_signed(delta_3)
        .ok_or(DecodeError::Overflow)?;
      let total_width = self
        .total_width
        .checked_add(widened)
        .ok_or(DecodeError::Overflow)?;

      width = widened;
      self.total_width = total_width;
      self.pixels = self
        .pixels
        .saturating_add(u64::from(width).saturating_mul(u64::from(self.height)));

      match ::core::primitive::bool::then_some(self.pixels > MAX_DICTIONARY_PIXELS, ()) {
        ::core::option::Option::Some(()) => return Err(DecodeError::Overflow),
        ::core::option::Option::None => {}
      }

      self.symbol(width)?;
      self.decoded = self.decoded.saturating_add(1);
    }

    Ok(())
  }

  fn symbol(&mut self, width: u32) -> Result<(), DecodeError> {
    let size = Size {
      width,
      height: self.height,
    };

    match (self.header.flags.entropy, self.header.flags.aggregation) {
      (Entropy::Arithmetic, Aggregation::Plain) => {
        let mut region = Bitmap::white(size)?;
        let settings = Settings {
          template: self.header.flags.template,
          adaptive: self.header.adaptive,
          prediction: Prediction::Plain,
          skip: None,
        };

        match &mut self.coder {
          Coder::Arithmetic(arithmetical) => generic::decode(
            &mut region,
            &mut arithmetical.decoder,
            &mut self.generic,
            &settings,
          )?,
          Coder::Huffman(_) => return Err(DecodeError::InvalidSymbol),
        }

        self.new.push(region);
      }
      (Entropy::Huffman, Aggregation::Plain) => self.widths.push(width),
      (_, Aggregation::Refined) => {
        let symbol = self.refined(size)?;

        self.new.push(symbol);
      }
    }

    Ok(())
  }

  fn aggregate_count(&mut self) -> Result<i32, DecodeError> {
    match &mut self.coder {
      Coder::Huffman(huffman) => huffman.aggregate.decode_in_band(&mut huffman.reader),
      Coder::Arithmetic(arithmetical) => {
        in_band(arithmetical.aggregate.decode(&mut arithmetical.decoder))
      }
    }
  }

  fn refined(&mut self, size: Size<u32>) -> Result<Bitmap, DecodeError> {
    let count = self.aggregate_count()?;

    match count.cmp(&1) {
      Ordering::Equal => self.single(size),
      Ordering::Greater => {
        let count_2 = u32::try_from(count).map_err(|_| DecodeError::Overflow)?;

        self.aggregated(size, count_2)
      }
      Ordering::Less => Err(DecodeError::InvalidSymbol),
    }
  }

  fn text_integers(&mut self, length: u32) -> Result<(), DecodeError> {
    match &mut self.coder {
      Coder::Arithmetic(arithmetical) => match arithmetical.text {
        Some(_) => Ok(()),
        None => {
          let integers = Integers::new(length)?;

          arithmetical.text = Some(integers);

          Ok(())
        }
      },
      Coder::Huffman(_) => Ok(()),
    }
  }
}

fn lookup<'s>(
  input: &'s [&'s Bitmap],
  new: &'s [Bitmap],
  id: u32,
) -> Result<&'s Bitmap, DecodeError> {
  let Ok(at) = index(id);
  let found = match at.checked_sub(input.len()) {
    None => input.get(at).copied(),
    Some(later) => new.get(later),
  };

  found.ok_or(DecodeError::SymbolOutOfRange)
}

fn refinement_settings(header: &Header<'_>) -> Result<refinement::Settings, Never> {
  Ok(refinement::Settings {
    template: header.flags.refinement_template,
    adaptive: header.refinement_adaptive,
    prediction: Prediction::Plain,
  })
}

impl Dictionary<'_, '_> {
  fn single(&mut self, size: Size<u32>) -> Result<Bitmap, DecodeError> {
    let total = self.total()?;
    let Ok(length) = code_length(total);

    self.text_integers(length)?;

    let Dictionary {
      header,
      input,
      coder,
      refinement,
      new,
      ..
    } = self;
    let Ok(settings) = refinement_settings(header);
    let mut region = Bitmap::white(size)?;

    match coder {
      Coder::Huffman(huffman) => {
        let id = huffman.reader.bits(length.max(1))?;
        let x = huffman.offset.decode_in_band(&mut huffman.reader)?;
        let y = huffman.offset.decode_in_band(&mut huffman.reader)?;
        let bitmap = lookup(input, new, id)?;
        let against = Reference {
          bitmap,
          shift: Point { x, y },
        };
        let bytes = huffman.size.decode_in_band(&mut huffman.reader)?;
        let bytes_2 = u32::try_from(bytes).map_err(|_| DecodeError::Overflow)?;
        let Ok(()) = huffman.reader.align();
        let data = huffman.reader.bytes(bytes_2)?;
        let Ok(mut decoder) = Arithmetic::new(data);

        refinement::decode(&mut region, &mut decoder, refinement, &against, &settings)?;
      }
      Coder::Arithmetic(arithmetical) => {
        let integers = arithmetical
          .text
          .as_mut()
          .ok_or(DecodeError::InvalidSymbol)?;
        let id = integers.identifier.decode(&mut arithmetical.decoder)?;
        let x = in_band(integers.x.decode(&mut arithmetical.decoder))?;
        let y = in_band(integers.y.decode(&mut arithmetical.decoder))?;
        let bitmap = lookup(input, new, id)?;
        let against = Reference {
          bitmap,
          shift: Point { x, y },
        };

        refinement::decode(
          &mut region,
          &mut arithmetical.decoder,
          refinement,
          &against,
          &settings,
        )?;
      }
    }

    Ok(region)
  }

  fn aggregated(&mut self, size: Size<u32>, count: u32) -> Result<Bitmap, DecodeError> {
    let total = self.total()?;
    let Ok(length) = code_length(total);

    self.text_integers(length)?;

    let Dictionary {
      header,
      input,
      coder,
      refinement,
      new,
      ..
    } = self;
    let Ok(refinement_settings) = refinement_settings(header);
    let settings = text::Settings {
      refining: Refining::Allowed,
      log_strip_size: 0,
      corner: CornerPosition::TopLeft,
      orientation: Orientation::Upright,
      operator: Operator::Or,
      default_pixel: Color::White,
      delta_s_offset: 0,
      refinement: refinement_settings,
      instances: count,
    };
    let mut all: Vec<&Bitmap> = input.to_vec();

    all.extend(new.iter());

    let mut region = Bitmap::white(size)?;

    match coder {
      Coder::Huffman(huffman) => {
        let Ok(selections) = Selections::aggregating();
        let tables = selections.resolve(&[])?;
        let Ok(prefix) = crate::huffman::prefix_of(length);
        let codes = HuffmanTable::uniform(total, prefix)?;
        let source = Source::Huffman {
          reader: &mut huffman.reader,
          tables,
          codes: &codes,
          contexts: refinement,
        };

        text::decode_with(source, &all, &settings, &mut region)?;
      }
      Coder::Arithmetic(arithmetical) => {
        let integers = arithmetical
          .text
          .as_mut()
          .ok_or(DecodeError::InvalidSymbol)?;
        let source = Source::Arithmetic {
          decoder: &mut arithmetical.decoder,
          integers,
          contexts: refinement,
        };

        text::decode_with(source, &all, &settings, &mut region)?;
      }
    }

    Ok(region)
  }

  fn collective(&mut self) -> Result<(), DecodeError> {
    let size = Size {
      width: self.total_width,
      height: self.height,
    };
    let mut collective = Bitmap::white(size)?;

    match &mut self.coder {
      Coder::Huffman(huffman) => {
        let bytes = huffman.collective.decode_in_band(&mut huffman.reader)?;
        let bytes_2 = u32::try_from(bytes).map_err(|_| DecodeError::Overflow)?;
        let Ok(()) = huffman.reader.align();

        match bytes_2 {
          0 => raw(&mut collective, &mut huffman.reader)?,
          _ => {
            let data = huffman.reader.bytes(bytes_2)?;
            let _read = mmr::decode(&mut collective, data)?;
          }
        }
      }
      Coder::Arithmetic(_) => return Err(DecodeError::InvalidSymbol),
    }

    let mut offset = 0u32;

    for at in self.class_start..self.decoded {
      let Ok(slot) = index(at);
      let held = self.widths.get(slot).ok_or(DecodeError::InvalidSymbol)?;
      let width = *held;
      let mut symbol = Bitmap::white(Size {
        width,
        height: self.height,
      })?;
      let start = i32::try_from(offset).map_err(|_| DecodeError::Overflow)?;
      let Ok(()) = symbol.combine(
        &collective,
        Point {
          x: start.saturating_neg(),
          y: 0,
        },
        Operator::Replace,
      );

      self.new.push(symbol);
      let advanced = offset.checked_add(width).ok_or(DecodeError::Overflow)?;

      offset = advanced;
    }

    Ok(())
  }

  fn run_length(&mut self) -> Result<u32, DecodeError> {
    let value = match &mut self.coder {
      Coder::Huffman(huffman) => huffman.export.decode_in_band(&mut huffman.reader),
      Coder::Arithmetic(arithmetical) => {
        in_band(arithmetical.export.decode(&mut arithmetical.decoder))
      }
    };
    let value_2 = value?;

    u32::try_from(value_2).map_err(|_| DecodeError::InvalidHuffmanCode)
  }

  fn export(&mut self) -> Result<Vec<Bitmap>, DecodeError> {
    let total = self.total()?;
    let mut exported = Vec::new();
    let mut at = 0u32;
    let mut exporting = false;

    for _run in 0..total.saturating_mul(2).saturating_add(2) {
      match ::core::primitive::bool::then_some(at >= total, ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => {}
      }

      let run = self.run_length()?;
      let end = at.checked_add(run).ok_or(DecodeError::Overflow)?;

      match ::core::primitive::bool::then_some(end > total, ()) {
        ::core::option::Option::Some(()) => return Err(DecodeError::SymbolOutOfRange),
        ::core::option::Option::None => {}
      }

      match ::core::primitive::bool::then_some(exporting, ()) {
        ::core::option::Option::Some(()) => {
          for id in at..end {
            let found = lookup(self.input, &self.new, id)?;

            exported.push(found.clone());
          }
        }
        ::core::option::Option::None => {}
      }

      at = end;
      exporting = !exporting;
    }

    let Ok(count) = fitted::<_, u32>(exported.len());

    match (at >= total, count == self.header.exported) {
      (true, true) => Ok(exported),
      (false, _) | (_, false) => Err(DecodeError::NoSymbols),
    }
  }
}

fn raw(collective: &mut Bitmap, reader: &mut Reader<'_>) -> Result<(), DecodeError> {
  let size = collective.size;

  for y in 0..size.height {
    for byte_x in 0..size.width.div_ceil(8) {
      let byte = reader.byte()?;

      for bit in 0..8u32 {
        let x = byte_x.saturating_mul(8).saturating_add(bit);
        let pixel = u32::from(byte).wrapping_shr(7u32.saturating_sub(bit)) & 1;
        let Ok(()) = collective.put(Point { x, y }, pixel);
      }
    }
  }

  Ok(())
}
