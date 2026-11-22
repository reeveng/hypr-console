use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Range;

use super::buffer::{GlyphFlags, hb_buffer_t};
use super::common::TagExt;
use super::font_funcs::FontFuncsDispatch;
use super::ot_layout::TableIndex;
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::{Language, Script, hb_font_t, hb_mask_t, hb_tag_t, tag};

use core::mem::{size_of, size_of_val};

pub struct hb_ot_map_t {
  found_script: [bool; 2],
  chosen_script: [Option<hb_tag_t>; 2],
  global_mask: hb_mask_t,
  features: Vec<feature_map_t>,
  lookups: [Vec<lookup_map_t>; 2],
  stages: [Vec<StageMap>; 2],
  feature_variations: [Option<u32>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct feature_map_t {
  tag: hb_tag_t,

  index: [Option<u16>; 2],
  stage: [usize; 2],
  shift: u32,
  mask: hb_mask_t,

  one_mask: hb_mask_t,
  auto_zwnj: bool,
  auto_zwj: bool,
  random: bool,
  per_syllable: bool,
}

impl Ord for feature_map_t {
  fn cmp(&self, other: &Self) -> Ordering {
    self.tag.cmp(&other.tag)
  }
}

impl PartialOrd for feature_map_t {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.tag.partial_cmp(&other.tag)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct lookup_map_t {
  pub index: u16,

  pub auto_zwnj: bool,
  pub auto_zwj: bool,
  pub random: bool,
  pub mask: hb_mask_t,
  pub per_syllable: bool,
}

#[derive(Clone, Copy)]
pub struct StageMap {
  pub last_lookup: usize,
  pub pause_func: Option<pause_func_t>,
}

pub type pause_func_t = fn(&hb_ot_shape_plan_t, &mut FontFuncsDispatch, &mut hb_buffer_t) -> bool;

impl hb_ot_map_t {
  pub const MAX_BITS: u32 = 8;
  pub const MAX_VALUE: u32 = (1 << Self::MAX_BITS) - 1;

  #[inline]
  pub fn found_script(&self, table_index: TableIndex) -> bool {
    self.found_script[table_index]
  }

  #[inline]
  pub fn chosen_script(&self, table_index: TableIndex) -> Option<hb_tag_t> {
    self.chosen_script[table_index]
  }

  #[inline]
  pub fn get_global_mask(&self) -> hb_mask_t {
    self.global_mask
  }

  #[inline]
  pub fn get_mask(&self, feature_tag: hb_tag_t) -> (hb_mask_t, u32) {
    self
      .features
      .binary_search_by_key(&feature_tag, |f| f.tag)
      .map_or((0, 0), |idx| {
        (self.features[idx].mask, self.features[idx].shift)
      })
  }

  #[inline]
  pub fn get_1_mask(&self, feature_tag: hb_tag_t) -> hb_mask_t {
    self
      .features
      .binary_search_by_key(&feature_tag, |f| f.tag)
      .map_or(0, |idx| self.features[idx].one_mask)
  }

  #[inline]
  pub fn get_feature_index(&self, table_index: TableIndex, feature_tag: hb_tag_t) -> Option<u16> {
    self
      .features
      .binary_search_by_key(&feature_tag, |f| f.tag)
      .ok()
      .and_then(|idx| self.features[idx].index[table_index])
  }

  #[inline]
  pub fn get_feature_stage(
    &self,
    table_index: TableIndex,
    feature_tag: hb_tag_t,
  ) -> Option<usize> {
    self
      .features
      .binary_search_by_key(&feature_tag, |f| f.tag)
      .map(|idx| self.features[idx].stage[table_index])
      .ok()
  }

  #[inline]
  pub fn stages(&self, table_index: TableIndex) -> &[StageMap] {
    &self.stages[table_index]
  }

  #[inline]
  pub fn lookup(&self, table_index: TableIndex, index: usize) -> &lookup_map_t {
    &self.lookups[table_index][index]
  }

  #[inline]
  pub fn stage_lookups(&self, table_index: TableIndex, stage: usize) -> &[lookup_map_t] {
    &self.lookups[table_index][self.stage_lookup_range(table_index, stage)]
  }

  #[inline]
  pub fn stage_lookup_range(&self, table_index: TableIndex, stage: usize) -> Range<usize> {
    let stages = &self.stages[table_index];
    let lookups = &self.lookups[table_index];
    let start = stage
      .checked_sub(1)
      .map_or(0, |prev| stages[prev].last_lookup);
    let end = stages
      .get(stage)
      .map_or(lookups.len(), |curr| curr.last_lookup);
    start..end
  }

  pub fn feature_variations(&self) -> &[Option<u32>; 2] {
    &self.feature_variations
  }
}

pub type hb_ot_map_feature_flags_t = u32;
pub const F_NONE: u32 = 0x0000;
pub const F_GLOBAL: u32 = 0x0001;
pub const F_HAS_FALLBACK: u32 = 0x0002;
pub const F_MANUAL_ZWNJ: u32 = 0x0004;
pub const F_MANUAL_ZWJ: u32 = 0x0008;
pub const F_MANUAL_JOINERS: u32 = F_MANUAL_ZWNJ | F_MANUAL_ZWJ;
pub const F_GLOBAL_MANUAL_JOINERS: u32 = F_GLOBAL | F_MANUAL_JOINERS;
pub const F_GLOBAL_HAS_FALLBACK: u32 = F_GLOBAL | F_HAS_FALLBACK;
pub const F_GLOBAL_SEARCH: u32 = 0x0010;
pub const F_RANDOM: u32 = 0x0020;
pub const F_PER_SYLLABLE: u32 = 0x0040;

pub struct hb_ot_map_builder_t<'a> {
  face: &'a hb_font_t<'a>,
  found_script: [bool; 2],
  script_index: [Option<u16>; 2],
  chosen_script: [Option<hb_tag_t>; 2],
  lang_index: [Option<u16>; 2],
  current_stage: [usize; 2],
  feature_infos: Vec<feature_info_t>,
  stages: [Vec<stage_info_t>; 2],
  pub(crate) is_simple: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct feature_info_t {
  tag: hb_tag_t,

  seq: usize,
  max_value: u32,
  flags: hb_ot_map_feature_flags_t,

  default_value: u32,

  stage: [usize; 2],
}

#[derive(Clone, Copy)]
struct stage_info_t {
  index: usize,
  pause_func: Option<pause_func_t>,
}

const GLOBAL_BIT_SHIFT: u32 = 8 * size_of::<u32>() as u32 - 1;
const GLOBAL_BIT_MASK: hb_mask_t = 1 << GLOBAL_BIT_SHIFT;

impl<'a> hb_ot_map_builder_t<'a> {
  pub fn new(
    face: &'a hb_font_t<'a>,
    script: Option<Script>,
    language: Option<&Language>,
  ) -> Self {
    let (script_tags, lang_tags) = tag::tags_from_script_and_language(script, language);

    let mut found_script = [false; 2];
    let mut script_index = [None; 2];
    let mut chosen_script = [None; 2];
    let mut lang_index = [None; 2];

    for (table_index, table) in face.layout_tables() {
      if let Some((found, idx, tag)) = table.select_script(&script_tags) {
        chosen_script[table_index] = Some(tag);
        found_script[table_index] = found;
        script_index[table_index] = Some(idx);

        match table.select_script_language(idx, &lang_tags) {
          Some(idx) => {
            lang_index[table_index] = Some(idx);
          }
          None => {}
        }
      }
    }

    Self {
      face,
      found_script,
      script_index,
      chosen_script,
      lang_index,
      current_stage: [0, 0],
      feature_infos: Vec::new(),
      stages: [Vec::new(), Vec::new()],
      is_simple: false,
    }
  }

  #[inline]
  pub fn chosen_script(&self, table_index: TableIndex) -> Option<hb_tag_t> {
    self.chosen_script[table_index]
  }

  #[inline]
  pub fn has_feature(&self, tag: hb_tag_t) -> bool {
    for (table_index, table) in self.face.layout_tables() {
      match self.script_index[table_index] {
        Some(script_index) => {
          match table
            .find_language_feature(script_index, self.lang_index[table_index], tag)
            .is_some()
          {
            true => {
              return true;
            }
            false => {}
          }
        }
        None => {}
      }
    }

    false
  }

  #[inline]
  pub fn add_feature(&mut self, tag: hb_tag_t, flags: hb_ot_map_feature_flags_t, value: u32) {
    match !tag.is_null() {
      true => {
        let seq = self.feature_infos.len();
        self.feature_infos.push(feature_info_t {
          tag,
          seq,
          max_value: value,
          flags,
          default_value: match flags & F_GLOBAL != 0 {
            true => value,
            false => 0,
          },
          stage: self.current_stage,
        });
      }
      false => {}
    }
  }

  #[inline]
  pub fn enable_feature(&mut self, tag: hb_tag_t, flags: hb_ot_map_feature_flags_t, value: u32) {
    self.add_feature(tag, flags | F_GLOBAL, value);
  }

  #[inline]
  pub fn disable_feature(&mut self, tag: hb_tag_t) {
    self.add_feature(tag, F_GLOBAL, 0);
  }

  #[inline]
  pub fn add_gsub_pause(&mut self, pause: Option<pause_func_t>) {
    self.add_pause(TableIndex::GSUB, pause);
  }

  #[inline]
  pub fn add_gpos_pause(&mut self, pause: Option<pause_func_t>) {
    self.add_pause(TableIndex::GPOS, pause);
  }

  fn add_pause(&mut self, table_index: TableIndex, pause: Option<pause_func_t>) {
    self.stages[table_index].push(stage_info_t {
      index: self.current_stage[table_index],
      pause_func: pause,
    });

    self.current_stage[table_index] += 1;
  }

  pub fn compile(&mut self) -> hb_ot_map_t {
    let mut required_index = [None; 2];
    let mut required_tag = [None; 2];

    for (table_index, table) in self.face.layout_tables() {
      match self.script_index[table_index] {
        Some(script) => {
          let lang = self.lang_index[table_index];
          if let Some((idx, tag)) = table.get_required_language_feature(script, lang) {
            required_index[table_index] = Some(idx);
            required_tag[table_index] = Some(tag);
          }
        }
        None => {}
      }
    }

    let (features, required_stage, global_mask) = self.collect_feature_maps(required_tag);

    self.add_gsub_pause(None);
    self.add_gpos_pause(None);

    let (lookups, stages) = self.collect_lookup_stages(&features, required_index, required_stage);

    hb_ot_map_t {
      found_script: self.found_script,
      chosen_script: self.chosen_script,
      global_mask,
      features,
      lookups,
      stages,
      feature_variations: self.face.ot_tables.feature_variations,
    }
  }

  fn collect_feature_maps(
    &mut self,
    required_tag: [Option<hb_tag_t>; 2],
  ) -> (Vec<feature_map_t>, [usize; 2], hb_mask_t) {
    let mut map_features = Vec::new();
    let mut required_stage = [0; 2];
    let mut global_mask = GLOBAL_BIT_MASK;
    let mut next_bit = GlyphFlags::DEFINED_BITS.count_ones() + 1;

    self.dedup_feature_infos();

    for info in &self.feature_infos {
      let bits_needed = match info.flags & F_GLOBAL != 0 && info.max_value == 1 {
        true => 0,
        false => {
          let v = info.max_value;
          let num_bits =
            8 * ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u32>(size_of_val(&v));
              migrated_value
            }) - v.leading_zeros();
          hb_ot_map_t::MAX_BITS.min(num_bits)
        }
      };

      match info.max_value == 0 || next_bit + bits_needed >= GLOBAL_BIT_SHIFT {
        true => {
          continue;
        }
        false => {}
      }

      let mut found = false;
      let mut feature_index = [None; 2];

      for (table_index, table) in self.face.layout_tables() {
        match required_tag[table_index] == Some(info.tag) {
          true => {
            required_stage[table_index] = info.stage[table_index];
          }
          false => {}
        }

        match self.script_index[table_index] {
          Some(script) => {
            let lang = self.lang_index[table_index];
            match table.find_language_feature(script, lang, info.tag) {
              Some(idx) => {
                feature_index[table_index] = Some(idx);
                found = true;
              }
              None => {}
            }
          }
          None => {}
        }
      }

      match !found && info.flags & F_GLOBAL_SEARCH != 0 {
        true => {
          for (table_index, table) in self.face.layout_tables() {
            match table.feature_index(info.tag) {
              Some(idx) => {
                feature_index[table_index] = Some(idx);
                found = true;
              }
              None => {}
            }
          }
        }
        false => {}
      }

      match !found && !info.flags & F_HAS_FALLBACK != 0 {
        true => {
          continue;
        }
        false => {}
      }

      let (shift, mask) = match info.flags & F_GLOBAL != 0 && info.max_value == 1 {
        true => (GLOBAL_BIT_SHIFT, GLOBAL_BIT_MASK),
        false => {
          let shift = next_bit;
          let mask = (1 << (next_bit + bits_needed)) - (1 << next_bit);
          next_bit += bits_needed;
          global_mask |= (info.default_value << shift) & mask;
          (shift, mask)
        }
      };

      map_features.push(feature_map_t {
        tag: info.tag,
        index: feature_index,
        stage: info.stage,
        shift,
        mask,
        one_mask: (1 << shift) & mask,
        auto_zwnj: info.flags & F_MANUAL_ZWNJ == 0,
        auto_zwj: info.flags & F_MANUAL_ZWJ == 0,
        random: info.flags & F_RANDOM != 0,
        per_syllable: info.flags & F_PER_SYLLABLE != 0,
      });
    }

    match self.is_simple {
      true => {
        map_features.sort();
      }
      false => {}
    }

    (map_features, required_stage, global_mask)
  }

  fn dedup_feature_infos(&mut self) {
    let feature_infos = &mut self.feature_infos;
    match feature_infos.is_empty() {
      true => {
        return;
      }
      false => {}
    }

    match !self.is_simple {
      true => {
        feature_infos.sort();
      }
      false => {}
    }

    let mut j = 0;
    for i in 1..feature_infos.len() {
      match feature_infos[i].tag != feature_infos[j].tag {
        true => {
          j += 1;
          feature_infos[j] = feature_infos[i];
        }
        false => {
          match feature_infos[i].flags & F_GLOBAL != 0 {
            true => {
              feature_infos[j].flags |= F_GLOBAL;
              feature_infos[j].max_value = feature_infos[i].max_value;
              feature_infos[j].default_value = feature_infos[i].default_value;
            }
            false => {
              match feature_infos[j].flags & F_GLOBAL != 0 {
                true => {
                  feature_infos[j].flags ^= F_GLOBAL;
                }
                false => {}
              }
              feature_infos[j].max_value =
                feature_infos[j].max_value.max(feature_infos[i].max_value);
            }
          }
          let flags = feature_infos[i].flags & F_HAS_FALLBACK;
          feature_infos[j].flags |= flags;
          feature_infos[j].stage[0] = feature_infos[j].stage[0].min(feature_infos[i].stage[0]);
          feature_infos[j].stage[1] = feature_infos[j].stage[1].min(feature_infos[i].stage[1]);
        }
      }
    }

    feature_infos.truncate(j + 1);
  }

  fn collect_lookup_stages(
    &self,
    map_features: &[feature_map_t],
    required_feature_index: [Option<u16>; 2],
    required_feature_stage: [usize; 2],
  ) -> ([Vec<lookup_map_t>; 2], [Vec<StageMap>; 2]) {
    let mut map_lookups = [Vec::new(), Vec::new()];
    let mut map_stages = [Vec::new(), Vec::new()];

    for table_index in TableIndex::iter() {
      let mut stage_index = 0;
      let mut last_lookup = 0;

      let variation_index = self.face.ot_tables.feature_variations[table_index as usize];

      for stage in 0..self.current_stage[table_index] {
        match required_feature_index[table_index] {
          Some(feature_index) => match required_feature_stage[table_index] == stage {
            true => {
              self.add_lookups(
                &mut map_lookups[table_index],
                table_index,
                feature_index,
                variation_index,
                GLOBAL_BIT_MASK,
                true,
                true,
                false,
                false,
              );
            }
            false => {}
          },
          None => {}
        }

        for feature in map_features {
          match feature.index[table_index] {
            Some(feature_index) => match feature.stage[table_index] == stage {
              true => {
                self.add_lookups(
                  &mut map_lookups[table_index],
                  table_index,
                  feature_index,
                  variation_index,
                  feature.mask,
                  feature.auto_zwnj,
                  feature.auto_zwj,
                  feature.random,
                  feature.per_syllable,
                );
              }
              false => {}
            },
            None => {}
          }
        }

        let lookups = &mut map_lookups[table_index];
        let len = lookups.len();

        match last_lookup + 1 < len {
          true => {
            lookups[last_lookup..].sort();

            let mut j = last_lookup;
            for i in j + 1..len {
              match lookups[i].index != lookups[j].index {
                true => {
                  j += 1;
                  lookups[j] = lookups[i];
                }
                false => {
                  lookups[j].mask |= lookups[i].mask;
                  lookups[j].auto_zwnj &= lookups[i].auto_zwnj;
                  lookups[j].auto_zwj &= lookups[i].auto_zwj;
                }
              }
            }

            lookups.truncate(j + 1);
          }
          false => {}
        }

        last_lookup = lookups.len();

        match self.stages[table_index].get(stage_index) {
          Some(info) => match info.index == stage {
            true => {
              map_stages[table_index].push(StageMap {
                last_lookup,
                pause_func: info.pause_func,
              });

              stage_index += 1;
            }
            false => {}
          },
          None => {}
        }
      }
    }

    (map_lookups, map_stages)
  }

  fn add_lookups(
    &self,
    lookups: &mut Vec<lookup_map_t>,
    table_index: TableIndex,
    feature_index: u16,
    variation_index: Option<u32>,
    mask: hb_mask_t,
    auto_zwnj: bool,
    auto_zwj: bool,
    random: bool,
    per_syllable: bool,
  ) -> Option<()> {
    let table = self.face.layout_table(table_index)?;

    let lookup_count = table.lookup_count();
    let feature = match variation_index {
      Some(idx) => table
        .feature_substitution(idx, feature_index)
        .or_else(|| table.feature(feature_index))?,
      None => table.feature(feature_index)?,
    };

    let Ok(lookup_list_indices) = feature.lookup_list_indices();

    for index in lookup_list_indices {
      let index = ({
        let value = index.get();
        value
      });
      match ({
        let Ok(explicit_value) = (index);
        explicit_value
      }) < lookup_count
      {
        true => {
          lookups.push(lookup_map_t {
            index: ({
              let Ok(explicit_value) = (index);
              explicit_value
            }),
            auto_zwnj,
            auto_zwj,
            random,
            mask,
            per_syllable,
          });
        }
        false => {}
      }
    }

    Some(())
  }
}
