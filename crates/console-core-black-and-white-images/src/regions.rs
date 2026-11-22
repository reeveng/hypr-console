use console_core_geometry::{Point, Size};

use crate::bitmap::Operator;
use crate::error::DecodeError;
use crate::reading::Reader;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RegionInfo {
  pub(crate) size: Size<u32>,
  pub(crate) origin: Point<u32>,
  pub(crate) operator: Operator,
}

impl RegionInfo {
  pub(crate) fn parse(reader: &mut Reader<'_>) -> Result<RegionInfo, DecodeError> {
    let width = reader.u32()?;
    let height = reader.u32()?;
    let x = reader.u32()?;
    let y = reader.u32()?;
    let flags = reader.byte()?;
    let operator = Operator::from_bits(flags)?;

    match flags & 0xF0 {
      0 => Ok(RegionInfo {
        size: Size { width, height },
        origin: Point { x, y },
        operator,
      }),
      _ => Err(DecodeError::InvalidCombinationOperator),
    }
  }
}
