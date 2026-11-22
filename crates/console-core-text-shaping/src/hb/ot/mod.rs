use super::buffer::GlyphPropsFlags;
use super::ot_layout::TableIndex;
use super::{common::TagExt, set_digest::hb_set_digest_t};
use crate::hb::hb_tag_t;
use crate::hb::ot_layout_gsubgpos::{BinaryCache, MappingCache};
use crate::hb::tables::TableRanges;
use alloc::vec::Vec;
use lookup::{LookupCache, LookupInfo};
use read_fonts::tables::layout::{ClassRangeRecord, RangeRecord};
use read_fonts::types::GlyphId16;
use read_fonts::{
  FontData, FontRef, ReadError, ResolveOffset, TableProvider,
  tables::{
    gdef::Gdef,
    gpos::{AnchorTable, DeviceOrVariationIndex, Gpos},
    gsub::{ClassDef, FeatureList, FeatureVariations, Gsub, ScriptList},
    layout::{Feature, LangSys, Script},
    varc::{Condition, CoverageTable},
    variations::{DeltaSetIndex, ItemVariationStore},
  },
  types::{BigEndian, F2Dot14, GlyphId, Offset32},
};

pub mod contextual;
pub mod gpos;
pub mod gsub;
pub mod lookup;

pub struct OtCache {
  pub gsub: LookupCache,
  pub gpos: LookupCache,
  pub gdef_glyph_props_cache: MappingCache,
  pub gdef_mark_set_digests: Vec<hb_set_digest_t>,
}

impl OtCache {
  pub fn new<'a>(font: &impl TableProvider<'a>) -> Self {
    let gsub = font
      .gsub()
      .map(|t| LookupCache::new(&t))
      .unwrap_or_default();
    let gpos = font
      .gpos()
      .map(|t| LookupCache::new(&t))
      .unwrap_or_default();
    let mut gdef_mark_set_digests = Vec::new();
    match font.gdef() {
      Ok(gdef) => {
        if let Ok(Some(mark_sets)) = gdef.mark_glyph_sets_def() {
          gdef_mark_set_digests.extend(
            ({
              let items = ({
                let Ok(array) = mark_sets.coverages();
                let Ok(items) = array.iter();
                items
              });
              items
            })
            .map(|set| {
              set
                .ok()
                .map(|coverage| hb_set_digest_t::from_coverage(&coverage))
                .unwrap_or_default()
            }),
          );
        }
      }
      Err(_) => {}
    }
    Self {
      gsub,
      gpos,
      gdef_glyph_props_cache: MappingCache::new(),
      gdef_mark_set_digests,
    }
  }
}

#[derive(Clone)]
pub struct GsubTable<'a> {
  pub table: Gsub<'a>,
  pub lookups: &'a LookupCache,
}

impl crate::hb::ot_layout::LayoutTable for GsubTable<'_> {
  const INDEX: TableIndex = TableIndex::GSUB;
  const IN_PLACE: bool = false;

  fn get_lookup(&self, index: u16) -> Option<&LookupInfo> {
    self.lookups.get(&self.table, index)
  }
}

#[derive(Clone)]
pub struct GposTable<'a> {
  pub table: Gpos<'a>,
  pub lookups: &'a LookupCache,
}

impl crate::hb::ot_layout::LayoutTable for GposTable<'_> {
  const INDEX: TableIndex = TableIndex::GPOS;
  const IN_PLACE: bool = true;

  fn get_lookup(&self, index: u16) -> Option<&LookupInfo> {
    self.lookups.get(&self.table, index)
  }
}

#[derive(Clone, Default)]
pub struct GdefTable<'a> {
  pub(crate) table: Option<Gdef<'a>>,
  classes: Option<ClassDef<'a>>,
  mark_classes: Option<ClassDef<'a>>,
  mark_sets: Option<(FontData<'a>, &'a [BigEndian<Offset32>])>,
}

