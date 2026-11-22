//! The channel definition box (cdef), defined in I.5.3.6.

use alloc::vec::Vec;

use crate::error::{FormatError, Result};
use crate::jp2::ImageBoxes;
use crate::reader::BitReader;

pub(crate) fn parse(boxes: &mut ImageBoxes, data: &[u8]) -> Result<()> {
  let Ok(mut reader) = BitReader::new(data);
  let checked_value = (match reader.read_u16() {
    Ok(value) => value,
  })
  .ok_or(FormatError::InvalidBox)?;
  let Ok(count) = console_core_number_conversion::wrapped_index(checked_value);

  match ::core::primitive::bool::then_some(count == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::FormatError,
      >>::from(FormatError::InvalidBox));
    }
    ::core::option::Option::None => {}
  }

  let mut definitions =
    Vec::with_capacity(count.min(data.len().saturating_sub(2).wrapping_div(6)));

  for _ in 0..count {
    let channel_index = (match reader.read_u16() {
      Ok(value) => value,
    })
    .ok_or(FormatError::InvalidBox)?;
    let channel_type = (match reader.read_u16() {
      Ok(value) => value,
    })
    .ok_or(FormatError::InvalidBox)?;
    let association = (match reader.read_u16() {
      Ok(value) => value,
    })
    .ok_or(FormatError::InvalidBox)?;

    definitions.push(ChannelDefinition {
      channel_index,
      channel_type: {
        let checked_value_2 = (match ChannelType::from_raw(channel_type) {
          Ok(value) => value,
        })
        .ok_or(FormatError::InvalidBox)?;
        checked_value_2
      },
      _association: {
        let checked_value_3 = (match ChannelAssociation::from_raw(association) {
          Ok(value) => value,
        })
        .ok_or(FormatError::InvalidBox)?;
        checked_value_3
      },
    });
  }

  definitions.sort_by_key(|definition| definition.channel_index);

  for (idx, def) in definitions.iter().enumerate() {
    let Ok(def_channel_index) = console_core_number_conversion::wrapped_index(def.channel_index);

    match ::core::primitive::bool::then_some(def_channel_index != idx, ()) {
      ::core::option::Option::Some(()) => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::FormatError,
        >>::from(FormatError::InvalidBox));
      }
      ::core::option::Option::None => {}
    }
  }

  boxes.channel_definition = Some(ChannelDefinitionBox {
    channel_definitions: definitions,
  });

  Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ChannelDefinitionBox {
  pub(crate) channel_definitions: Vec<ChannelDefinition>,
}

#[derive(Debug, Clone)]
pub(crate) struct ChannelDefinition {
  pub(crate) channel_index: u16,
  pub(crate) channel_type: ChannelType,
  pub(crate) _association: ChannelAssociation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelType {
  Colour,
  Opacity,
}

impl ChannelType {
  fn from_raw(value: u16) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match value {
      0 => Some(Self::Colour),
      1 => Some(Self::Opacity),

      _ => None,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelAssociation {
  WholeImage,
  Colour(u16),
}

impl ChannelAssociation {
  fn from_raw(value: u16) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match value {
      0 => Some(Self::WholeImage),

      u16::MAX => None,
      v => Some(Self::Colour(v)),
    })
  }
}
