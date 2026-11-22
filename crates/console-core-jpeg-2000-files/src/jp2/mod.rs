//! Reading a JP2 file, defined in Annex I.

use alloc::vec::Vec;

use crate::error::{FormatError, Result};
use crate::j2c::ComponentData;
use crate::jp2::r#box::{FILE_TYPE, JP2_SIGNATURE};
use crate::jp2::cdef::ChannelDefinitionBox;
use crate::jp2::cmap::{ComponentMappingBox, ComponentMappingEntry, ComponentMappingType};
use crate::jp2::colr::ColorSpecificationBox;
use crate::jp2::pclr::PaletteBox;
use crate::reader::BitReader;
use crate::{DecodeSettings, Image, resolve_alpha_and_color_space};

pub(crate) mod r#box;
pub(crate) mod cdef;
pub(crate) mod cmap;
pub(crate) mod colr;
pub(crate) mod icc;
pub(crate) mod pclr;

#[derive(Debug, Clone, Default)]
pub(crate) struct ImageBoxes {
  pub(crate) color_specification: Option<ColorSpecificationBox>,
  pub(crate) channel_definition: Option<ChannelDefinitionBox>,
  pub(crate) palette: Option<PaletteBox>,
  pub(crate) component_mapping: Option<ComponentMappingBox>,
}

pub(crate) struct DecodedImage<'a> {
  pub(crate) decoded_components: &'a mut Vec<ComponentData>,

  pub(crate) boxes: ImageBoxes,
}

pub(crate) fn parse<'a>(data: &'a [u8], mut settings: DecodeSettings) -> Result<Image<'a>> {
  let Ok(mut reader) = BitReader::new(data);
  let Ok(signature) = r#box::read(&mut reader);

  match signature {
    Some(r#box::Jp2Box {
      box_type: JP2_SIGNATURE,
      ..
    }) => {}
    Some(_) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::FormatError,
      >>::from(FormatError::InvalidSignature));
    }
    None => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::FormatError,
      >>::from(FormatError::InvalidBox));
    }
  }

  let Ok(file_type) = r#box::read(&mut reader);

  match file_type {
    Some(r#box::Jp2Box {
      box_type: FILE_TYPE,
      ..
    }) => {}
    Some(_) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::FormatError,
      >>::from(FormatError::InvalidFileType));
    }
    None => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::FormatError,
      >>::from(FormatError::InvalidBox));
    }
  }

  let mut image_boxes = None;
  let mut parsed_codestream = None;

  for _ in data.chunks(8) {
    let Ok(end) = reader.at_end();

    match end {
      crate::states::ReaderEnd::Reached => break,
      crate::states::ReaderEnd::Pending => {}
    }

    let Ok(next_box) = r#box::read(&mut reader);
    let current_box = match (next_box, settings.validation) {
      (Some(current), _) => current,
      (None, crate::ValidationMode::Strict) => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::FormatError,
        >>::from(FormatError::InvalidBox));
      }
      (None, _) => break,
    };

    match current_box.box_type {
      r#box::JP2_HEADER => {
        let header = parse_header(current_box.data, &mut settings)?;
        image_boxes = Some(header);
      }
      r#box::CONTIGUOUS_CODESTREAM => {
        let stream = crate::j2c::parse_raw(current_box.data, &settings)?;
        parsed_codestream = Some(stream);
      }
      _ => {}
    }
  }

  let mut boxes = image_boxes.ok_or(FormatError::InvalidBox)?;
  let codestream = parsed_codestream.ok_or(FormatError::MissingCodestream)?;

  match (&boxes.palette, &boxes.component_mapping) {
    (Some(palette), None) => {
      let mappings = (0..palette.columns.len())
        .map(|index| {
          let Ok(column) = console_core_number_conversion::wrapped::<_, u8>(index);
          ComponentMappingEntry {
            component_index: 0,
            mapping_type: ComponentMappingType::Palette { column },
          }
        })
        .collect();
      boxes.component_mapping = Some(ComponentMappingBox { entries: mappings });
    }
    _ => {}
  }

  let (color_space, has_alpha) =
    resolve_alpha_and_color_space(&boxes, &codestream.header, &settings)?;
  Ok(Image {
    codestream: codestream.data,
    header: codestream.header,
    boxes,
    settings,
    color_space,
    has_alpha,
  })
}

fn parse_header(data: &[u8], settings: &mut DecodeSettings) -> Result<ImageBoxes> {
  let mut boxes = ImageBoxes::default();
  let Ok(mut reader) = BitReader::new(data);

  for _ in data.chunks(8) {
    let Ok(end) = reader.at_end();

    match end {
      crate::states::ReaderEnd::Reached => break,
      crate::states::ReaderEnd::Pending => {}
    }

    let Ok(next_box) = r#box::read(&mut reader);
    let child = next_box.ok_or(FormatError::InvalidBox)?;

    match child.box_type {
      r#box::CHANNEL_DEFINITION => {
        ignore_optional_box_error(cdef::parse(&mut boxes, child.data), settings.validation)?;
      }
      r#box::COLOUR_SPECIFICATION => colr::parse(&mut boxes, child.data)?,
      r#box::PALETTE => {
        ignore_optional_box_error(pclr::parse(&mut boxes, child.data), settings.validation)?;
        settings.target_resolution = None;
      }
      r#box::COMPONENT_MAPPING => cmap::parse(&mut boxes, child.data)?,
      tag => {
        let Ok(()) =
          crate::log::debug(format_args!("ignoring JP2 header box with tag {tag:#010x}"));
      }
    }
  }

  Ok(boxes)
}

fn ignore_optional_box_error(
  parsed: Result<()>,
  validation: crate::ValidationMode,
) -> Result<()> {
  match (parsed, validation) {
    (Ok(()), _) => Ok(()),
    (Err(error), crate::ValidationMode::Strict) => Err(error),
    (Err(error), _) => {
      let Ok(()) = crate::log::debug(format_args!("ignoring optional JP2 header box: {error}"));
      Ok(())
    }
  }
}
