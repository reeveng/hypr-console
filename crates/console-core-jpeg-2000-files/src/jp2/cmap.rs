//! The component mapping box (cmap), defined in I.5.3.5.

use alloc::vec::Vec;

use crate::error::{FormatError, Result};
use crate::jp2::ImageBoxes;

pub(crate) fn parse(boxes: &mut ImageBoxes, data: &[u8]) -> Result<()> {
  let mut entries = Vec::with_capacity(data.len().wrapping_div(4));

  for record in data.chunks(4) {
    let (component_index, mapping_type, palette_column) = match record {
      &[high, low, method, column] => (u16::from_be_bytes([high, low]), method, column),
      _ => return Err(crate::DecodeError::Format(FormatError::InvalidBox)),
    };
    let mapping_type_2 = match mapping_type {
      0 => ComponentMappingType::Direct,
      1 => ComponentMappingType::Palette {
        column: palette_column,
      },
      _ => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::FormatError,
        >>::from(FormatError::InvalidBox));
      }
    };

    entries.push(ComponentMappingEntry {
      component_index,
      mapping_type: mapping_type_2,
    });
  }

  boxes.component_mapping = Some(ComponentMappingBox { entries });

  Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentMappingBox {
  pub(crate) entries: Vec<ComponentMappingEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentMappingEntry {
  pub(crate) component_index: u16,
  pub(crate) mapping_type: ComponentMappingType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComponentMappingType {
  Direct,
  Palette { column: u8 },
}
