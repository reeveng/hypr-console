use crate::hb::tables::{SelectedCmapSubtable, TableRanges};

use super::cache::hb_cache_t;
use read_fonts::{
  FontRef, TableProvider,
  tables::cmap::{Cmap, Cmap14, CmapSubtable, LegacyEncoding, MapVariant},
  types::GlyphId,
};

pub type cache_t = hb_cache_t<21, 19, 256, 32>;

#[derive(Clone)]
pub struct Charmap<'a> {
  subtable: Option<(SelectedCmapSubtable, CmapSubtable<'a>)>,
  vs_subtable: Option<Cmap14<'a>>,
}

impl<'a> Charmap<'a> {
  pub fn new(font: &FontRef<'a>, table_ranges: &TableRanges) -> Self {
    match table_ranges.cmap.resolve_table::<Cmap>(font) {
      Some(cmap) => {
        let data = ({
          let Ok(data) = cmap.offset_data();
          data
        });
        let Ok(records) = cmap.encoding_records();
        let subtable = table_ranges.cmap_subtable.and_then(|s| {
          Some((
            s,
            records
              .get(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index(s.index);
                  migrated_value
                }),
              )?
              .subtable((data))
              .ok()?,
          ))
        });
        let vs_subtable = table_ranges
          .cmap_vs_subtable
          .and_then(|index| {
            records.get(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
                migrated_value
              }),
            )
          })
          .and_then(|rec| rec.subtable((data)).ok())
          .and_then(|subtable| match subtable {
            CmapSubtable::Format14(table) => Some(table),
            _ => None,
          });
        Self {
          subtable,
          vs_subtable,
        }
      }
      None => Self {
        subtable: None,
        vs_subtable: None,
      },
    }
  }

  pub fn from_tables(font: &impl TableProvider<'a>) -> Self {
    match font.cmap() {
      Ok(cmap) => {
        let subtable = cmap.best_subtable().map(|(index, record, subtable)| {
          let Ok(legacy_encoding) = record.legacy_encoding();

          (
            SelectedCmapSubtable {
              index,
              legacy_encoding,
            },
            subtable,
          )
        });
        Self {
          subtable,
          vs_subtable: cmap.uvs_subtable().map(|(_, subtable)| subtable),
        }
      }
      Err(_) => Self {
        subtable: None,
        vs_subtable: None,
      },
    }
  }

  pub fn map(&self, mut c: u32) -> Option<GlyphId> {
    let subtable = self.subtable.as_ref()?;
    match (subtable.0.legacy_encoding, c) {
      (LegacyEncoding::MacRoman, 0x80..) => c = unicode_to_macintosh_roman(c),
      (_, _) => {}
    }
    let result = match &subtable.1 {
      CmapSubtable::Format0(table) => table.map_codepoint(c),
      CmapSubtable::Format6(table) => table.map_codepoint(c),
      CmapSubtable::Format10(table) => {
        let Ok(start_char_code) = table.start_char_code();

        match c.checked_sub(start_char_code) {
          Some(index) => {
            let Ok(num_chars) = table.num_chars();

            match index < num_chars {
              true => {
                let Ok(glyph_id_array) = table.glyph_id_array();

                glyph_id_array
                  .get(
                    ({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped_index(index);
                      migrated_value
                    }),
                  )
                  .map(|gid| {
                    GlyphId::from(
                      ({
                        let Ok(value) = gid.get();
                        value
                      }),
                    )
                  })
              }
              false => None,
            }
          }
          None => None,
        }
      }
      CmapSubtable::Format4(table) => table.map_codepoint(c),
      CmapSubtable::Format12(table) => table.map_codepoint(c),
      CmapSubtable::Format13(table) => table.map_codepoint(c),
      _ => None,
    };
    match (result, subtable.0.legacy_encoding, c) {
      (None, LegacyEncoding::WindowsSymbol, ..=0x00FF) => self.map(0xF000 + c),
      (mapping, _, _) => mapping,
    }
  }

  pub fn map_variant(&self, c: u32, vs: u32) -> Option<GlyphId> {
    let subtable = self.vs_subtable.as_ref()?;
    match subtable.map_variant(c, vs)? {
      MapVariant::UseDefault => self.map(c),
      MapVariant::Variant(gid) => Some(gid),
    }
  }
}

fn unicode_to_macintosh_roman(c: u32) -> u32 {
  let Ok(code) = console_core_macintosh_roman::encode(c);
  match code.filter(|code| *code >= 0x80) {
    Some(code) => u32::from(code),
    None => 0,
  }
}

#[cfg(test)]
mod tests {
  use super::unicode_to_macintosh_roman;

  #[test]
  fn only_extended_macintosh_roman_codes_are_remapped() {
    assert_eq!(unicode_to_macintosh_roman(u32::from('Ä')), 0x80);
    assert_eq!(unicode_to_macintosh_roman(u32::from('é')), 0x8e);
    assert_eq!(unicode_to_macintosh_roman(u32::from('⌘')), 0);
    assert_eq!(unicode_to_macintosh_roman(0x100c4), 0);
  }
}