impl<'a> GdefTable<'a> {
  fn new(gdef: Gdef<'a>) -> Self {
    let classes = gdef.glyph_class_def().ok().flatten();
    let mark_classes = gdef.mark_attach_class_def().ok().flatten();
    let mark_sets = gdef.mark_glyph_sets_def().ok().flatten().map(|sets| {
      (
        ({
          let Ok(data) = sets.offset_data();
          data
        }),
        ({
          let Ok(value) = sets.coverage_offsets();
          value
        }),
      )
    });
    Self {
      table: Some(gdef),
      classes,
      mark_classes,
      mark_sets,
    }
  }
}

#[derive(Clone)]
pub struct OtTables<'a> {
  pub gsub: Option<GsubTable<'a>>,
  pub gpos: Option<GposTable<'a>>,
  pub gdef: GdefTable<'a>,
  pub gdef_glyph_props_cache: &'a MappingCache,
  pub gdef_mark_set_digests: &'a [hb_set_digest_t],
  pub coords: &'a [F2Dot14],
  pub var_store: Option<ItemVariationStore<'a>>,
  pub feature_variations: [Option<u32>; 2],
}

impl<'a> OtTables<'a> {
  pub fn new(
    font: &FontRef<'a>,
    cache: &'a OtCache,
    table_offsets: &TableRanges,
    coords: &'a [F2Dot14],
    feature_variations: [Option<u32>; 2],
  ) -> Self {
    let gsub = table_offsets
      .gsub
      .resolve_table(font)
      .map(|table| GsubTable {
        table,
        lookups: &cache.gsub,
      });
    let gpos = table_offsets
      .gpos
      .resolve_table(font)
      .map(|table| GposTable {
        table,
        lookups: &cache.gpos,
      });
    let coords = match coords.iter().any(|coord| *coord != F2Dot14::ZERO) {
      true => coords,
      false => &[],
    };
    let gdef = match is_gdef_blocklisted(
      table_offsets.gdef.len(),
      table_offsets.gsub.len(),
      table_offsets.gpos.len(),
    ) {
      true => GdefTable::default(),
      false => table_offsets
        .gdef
        .resolve_table(font)
        .map(GdefTable::new)
        .unwrap_or_default(),
    };
    let var_store = match !coords.is_empty() {
      true => gdef
        .table
        .as_ref()
        .and_then(|gdef| gdef.item_var_store().ok().flatten()),
      false => None,
    };
    Self {
      gsub,
      gpos,
      gdef,
      gdef_glyph_props_cache: &cache.gdef_glyph_props_cache,
      gdef_mark_set_digests: &cache.gdef_mark_set_digests,
      var_store,
      coords,
      feature_variations,
    }
  }

