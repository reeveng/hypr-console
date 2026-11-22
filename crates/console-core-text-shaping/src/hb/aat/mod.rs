pub mod layout;
pub mod layout_common;
pub mod layout_kerx_table;
pub mod layout_morx_table;
pub mod layout_trak_table;
pub mod map;

use crate::hb::aat::layout_kerx_table::KerxSubtableCache;
use crate::hb::aat::layout_morx_table::MorxSubtableCache;
use crate::hb::kerning::KernSubtableCache;
use crate::hb::ot::OtTables;
use crate::hb::tables::TableRanges;
use alloc::vec::Vec;
use read_fonts::{
  FontRef, TableProvider,
  tables::{ankr::Ankr, feat::Feat, kern::Kern, kerx::Kerx, morx::Morx, trak::Trak},
};

#[derive(Default)]
pub struct AatCache {
  pub morx: Vec<MorxSubtableCache>,
  pub kern: Vec<KernSubtableCache>,
  pub kerx: Vec<KerxSubtableCache>,
}

impl AatCache {
  #[allow(unused)]
  pub fn new<'a>(font: &impl TableProvider<'a>) -> Self {
    let mut cache = Self::default();
    let num_glyphs = font
      .maxp()
      .map(|maxp| {
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
            ({
              let Ok(value) = maxp.num_glyphs();
              value
            }),
          );
          migrated_value
        })
      })
      .unwrap_or_default();
    match font.morx() {
      Ok(morx) => {
        let Ok(chains) = morx.chains();
        for chain in ({
          let Ok(items) = chains.iter();
          items
        }) {
          let chain = match chain {
            Ok(chain) => chain,
            Err(_) => {
              continue;
            }
          };
          for subtable in ({
            let items = ({
              let Ok(array) = chain.subtables();
              let Ok(items) = array.iter();
              items
            });
            items
          }) {
            let subtable = match subtable {
              Ok(subtable) => subtable,
              Err(_) => {
                continue;
              }
            };
            cache
              .morx
              .push(MorxSubtableCache::new(&subtable, num_glyphs));
          }
        }
      }
      Err(_) => {}
    }
    match font.kern() {
      Ok(kern) => {
        for subtable in kern.subtables() {
          let subtable = match subtable {
            Ok(subtable) => subtable,
            Err(_) => {
              continue;
            }
          };
          cache
            .kern
            .push(KernSubtableCache::new(&subtable, num_glyphs));
        }
      }
      Err(_) => {}
    }
    match font.kerx() {
      Ok(kerx) => {
        for subtable in ({
          let items = ({
            let Ok(array) = kerx.subtables();
            let Ok(items) = array.iter();
            items
          });
          items
        }) {
          let subtable = match subtable {
            Ok(subtable) => subtable,
            Err(_) => {
              continue;
            }
          };
          cache
            .kerx
            .push(KerxSubtableCache::new(&subtable, num_glyphs));
        }
      }
      Err(_) => {}
    }
    cache
  }
}

#[derive(Clone, Default)]
pub struct AatTables<'a> {
  pub morx: Option<(Morx<'a>, &'a [MorxSubtableCache])>,
  pub ankr: Option<Ankr<'a>>,
  pub kern: Option<(Kern<'a>, &'a [KernSubtableCache])>,
  pub kerx: Option<(Kerx<'a>, &'a [KerxSubtableCache])>,
  pub trak: Option<Trak<'a>>,
  pub feat: Option<Feat<'a>>,
}

use crate::hb::algs::HB_CODEPOINT_ENCODE3 as encode3;

/// Blocklist specific broken morx tables identified by the combination of
/// morx, GSUB, and GDEF table lengths.
fn is_morx_blocklisted(morx_len: u32, gsub_len: u32, gdef_len: u32) -> bool {
  const BLOCKLIST: &[u64] = &[encode3(19892, 2794, 340)];
  let key = encode3(morx_len, gsub_len, gdef_len);
  BLOCKLIST.contains(&key)
}

impl<'a> AatTables<'a> {
  pub fn new(font: &FontRef<'a>, cache: &'a AatCache, table_ranges: &TableRanges) -> Self {
    let morx = match is_morx_blocklisted(
      table_ranges.morx.len(),
      table_ranges.gsub.len(),
      table_ranges.gdef.len(),
    ) {
      true => None,
      false => table_ranges
        .morx
        .resolve_table(font)
        .map(|table| (table, cache.morx.as_slice())),
    };
    let ankr = table_ranges.ankr.resolve_table(font);
    let kern = table_ranges
      .kern
      .resolve_table(font)
      .map(|table| (table, cache.kern.as_slice()));
    let kerx = table_ranges
      .kerx
      .resolve_table(font)
      .map(|table| (table, cache.kerx.as_slice()));
    let trak = table_ranges.trak.resolve_table(font);
    let feat = table_ranges.feat.resolve_table(font);
    Self {
      morx,
      ankr,
      kern,
      kerx,
      trak,
      feat,
    }
  }

  pub fn from_tables(
    font: &impl TableProvider<'a>,
    ot_tables: &OtTables,
    cache: &'a AatCache,
  ) -> Self {
    let morx = match font.morx() {
      Ok(morx) => {
        let gsub_len = ot_tables.gsub.as_ref().map_or(0, |table| {
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
              let Ok(value) = ({
                let Ok(value) = table.table.offset_data();
                value
              })
              .len();
              value
            });
            migrated_value
          })
        });
        let gdef_len = ot_tables.gdef.table.as_ref().map_or(0, |table| {
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
              let Ok(value) = ({
                let Ok(value) = table.offset_data();
                value
              })
              .len();
              value
            });
            migrated_value
          })
        });
        if is_morx_blocklisted(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
              let Ok(value) = ({
                let Ok(value) = morx.offset_data();
                value
              })
              .len();
              value
            });
            migrated_value
          }),
          gsub_len,
          gdef_len,
        ) {
          None
        } else {
          Some((morx, cache.morx.as_slice()))
        }
      }
      Err(_) => None,
    };
    let ankr = font.ankr().ok();
    let kern = font.kern().ok().map(|table| (table, cache.kern.as_slice()));
    let kerx = font.kerx().ok().map(|table| (table, cache.kerx.as_slice()));
    let trak = font.trak().ok();
    let feat = font.feat().ok();
    Self {
      morx,
      ankr,
      kern,
      kerx,
      trak,
      feat,
    }
  }
}
