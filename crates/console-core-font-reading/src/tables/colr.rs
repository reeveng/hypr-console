//! The [COLR](https://docs.microsoft.com/en-us/typography/opentype/spec/colr) table

#[cfg(feature = "std")]
mod closure;

use super::variations::{DeltaSetIndexMap, ItemVariationStore};

include!("../../generated/generated_colr.rs");

pub type PaintId = u64;

impl<'a> Colr<'a> {
  pub fn v0_base_glyph(&self, glyph_id: GlyphId) -> Result<Option<Range<u64>>, ReadError> {
    let base_glyph_records = self.base_glyph_records()?;
    let records = base_glyph_records.ok_or(ReadError::NullOffset)?;
    let glyph_id_3 = match glyph_id.try_into() {
      Ok(glyph_id_2) => glyph_id_2,
      Err(_glyph_outside_font_range) => {
        return Ok(None);
      }
    };
    let record = match records.binary_search_by(|rec| {
      let Ok(rec_glyph_id) = rec.glyph_id();

      rec_glyph_id.cmp(&glyph_id_3)
    }) {
      Ok(ix) => &*match <[_]>::get(&*records, ix) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
        }
      },
      ::core::result::Result::Err(_insertion_position) => return Ok(None),
    };
    let start = {
      let Ok(first_layer_index) = record.first_layer_index();
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u64>(first_layer_index);
      migrated_value
    };
    let end = ::core::primitive::u64::wrapping_add(start, {
      let Ok(num_layers) = record.num_layers();
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u64>(num_layers);
      migrated_value
    });
    Ok(Some(start..end))
  }

  pub fn v0_layer(&self, index: u64) -> Result<(GlyphId16, u16), ReadError> {
    let layer_records = self.layer_records()?;
    let layers = layer_records.ok_or(ReadError::NullOffset)?;
    let Ok(value_8) = console_core_number_conversion::index(index);

    let layer = layers.get(value_8).ok_or(ReadError::OutOfBounds)?;
    let Ok(glyph_id) = layer.glyph_id();
    let Ok(palette_index) = layer.palette_index();

    Ok((glyph_id, palette_index))
  }

  pub fn v1_base_glyph(
    &self,
    glyph_id: GlyphId,
  ) -> Result<Option<(Paint<'a>, PaintId)>, ReadError> {
    let glyph_id_3 = match glyph_id.try_into() {
      Ok(glyph_id_2) => glyph_id_2,
      Err(_glyph_outside_font_range) => {
        return Ok(None);
      }
    };
    let base_glyph_list = self.base_glyph_list()?;
    let list = base_glyph_list.ok_or(ReadError::NullOffset)?;
    let Ok(records) = list.base_glyph_paint_records();
    let record = match records.binary_search_by(|rec| {
      let Ok(rec_glyph_id) = rec.glyph_id();

      rec_glyph_id.cmp(&glyph_id_3)
    }) {
      Ok(ix) => &*match <[_]>::get(&*records, ix) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
        }
      },
      ::core::result::Result::Err(_insertion_position) => return Ok(None),
    };
    let Ok(offset_data) = list.offset_data();

    let Ok(paint_offset) = record.paint_offset();
    let Ok(value_1) = paint_offset.to_u32();
    let Ok(base_address) = offset_data.base_address();

    let id = base_address.wrapping_add(u64::from(value_1));
    let paint = record.paint(offset_data)?;

    Ok(Some((paint, id)))
  }

  pub fn v1_layer(&self, index: u64) -> Result<(Paint<'a>, PaintId), ReadError> {
    let layer_list = self.layer_list()?;
    let list = layer_list.ok_or(ReadError::NullOffset)?;
    let Ok(value_9) = console_core_number_conversion::index(index);

    let Ok(paint_offsets) = list.paint_offsets();
    let paint_offset = paint_offsets.get(value_9).ok_or(ReadError::OutOfBounds)?;
    let Ok(value_10) = paint_offset.get();

    let offset = value_10;
    let Ok(offset_data) = list.offset_data();

    let value_3 = offset;
    let Ok(base_address) = offset_data.base_address();

    let Ok(value_7) = value_3.to_u32();

    let id = base_address.wrapping_add(u64::from(value_7));
    let value_5 = offset;

    let resolve = value_5.resolve(offset_data)?;

    Ok((resolve, id))
  }

  pub fn v1_clip_box(&self, glyph_id: GlyphId) -> Result<Option<ClipBox<'a>>, ReadError> {
    use core::cmp::Ordering;

    let glyph_id_2 = match GlyphId16::try_from(glyph_id) {
      Ok(small_glyph_id) => small_glyph_id,
      Err(_outside_font_glyph_range) => return Ok(None),
    };

    let clip_list = self.clip_list()?;
    let list = clip_list.ok_or(ReadError::NullOffset)?;
    let Ok(clips) = list.clips();
    let clip = match clips.binary_search_by(|clip| {
      let Ok(start_glyph_id) = clip.start_glyph_id();

      match ::core::primitive::bool::then_some(glyph_id_2 < start_glyph_id, ()) {
        ::core::option::Option::Some(()) => Ordering::Greater,
        ::core::option::Option::None => {
          let Ok(end_glyph_id) = clip.end_glyph_id();

          match ::core::primitive::bool::then_some(glyph_id_2 > end_glyph_id, ()) {
            ::core::option::Option::Some(()) => Ordering::Less,
            ::core::option::Option::None => Ordering::Equal,
          }
        }
      }
    }) {
      Ok(ix) => &*match <[_]>::get(&*clips, ix) {
        ::core::option::Option::Some(indexed_value) => indexed_value,
        ::core::option::Option::None => {
          return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
        }
      },
      ::core::result::Result::Err(_insertion_position) => return Ok(None),
    };
    let Ok(value_6) = list.offset_data();

    let clip_box = clip.clip_box(value_6)?;

    Ok(Some(clip_box))
  }
}