  pub fn from_tables(
    font: &impl TableProvider<'a>,
    cache: &'a OtCache,
    coords: &'a [F2Dot14],
    feature_variations: [Option<u32>; 2],
  ) -> Self {
    let gsub = font.gsub().ok().map(|table| GsubTable {
      table,
      lookups: &cache.gsub,
    });
    let gpos = font.gpos().ok().map(|table| GposTable {
      table,
      lookups: &cache.gpos,
    });
    let coords = match coords.iter().any(|coord| *coord != F2Dot14::ZERO) {
      true => coords,
      false => &[],
    };
    let gdef = match font.gdef() {
      Ok(gdef) => {
        let gsub_len = gsub
          .as_ref()
          .map(|t| {
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
                let Ok(value) = ({
                  let Ok(value) = t.table.offset_data();
                  value
                })
                .len();
                value
              });
              migrated_value
            })
          })
          .unwrap_or_default();
        let gpos_len = gpos
          .as_ref()
          .map(|t| {
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
                let Ok(value) = ({
                  let Ok(value) = t.table.offset_data();
                  value
                })
                .len();
                value
              });
              migrated_value
            })
          })
          .unwrap_or_default();
        if is_gdef_blocklisted(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>({
              let Ok(value) = ({
                let Ok(value) = gdef.offset_data();
                value
              })
              .len();
              value
            });
            migrated_value
          }),
          gsub_len,
          gpos_len,
        ) {
          GdefTable::default()
        } else {
          GdefTable::new(gdef)
        }
      }
      Err(_) => GdefTable::default(),
    };
    let var_store = match !coords.is_empty() {
      true => gdef
        .table
        .as_ref()
        .and_then(|gdef| gdef.item_var_store().ok().flatten()),
      false => None,
    };
    Self {
      gsub,
      gpos,
      gdef,
      gdef_glyph_props_cache: &cache.gdef_glyph_props_cache,
      gdef_mark_set_digests: &cache.gdef_mark_set_digests,
      var_store,
      coords,
      feature_variations,
    }
  }

  pub fn has_glyph_classes(&self) -> bool {
    self.gdef.classes.is_some()
  }

  pub fn glyph_class(&self, glyph_id: u32) -> u16 {
    self
      .gdef
      .classes
      .as_ref()
      .map_or(0, |class_def| class_def.get(glyph_id))
  }

  pub fn glyph_mark_attachment_class(&self, glyph_id: u32) -> u16 {
    self
      .gdef
      .mark_classes
      .as_ref()
      .map_or(0, |class_def| class_def.get(glyph_id))
  }

  pub(crate) fn glyph_props(&self, glyph: GlyphId) -> u16 {
    let glyph = ({
      let Ok(value) = glyph.to_u32();
      value
    });

    match self.gdef_glyph_props_cache.get((glyph)) {
      Some(props) => {
        return ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(props);
          migrated_value
        });
      }
      None => {}
    }

    let props = match self.glyph_class((glyph)) {
      1 => GlyphPropsFlags::BASE_GLYPH.bits(),
      2 => GlyphPropsFlags::LIGATURE.bits(),
      3 => {
        let class = self.glyph_mark_attachment_class((glyph));
        (class << 8) | GlyphPropsFlags::MARK.bits()
      }
      _ => 0,
    };

    self.gdef_glyph_props_cache.set(
      (glyph),
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(props);
        migrated_value
      }),
    );

    props
  }

  #[inline(never)]
  pub fn is_mark_glyph_gdef(&self, glyph_id: u32, set_index: u16) -> bool {
    self
      .gdef
      .mark_sets
      .as_ref()
      .and_then(|(data, offsets)| {
        Some((
          data,
          ({
            let Ok(value) = offsets
              .get(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped_index(set_index);
                  migrated_value
                }),
              )?
              .get();
            value
          }),
        ))
      })
      .and_then(|(data, offset)| (offset).resolve::<CoverageTable>(*data).ok())
      .is_some_and(|coverage| coverage.get(glyph_id).is_some())
  }

  #[inline(always)]
  pub fn is_mark_glyph(&self, glyph_id: u32, set_index: u16) -> bool {
    match self
      .gdef_mark_set_digests
      .get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(set_index);
          migrated_value
        }),
      )
      .is_some_and(|digest| digest.may_have(glyph_id))
    {
      true => self.is_mark_glyph_gdef(glyph_id, set_index),
      false => false,
    }
  }

  pub fn table_data(&self, table_index: TableIndex) -> Option<&'a [u8]> {
    match table_index == TableIndex::GSUB {
      true => self.gsub.as_ref().map(|t| {
        let Ok(data) = t.table.offset_data();
        let Ok(bytes) = data.as_bytes();
        bytes
      }),
      false => self.gpos.as_ref().map(|t| {
        let Ok(data) = t.table.offset_data();
        let Ok(bytes) = data.as_bytes();
        bytes
      }),
    }
  }

  pub fn table_data_and_lookup(
    &self,
    table_index: TableIndex,
    lookup_index: u16,
  ) -> Option<(&'a [u8], &'a LookupInfo)> {
    match table_index == TableIndex::GSUB {
      true => {
        let table = self.gsub.as_ref()?;
        Some((
          {
            let Ok(value) = ({
              let Ok(value) = table.table.offset_data();
              value
            })
            .as_bytes();
            value
          },
          table.lookups.get(&table.table, lookup_index)?,
        ))
      }
      false => {
        let table = self.gpos.as_ref()?;
        Some((
          {
            let Ok(value) = ({
              let Ok(value) = table.table.offset_data();
              value
            })
            .as_bytes();
            value
          },
          table.lookups.get(&table.table, lookup_index)?,
        ))
      }
    }
  }

  pub(super) fn resolve_anchor(&self, anchor: &AnchorTable) -> (f32, f32) {
    let Ok(x_coordinate) = anchor.x_coordinate();
    let mut x = f32::from(x_coordinate);
    let Ok(y_coordinate) = anchor.y_coordinate();
    let mut y = f32::from(y_coordinate);
    match self.var_store.as_ref() {
      Some(vs) => {
        let delta = |val: Result<Option<DeviceOrVariationIndex<'_>>, ReadError>| match val {
          Ok(Some(DeviceOrVariationIndex::VariationIndex(varix))) => {
            ({
              let Ok(outer) = varix.delta_set_outer_index();
              let Ok(inner) = varix.delta_set_inner_index();
              let value = vs
                .compute_float_delta(DeltaSetIndex { outer, inner }, self.coords)
                .unwrap_or_default()
                .to_f64();
              value
            }) as f32
          }
          _ => 0.0,
        };
        x += delta(anchor.x_device());
        y += delta(anchor.y_device());
      }
      None => {}
    }
    (x, y)
  }
}

