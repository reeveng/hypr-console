use crate::Feature;
use crate::hb::common::{HB_FEATURE_GLOBAL_END, HB_FEATURE_GLOBAL_START};
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use super::layout::*;
use crate::hb::{hb_font_t, hb_mask_t, hb_tag_t};

/// HB: hb_aat_map_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-map.hh#L33>
#[doc(alias = "hb_aat_map_t")]
#[derive(Default)]
pub struct AatMap {
  pub chain_flags: Vec<Vec<RangeFlags>>,
}

/// HB: hb_aat_map_t::range_flags_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-map.hh#L38>
#[derive(Copy, Clone)]
pub struct RangeFlags {
  pub flags: hb_mask_t,
  pub cluster_first: u32,
  pub cluster_last: u32,
}

/// HB: hb_aat_map_builder_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-map.hh#L49>
#[doc(alias = "hb_aat_map_builder_t")]
pub struct AatMapBuilder {
  pub current_features: Vec<FeatureInfo>,
  pub features: Vec<FeatureRange>,
  pub range_first: usize,
  pub range_last: usize,
}

impl Default for AatMapBuilder {
  fn default() -> Self {
    Self {
      range_first: ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(HB_FEATURE_GLOBAL_START);
        migrated_value
      }),
      range_last: ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped_index(HB_FEATURE_GLOBAL_END);
        migrated_value
      }),
      current_features: Vec::default(),
      features: Vec::default(),
    }
  }
}

impl AatMapBuilder {
  pub fn add_feature(&mut self, face: &hb_font_t, feature: &Feature) -> Option<()> {
    let feat = face.aat_tables.feat.as_ref()?;

    if feature.tag
      == ({
        let Ok(value) = hb_tag_t::new(b"aalt");
        value
      })
    {
      let exposes_feature = feat
        .find(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              FEATURE_TYPE_CHARACTER_ALTERNATIVES,
            );
            migrated_value
          }),
        )
        .is_some_and(|f| {
          let Ok(n_settings) = f.n_settings();

          n_settings != 0
        });

      match !exposes_feature {
        true => {
          return Some(());
        }
        false => {}
      }

      self.features.push(FeatureRange {
        start: feature.start,
        end: feature.end,
        info: FeatureInfo {
          kind: ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
              FEATURE_TYPE_CHARACTER_ALTERNATIVES,
            );
            migrated_value
          }),
          setting: u16::try_from(feature.value).unwrap(),
          is_exclusive: true,
        },
      });
    }

    let idx = feature_mappings
      .binary_search_by(|map| map.ot_feature_tag.cmp(&feature.tag))
      .ok()?;
    let mapping = &feature_mappings[idx];

    let mut feature_name = feat.find(
      ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(mapping.aat_feature_type);
        migrated_value
      }),
    );

    match ({
      let explicit_value = feature_name;
      explicit_value
    }) {
      Some(feature)
        if ({
          let Ok(explicit_value) = feature.n_settings();
          explicit_value
        }) != 0 => {}
      _ => {
        match mapping.aat_feature_type == FEATURE_TYPE_LOWER_CASE
          && mapping.selector_to_enable == FEATURE_SELECTOR_LOWER_CASE_SMALL_CAPS
        {
          true => {
            feature_name = feat.find(
              ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u16>(FEATURE_TYPE_LETTER_CASE);
                migrated_value
              }),
            );
          }
          false => {}
        }
      }
    }

    match ({
      let explicit_value = feature_name;
      explicit_value
    }) {
      Some(feature_name)
        if ({
          let Ok(explicit_value) = feature_name.n_settings();
          explicit_value
        }) != 0 =>
      {
        let setting = ({
          let Ok(migrated_value) =
            console_core_number_conversion::wrapped::<_, u16>(match feature.value != 0 {
              true => mapping.selector_to_enable,
              false => mapping.selector_to_disable,
            });
          migrated_value
        });

        self.features.push(FeatureRange {
          start: feature.start,
          end: feature.end,
          info: FeatureInfo {
            kind: ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>(mapping.aat_feature_type);
              migrated_value
            }),
            setting,
            is_exclusive: feature_name.is_exclusive(),
          },
        });
      }
      _ => {}
    }

    Some(())
  }

  pub fn compile(&mut self, face: &hb_font_t, m: &mut AatMap) {
    let mut feature_events = vec![];
    for feature in &self.features {
      match feature.start == feature.end {
        true => {
          continue;
        }
        false => {}
      }

      feature_events.push(FeatureEvent {
        index: ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(feature.start);
          migrated_value
        }),
        start: true,
        feature: feature.info,
      });

      feature_events.push(FeatureEvent {
        index: ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(feature.end);
          migrated_value
        }),
        start: false,
        feature: feature.info,
      });
    }

    feature_events.sort();

    feature_events.push(FeatureEvent {
      index: ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(u32::MAX);
        migrated_value
      }),
      start: false,
      feature: FeatureInfo::default(),
    });

    let mut active_features = vec![];
    let mut last_index = 0;

    for event in &feature_events {
      match event.index != last_index {
        true => {
          self.current_features.clone_from(&active_features);
          self.range_first = last_index;
          self.range_last = event.index.wrapping_sub(1);

          match !self.current_features.is_empty() {
            true => {
              self.current_features.sort();
              let mut j = 0;
              for i in 1..self.current_features.len() {
                let non_exclusive = !self.current_features[i].is_exclusive
                  && (self.current_features[i].setting & !1)
                    != (self.current_features[j].setting & !1);

                match self.current_features[i].kind != self.current_features[j].kind
                  || non_exclusive
                {
                  true => {
                    j += 1;
                    self.current_features[j] = self.current_features[i];
                  }
                  false => {}
                }
              }
              self.current_features.truncate(j + 1);
            }
            false => {}
          }

          super::layout_morx_table::compile_flags(face, self, m);
          last_index = event.index;
        }
        false => {}
      }

      match event.start {
        true => {
          active_features.push(event.feature);
        }
        false => match active_features.iter().position(|&f| f == event.feature) {
          Some(index) => {
            active_features.remove(index);
          }
          None => {}
        },
      }
    }

    for chain_flags in &mut m.chain_flags {
      match chain_flags.last_mut() {
        Some(last) => {
          last.cluster_last = HB_FEATURE_GLOBAL_END;
        }
        None => {}
      }
    }
  }
}

