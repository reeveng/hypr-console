use std::collections::BTreeMap;

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::fitted;

use crate::bitmap::{Bitmap, Operator};
use crate::error::DecodeError;
use crate::generic;
use crate::halftone::{self, Patterns};
use crate::huffman::HuffmanTable;
use crate::reading::Reader;
use crate::refinement;
use crate::segments::{Header, Kind, Placement, Segment};
use crate::symbols::{self, SymbolDictionary};
use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
  Continue,
  Stop,
}

struct State {
  page: Bitmap,
  regions: BTreeMap<u32, Bitmap>,
  patterns: BTreeMap<u32, Patterns>,
  dictionaries: BTreeMap<u32, SymbolDictionary>,
  tables: BTreeMap<u32, HuffmanTable>,
}

pub(crate) fn decode(segments: &[Segment<'_>], page: Bitmap) -> Result<Bitmap, DecodeError> {
  let mut state = State {
    page,
    regions: BTreeMap::new(),
    patterns: BTreeMap::new(),
    dictionaries: BTreeMap::new(),
    tables: BTreeMap::new(),
  };

  for segment in segments {
    let flow = step(&mut state, segment)?;

    match flow {
      Flow::Stop => break,
      Flow::Continue => {}
    }
  }

  Ok(state.page)
}

impl State {
  fn deliver(
    &mut self,
    header: &Header,
    placement: Placement,
    region: Bitmap,
    operator: Operator,
  ) -> Result<(), DecodeError> {
    match placement {
      Placement::Intermediate => {
        self.regions.insert(header.number, region);
      }
      Placement::Immediate => {
        let x = i32::try_from(region.origin.x).map_err(|_| DecodeError::Overflow)?;
        let y = i32::try_from(region.origin.y).map_err(|_| DecodeError::Overflow)?;
        let Ok(()) = self.page.combine(&region, Point { x, y }, operator);
      }
    }

    Ok(())
  }

  fn symbols_of(&self, header: &Header) -> Result<Vec<&Bitmap>, Never> {
    Ok(
      header
        .refers
        .iter()
        .filter_map(|number| self.dictionaries.get(number))
        .flat_map(|dictionary| dictionary.exported.iter())
        .collect(),
    )
  }

  fn tables_of(&self, header: &Header) -> Result<Vec<&HuffmanTable>, Never> {
    Ok(
      header
        .refers
        .iter()
        .filter_map(|number| self.tables.get(number))
        .collect(),
    )
  }
}

fn generic_region(
  state: &mut State,
  segment: &Segment<'_>,
  placement: Placement,
) -> Result<(), DecodeError> {
  let Ok(mut reader) = Reader::new(segment.data);
  let header = generic::parse(&mut reader, segment.header.length)?;
  let region = generic::decode_region(&header)?;

  state.deliver(&segment.header, placement, region, header.info.operator)
}

fn symbol_dictionary(state: &mut State, segment: &Segment<'_>) -> Result<(), DecodeError> {
  let Ok(mut reader) = Reader::new(segment.data);
  let header = symbols::parse(&mut reader)?;
  let retained = match segment.header.refers.last() {
    Some(last) => match state.dictionaries.get(last) {
      Some(dictionary) => dictionary.retained.as_ref(),
      None => None,
    },
    None => None,
  };
  let Ok(input) = state.symbols_of(&segment.header);
  let Ok(custom) = state.tables_of(&segment.header);
  let dictionary = symbols::decode(&header, &input, &custom, retained)?;

  state.dictionaries.insert(segment.header.number, dictionary);

  Ok(())
}

fn text_region(
  state: &mut State,
  segment: &Segment<'_>,
  placement: Placement,
) -> Result<(), DecodeError> {
  let Ok(mut reader) = Reader::new(segment.data);
  let Ok(input) = state.symbols_of(&segment.header);
  let Ok(custom) = state.tables_of(&segment.header);
  let Ok(count) = fitted::<_, u32>(input.len());
  let header = text::parse(&mut reader, count)?;
  let region = text::decode_region(&header, &input, &custom)?;

  state.deliver(&segment.header, placement, region, header.info.operator)
}

fn halftone_region(
  state: &mut State,
  segment: &Segment<'_>,
  placement: Placement,
) -> Result<(), DecodeError> {
  let first = segment
    .header
    .refers
    .first()
    .ok_or(DecodeError::MissingPatternDictionary)?;
  let patterns = state
    .patterns
    .get(first)
    .ok_or(DecodeError::MissingPatternDictionary)?;
  let Ok(mut reader) = Reader::new(segment.data);
  let header = halftone::parse(&mut reader)?;
  let region = halftone::decode_region(&header, patterns)?;

  state.deliver(&segment.header, placement, region, header.info.operator)
}

fn refinement_region(
  state: &mut State,
  segment: &Segment<'_>,
  placement: Placement,
) -> Result<(), DecodeError> {
  let referred = match segment.header.refers.first() {
    Some(first) => state.regions.get(first),
    None => None,
  };
  let reference = match referred {
    Some(found) => found,
    None => &state.page,
  };
  let Ok(mut reader) = Reader::new(segment.data);
  let header = refinement::parse(&mut reader)?;
  let region = refinement::decode_region(&header, reference)?;

  state.deliver(&segment.header, placement, region, header.info.operator)
}

fn step(state: &mut State, segment: &Segment<'_>) -> Result<Flow, DecodeError> {
  match segment.header.kind {
    Kind::GenericRegion(placement) => generic_region(state, segment, placement)?,
    Kind::PatternDictionary => {
      let Ok(mut reader) = Reader::new(segment.data);
      let header = halftone::parse_patterns(&mut reader)?;
      let patterns = halftone::decode_patterns(&header)?;

      state.patterns.insert(segment.header.number, patterns);
    }
    Kind::SymbolDictionary => symbol_dictionary(state, segment)?,
    Kind::TextRegion(placement) => text_region(state, segment, placement)?,
    Kind::HalftoneRegion(placement) => halftone_region(state, segment, placement)?,
    Kind::RefinementRegion(placement) => refinement_region(state, segment, placement)?,
    Kind::Tables => {
      let Ok(mut reader) = Reader::new(segment.data);
      let table = HuffmanTable::read_custom(&mut reader)?;

      state.tables.insert(segment.header.number, table);
    }
    Kind::EndOfPage | Kind::EndOfFile => return Ok(Flow::Stop),
    Kind::PageInformation
    | Kind::EndOfStripe
    | Kind::Profiles
    | Kind::ColourPalette
    | Kind::Extension => {}
  }

  Ok(Flow::Continue)
}