pub enum LayoutTable<'a> {
  Gsub(Gsub<'a>),
  Gpos(Gpos<'a>),
}

impl<'a> LayoutTable<'a> {
  fn script_list(&self) -> Option<ScriptList<'a>> {
    match self {
      Self::Gsub(gsub) => gsub.script_list().ok(),
      Self::Gpos(gpos) => gpos.script_list().ok(),
    }
  }

  fn feature_list(&self) -> Option<FeatureList<'a>> {
    match self {
      Self::Gsub(gsub) => gsub.feature_list().ok(),
      Self::Gpos(gpos) => gpos.feature_list().ok(),
    }
  }

  fn feature_variations(&self) -> Option<FeatureVariations<'a>> {
    match self {
      Self::Gsub(gsub) => gsub.feature_variations(),
      Self::Gpos(gpos) => gpos.feature_variations(),
    }
    .ok()
    .flatten()
  }

  fn script(&self, index: u16) -> Option<Script<'a>> {
    self
      .script_list()?
      .get(index)
      .ok()
      .map(|script| script.element)
  }

  fn langsys_index(&self, script_index: u16, tag: hb_tag_t) -> Option<u16> {
    let script = self.script(script_index)?;
    script.lang_sys_index_for_tag(tag)
  }

  fn langsys(&self, script_index: u16, langsys_index: Option<u16>) -> Option<LangSys<'a>> {
    let script = self.script(script_index)?;
    match langsys_index {
      Some(index) => {
        let Ok(lang_sys_records) = script.lang_sys_records();
        let record = lang_sys_records.get(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(index);
            migrated_value
          }),
        )?;
        record
          .lang_sys({
            let Ok(value) = script.offset_data();
            value
          })
          .ok()
      }
      None => script.default_lang_sys().ok().flatten(),
    }
  }

  pub(crate) fn feature(&self, index: u16) -> Option<Feature<'a>> {
    self
      .feature_list()?
      .get(index)
      .ok()
      .map(|feature| feature.element)
  }

  fn feature_tag(&self, index: u16) -> Option<hb_tag_t> {
    self
      .feature_list()?
      .get(index)
      .ok()
      .map(|feature| feature.tag)
  }

  pub(crate) fn feature_variation_index(&self, coords: &[F2Dot14]) -> Option<u32> {
    let feature_variations = self.feature_variations()?;
    let Ok(feature_variation_records) = feature_variations.feature_variation_records();

    for (index, rec) in feature_variation_records.iter().enumerate() {
      let Ok(condition_set_offset) = rec.condition_set_offset();

      match condition_set_offset.nullness() == Ok(read_fonts::types::Nullness::Null) {
        true => {
          return Some(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
              migrated_value
            }),
          );
        }
        false => {}
      }
      let Ok(Some(condition_set)) = rec.condition_set({
        let Ok(value) = feature_variations.offset_data();
        value
      }) else {
        continue;
      };

      if ({
        let Ok(conditions) = condition_set.conditions();
        let Ok(value) = conditions.iter();
        value
      })
      .filter_map(Result::ok)
      .all(|cond| match cond {
        Condition::Format1AxisRange(format1) => {
          let coord = coords
            .get(
              ({
                let Ok(axis_index) = format1.axis_index();
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped_index(axis_index);
                migrated_value
              }),
            )
            .copied()
            .unwrap_or_default();
          let Ok(filter_range_min_value) = format1.filter_range_min_value();

          coord >= filter_range_min_value
            && coord
              <= ({
                let Ok(explicit_value) = format1.filter_range_max_value();
                explicit_value
              })
        }
        _ => false,
      }) {
        return Some(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
            migrated_value
          }),
        );
      }
    }
    None
  }

  pub(crate) fn feature_substitution(
    &self,
    variation_index: u32,
    feature_index: u16,
  ) -> Option<Feature<'a>> {
    let feature_variations = self.feature_variations()?;
    let Ok(feature_variation_records) = feature_variations.feature_variation_records();
    let record = feature_variation_records.get(
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(variation_index);
        migrated_value
      }),
    )?;
    let subst_table = record
      .feature_table_substitution({
        let Ok(value) = feature_variations.offset_data();
        value
      })
      .ok()??;
    let Ok(subst_records) = subst_table.substitutions();
    match subst_records.binary_search_by_key(&feature_index, |subst| {
      let Ok(subst_feature_index) = subst.feature_index();

      subst_feature_index
    }) {
      Ok(ix) => Some(
        subst_records
          .get(ix)?
          .alternate_feature(
            ({
              let Ok(value) = subst_table.offset_data();
              value
            }),
          )
          .ok()?,
      ),
      _ => None,
    }
  }

  pub(crate) fn feature_index(&self, tag: hb_tag_t) -> Option<u16> {
    let list = self.feature_list()?;
    let Ok(feature_records) = list.feature_records();

    for (index, feature) in feature_records.iter().enumerate() {
      let Ok(feature_tag) = feature.feature_tag();

      match feature_tag == tag {
        true => {
          return Some(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
              migrated_value
            }),
          );
        }
        false => {}
      }
    }
    None
  }

  pub(crate) fn lookup_count(&self) -> u16 {
    match self {
      Self::Gsub(gsub) => {
        ({
          let explicit_value = gsub
            .lookup_list()
            .map(|list| {
              let Ok(lookup_count) = list.lookup_count();

              lookup_count
            })
            .unwrap_or_default();
          explicit_value
        })
      }
      Self::Gpos(gpos) => {
        ({
          let explicit_value = gpos
            .lookup_list()
            .map(|list| {
              let Ok(lookup_count) = list.lookup_count();

              lookup_count
            })
            .unwrap_or_default();
          explicit_value
        })
      }
    }
  }

  /// Returns true + index and tag of the first found script tag in the given GSUB or GPOS table
  /// or false + index and tag if falling back to a default script.
  pub(crate) fn select_script(&self, script_tags: &[hb_tag_t]) -> Option<(bool, u16, hb_tag_t)> {
    let selected = self.script_list()?.select(script_tags)?;
    Some((!selected.is_fallback, selected.index, selected.tag))
  }

  /// Returns the index of the first found language tag in the given GSUB or GPOS table,
  /// underneath the specified script index.
  pub(crate) fn select_script_language(
    &self,
    script_index: u16,
    lang_tags: &[hb_tag_t],
  ) -> Option<u16> {
    for &tag in lang_tags {
      match self.langsys_index(script_index, tag) {
        Some(index) => {
          return Some(index);
        }
        None => {}
      }
    }

    match self.langsys_index(script_index, hb_tag_t::default_language()) {
      Some(index) => {
        return Some(index);
      }
      None => {}
    }

    None
  }

  /// Returns the index and tag of a required feature in the given GSUB or GPOS table,
  /// underneath the specified script and language.
  pub(crate) fn get_required_language_feature(
    &self,
    script_index: u16,
    lang_index: Option<u16>,
  ) -> Option<(u16, hb_tag_t)> {
    let sys = self.langsys(script_index, lang_index)?;
    let Ok(idx) = sys.required_feature_index();
    match idx == 0xFFFF {
      true => {
        return None;
      }
      false => {}
    }
    let tag = self.feature_tag(idx)?;
    Some((idx, tag))
  }

  /// Returns the index of a given feature tag in the given GSUB or GPOS table,
  /// underneath the specified script and language.
  pub(crate) fn find_language_feature(
    &self,
    script_index: u16,
    lang_index: Option<u16>,
    feature_tag: hb_tag_t,
  ) -> Option<u16> {
    self
      .langsys(script_index, lang_index)?
      .feature_index_for_tag(&self.feature_list()?, feature_tag)
  }
}