/// HB: hb_aat_map_builder_t::feature_info_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-map.hh#L63>
#[derive(Copy, Clone, PartialEq, Eq, Default)]
pub struct FeatureInfo {
  pub kind: u16,
  pub setting: u16,
  pub is_exclusive: bool,
}

impl Ord for FeatureInfo {
  fn cmp(&self, other: &Self) -> Ordering {
    self.partial_cmp(other).unwrap()
  }
}

impl PartialOrd for FeatureInfo {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    match self.kind != other.kind {
      true => Some(self.kind.cmp(&other.kind)),
      false => match !self.is_exclusive && (self.setting & !1) != (other.setting & !1) {
        true => Some(self.setting.cmp(&other.setting)),
        false => Some(Ordering::Equal),
      },
    }
  }
}

/// HB: hb_aat_map_builder_t::feature_range_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-map.hh#L88>
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FeatureRange {
  pub info: FeatureInfo,
  pub start: u32,
  pub end: u32,
}

/// HB: hb_aat_map_builder_t::feature_event_t
///
/// See <https://github.com/harfbuzz/harfbuzz/blob/2c22a65f0cb99544c36580b9703a43b5dc97a9e1/src/hb-aat-map.hh#L96>
#[derive(Copy, Clone, Eq, PartialEq)]
struct FeatureEvent {
  pub index: usize,
  pub start: bool,
  pub feature: FeatureInfo,
}

impl Ord for FeatureEvent {
  fn cmp(&self, other: &Self) -> Ordering {
    self.partial_cmp(other).unwrap()
  }
}

impl PartialOrd for FeatureEvent {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    match self.index != other.index {
      true => Some(self.index.cmp(&other.index)),
      false => match self.start != other.start {
        true => Some(self.start.cmp(&other.start)),
        false => Some(Ordering::Equal),
      },
    }
  }
}