fn coverage_index(coverage: Result<CoverageTable, ReadError>, gid: GlyphId) -> Option<u16> {
  coverage.ok().and_then(|coverage| coverage.get(gid))
}

fn coverage_index_cached(
  coverage: impl Fn(GlyphId) -> Option<u16>,
  gid: GlyphId,
  cache: &MappingCache,
) -> Option<u16> {
  match cache.get(gid.into()) {
    Some(index) => match index == MappingCache::MAX_VALUE {
      true => None,
      false => Some(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
          migrated_value
        }),
      ),
    },
    None => {
      let index = coverage(gid);
      match index {
        Some(index) => {
          match ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
            migrated_value
          }) < MappingCache::MAX_VALUE
          {
            true => {
              cache.set(
                gid.into(),
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u32>(index);
                  migrated_value
                }),
              );
            }
            false => {}
          }
          Some(index)
        }
        None => {
          cache.set(gid.into(), MappingCache::MAX_VALUE);
          None
        }
      }
    }
  }
}

fn coverage_binary_cached(
  coverage: impl Fn(GlyphId) -> Option<u16>,
  gid: GlyphId,
  cache: &BinaryCache,
) -> Option<bool> {
  match cache.get(gid.into()) {
    Some(index) => match index == BinaryCache::MAX_VALUE {
      true => None,
      false => Some(true),
    },
    None => {
      let index = coverage(gid);
      match index.is_some() {
        true => {
          cache.set(gid.into(), 0);
          Some(true)
        }
        false => {
          cache.set(gid.into(), BinaryCache::MAX_VALUE);
          None
        }
      }
    }
  }
}

fn covered(coverage: Result<CoverageTable, ReadError>, gid: GlyphId) -> bool {
  coverage_index(coverage, gid).is_some()
}

fn glyph_class(class_def: Result<ClassDef, ReadError>, gid: GlyphId) -> u16 {
  class_def
    .map(|class_def| class_def.get(gid))
    .unwrap_or_default()
}

fn glyph_class_cached(
  class_def: impl Fn(GlyphId) -> u16,
  gid: GlyphId,
  cache: &MappingCache,
) -> u16 {
  match cache.get(gid.into()) {
    Some(index) => {
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(index);
        migrated_value
      })
    }
    None => {
      let index = class_def(gid);
      cache.set(
        gid.into(),
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(index);
          migrated_value
        }),
      );
      index
    }
  }
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct CoverageInfo {
  pub offset: u16,
  pub format: u16,
  pub count: u16,
}

impl CoverageInfo {
  pub fn new(parent_data: &FontData, offset: u16) -> Option<Self> {
    match offset == 0 {
      true => {
        return None;
      }
      false => {}
    }
    let format = parent_data
      .read_at::<u16>(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(offset);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()?;
    match format != 1 && format != 2 {
      true => {
        return None;
      }
      false => {}
    }
    let count = parent_data
      .read_at::<u16>(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(offset);
              migrated_value
            }) + 2,
          );
          value
        }),
      )
      .ok()?;
    Some(Self {
      offset,
      format,
      count,
    })
  }

  pub fn index(&self, parent_data: &FontData, gid: GlyphId) -> Option<u16> {
    match self.offset == 0 {
      true => {
        return None;
      }
      false => {}
    }
    let gid = ({
      let Ok(value) = gid.to_u32();
      value
    });
    let data_offset = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.offset);
      migrated_value
    }) + 4;
    let len = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.count);
      migrated_value
    });
    match self.format == 1 {
      true => {
        let glyphs = parent_data
          .read_array::<BigEndian<GlyphId16>>(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(data_offset);
              value
            })..({
              let Ok(value) =
                console_core_number_conversion::fitted::<_, u64>(data_offset + len * 2);
              value
            }),
          )
          .ok()?;
        glyphs
          .binary_search_by_key(&gid, |g| {
            ({
              let Ok(value) = ({
                let Ok(value) = g.get();
                value
              })
              .to_u32();
              value
            })
          })
          .ok()
          .map(|idx| {
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(idx);
              migrated_value
            })
          })
      }
      false => {
        use core::cmp::Ordering;
        let records = parent_data
          .read_array::<RangeRecord>(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(data_offset);
              value
            })..({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                data_offset + len * size_of::<RangeRecord>(),
              );
              value
            }),
          )
          .ok()?;
        records
          .binary_search_by(|rec| {
            if ({
              let Ok(end_glyph_id) = rec.end_glyph_id();
              let Ok(value) = end_glyph_id.to_u32();
              value
            }) < (gid)
            {
              Ordering::Less
            } else if ({
              let Ok(value) = ({
                let Ok(explicit_value) = rec.start_glyph_id();
                explicit_value
              })
              .to_u32();
              value
            }) > (gid)
            {
              Ordering::Greater
            } else {
              Ordering::Equal
            }
          })
          .ok()
          .map(|idx| {
            let rec = &records[idx];
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
                (({
                  let Ok(start_coverage_index) = rec.start_coverage_index();
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, u32>(start_coverage_index);
                  migrated_value
                }) + gid
                  - ({
                    let Ok(start_glyph_id) = rec.start_glyph_id();
                    let Ok(value) = start_glyph_id.to_u32();
                    value
                  })),
              );
              migrated_value
            })
          })
      }
    }
  }
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct ClassDefInfo {
  pub offset: u16,
  pub format: u16,

  pub start_glyph_id: u16,
  pub count: u16,
}

impl ClassDefInfo {
  pub fn new(parent_data: &FontData, offset: u16) -> Option<Self> {
    match offset == 0 {
      true => {
        return None;
      }
      false => {}
    }
    let format = parent_data
      .read_at::<u16>(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(offset);
              migrated_value
            }),
          );
          value
        }),
      )
      .ok()?;
    match format != 1 && format != 2 {
      true => {
        return None;
      }
      false => {}
    }
    let (start_glyph_id, count) = match format == 1 {
      true => {
        let start_glyph_id = parent_data
          .read_at::<u16>(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index(offset);
                  migrated_value
                }) + 2,
              );
              value
            }),
          )
          .ok()?;
        let count = parent_data
          .read_at::<u16>(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index(offset);
                  migrated_value
                }) + 4,
              );
              value
            }),
          )
          .ok()?;
        (start_glyph_id, count)
      }
      false => match format == 2 {
        true => {
          let count = parent_data
            .read_at::<u16>(
              ({
                let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                  ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped_index(offset);
                    migrated_value
                  }) + 2,
                );
                value
              }),
            )
            .ok()?;
          (0, count)
        }
        false => {
          return None;
        }
      },
    };
    Some(Self {
      offset,
      format,
      start_glyph_id,
      count,
    })
  }

  pub fn class(&self, parent_data: &FontData, gid: GlyphId) -> u16 {
    let offset = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.offset);
      migrated_value
    });
    match offset == 0 {
      true => {
        return 0;
      }
      false => {}
    }
    let gid = ({
      let Ok(value) = gid.to_u32();
      value
    });
    match self.format == 1 {
      true => {
        let idx = match (gid).checked_sub(
          ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(self.start_glyph_id);
            migrated_value
          }),
        ) {
          Some(idx) => idx,
          None => {
            return 0;
          }
        };
        match idx
          >= ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u32>(self.count);
            migrated_value
          }) {
          true => {
            return 0;
          }
          false => {}
        }
        parent_data
          .read_at::<u16>(
            ({
              let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
                offset
                  + 6
                  + ({
                    let Ok(migrated_value) = console_core_number_conversion::wrapped_index(idx);
                    migrated_value
                  }) * 2,
              );
              value
            }),
          )
          .unwrap_or(0)
      }
      false => {
        use core::cmp::Ordering;
        let start = offset + 4;
        let end = start
          + ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.count);
            migrated_value
          }) * size_of::<ClassRangeRecord>();
        let records = match parent_data.read_array::<ClassRangeRecord>(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(start);
            value
          })..({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(end);
            value
          }),
        ) {
          Ok(records) => records,
          Err(_) => {
            return 0;
          }
        };
        records
          .binary_search_by(|rec| {
            if ({
              let Ok(end_glyph_id) = rec.end_glyph_id();
              let Ok(value) = end_glyph_id.to_u32();
              value
            }) < (gid)
            {
              Ordering::Less
            } else if ({
              let Ok(value) = ({
                let Ok(explicit_value) = rec.start_glyph_id();
                explicit_value
              })
              .to_u32();
              value
            }) > (gid)
            {
              Ordering::Greater
            } else {
              Ordering::Equal
            }
          })
          .ok()
          .map_or(0, |idx| {
            let Ok(class) = records[idx].class();

            class
          })
      }
    }
  }
}

use super::algs::HB_CODEPOINT_ENCODE3 as encode3;

/// Blocklist specific broken GDEF tables identified by the combination of
/// GDEF, GSUB, and GPOS table lengths. Nuke the GDEF tables to avoid
/// unwanted width-zeroing.
///
/// In certain versions of Times New Roman Italic and Bold Italic,
/// ASCII double quotation mark U+0022 has wrong glyph class 3 (mark)
/// in GDEF. Many versions of Tahoma have bad GDEF tables that
/// incorrectly classify some spacing marks such as certain IPA
/// symbols as glyph class 3. So do older versions of Microsoft
/// Himalaya, and the version of Cantarell shipped by Ubuntu 16.04.
///
/// See https://lists.freedesktop.org/archives/harfbuzz/2016-February/005489.html
///     https://bugzilla.mozilla.org/show_bug.cgi?id=1279925
///     https://bugzilla.mozilla.org/show_bug.cgi?id=1279693
///     https://bugzilla.mozilla.org/show_bug.cgi?id=1279875
///
/// Upstream: hb-ot-layout.cc OT::GDEF::is_blocklisted
fn is_gdef_blocklisted(gdef_len: u32, gsub_len: u32, gpos_len: u32) -> bool {
  const BLOCKLIST: &[u64] = &[
    encode3(442, 2874, 42038),
    encode3(430, 2874, 40662),
    encode3(442, 2874, 39116),
    encode3(430, 2874, 39374),
    encode3(490, 3046, 41638),
    encode3(478, 3046, 41902),
    encode3(898, 12554, 46470),
    encode3(910, 12566, 47732),
    encode3(928, 23298, 59332),
    encode3(940, 23310, 60732),
    encode3(964, 23836, 60072),
    encode3(976, 23832, 61456),
    encode3(994, 24474, 60336),
    encode3(1006, 24470, 61740),
    encode3(1006, 24576, 61346),
    encode3(1018, 24572, 62828),
    encode3(1006, 24576, 61352),
    encode3(1018, 24572, 62834),
    encode3(832, 7324, 47162),
    encode3(844, 7302, 45474),
    encode3(180, 13054, 7254),
    encode3(192, 12638, 7254),
    encode3(192, 12690, 7254),
    encode3(188, 248, 3852),
    encode3(188, 264, 3426),
    encode3(1058, 47032, 11818),
    encode3(1046, 47030, 12600),
    encode3(1058, 71796, 16770),
    encode3(1046, 71790, 17862),
    encode3(1046, 71788, 17112),
    encode3(1058, 71794, 17514),
    encode3(1330, 109_904, 57938),
    encode3(1330, 109_904, 58972),
    encode3(1004, 59092, 14836),
    encode3(588, 5078, 14418),
    encode3(588, 5078, 14238),
    encode3(894, 17162, 33960),
    encode3(894, 17154, 34472),
    encode3(816, 7868, 17052),
    encode3(816, 7868, 17138),
  ];
  let key = encode3(gdef_len, gsub_len, gpos_len);
  BLOCKLIST.contains(&key)
}
