//! Shaping support for autohinting.

use super::style::{GlyphStyle, StyleClass};
use crate::{FontRef, GlyphId, MetadataProvider, charmap::Charmap};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use raw::{
  ReadError, TableProvider,
  tables::{
    gsub::{
      ChainedSequenceContext, Gsub, SequenceContext, SingleSubst, SubstitutionLookupList,
      SubstitutionSubtables,
    },
    layout::{Feature, ScriptTags},
    varc::CoverageTable,
  },
  types::Tag,
};

const MAX_NESTING_DEPTH: usize = 64;

/// Determines the fidelity with which we apply shaping in the
/// autohinter.
///
/// Shaping only affects glyph style classification and the glyphs that
/// are chosen for metrics computations. We keep the `Nominal` mode around
/// to enable validation of internal algorithms against a configuration that
/// is known to match FreeType. The `BestEffort` mode should always be
/// used for actual rendering.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ShaperMode {
  /// Characters are mapped to nominal glyph identifiers and layout tables
  /// are not used for style coverage.
  ///
  /// This matches FreeType when HarfBuzz support is not enabled.
  Nominal,
  /// Simple substitutions are applied according to script rules and layout
  /// tables are used to extend style coverage beyond the character map.
  #[allow(unused)]
  BestEffort,
}

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct ShapedGlyph {
  pub id: GlyphId,
  /// This may be used for computing vertical alignment zones, particularly
  /// for glyphs like super/subscripts which might have adjustments in GPOS.
  ///
  /// Note that we don't do the same in the horizontal direction which
  /// means that we don't care about the x-offset.
  pub y_offset: i32,
}

/// Arbitrarily chosen to cover our max input size plus some extra to account
/// for expansion from multiple substitution tables.

/// Container for storing the result of shaping a cluster.
///
/// Some of our input "characters" for metrics computations are actually
/// multi-character [grapheme clusters](https://www.unicode.org/reports/tr29/#Grapheme_Cluster_Boundaries)
/// that may expand to multiple glyphs.
pub(crate) type ShapedCluster = Vec<ShapedGlyph>;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum ShaperCoverageKind {
  /// Shaper coverage that traverses a specific script.
  Script,
  /// Shaper coverage that also includes the `Dflt` script.
  ///
  /// This is used as a catch all after all styles are processed.
  Default,
}

/// Maps characters to glyphs and handles extended style coverage beyond
/// glyphs that are available in the character map.
///
/// Roughly covers the functionality in <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afshaper.c>.
pub(crate) struct Shaper<'a> {
  font: FontRef<'a>,
  #[allow(unused)]
  mode: ShaperMode,
  charmap: Charmap<'a>,
  gsub: Option<Gsub<'a>>,
}

impl<'a> Shaper<'a> {
  pub fn new(font: &FontRef<'a>, mode: ShaperMode) -> Self {
    let charmap = font.charmap();
    let gsub = (mode != ShaperMode::Nominal)
      .then(|| font.gsub().ok())
      .flatten();
    Self {
      font: font.clone(),
      mode,
      charmap,
      gsub,
    }
  }

  pub fn font(&self) -> &FontRef<'a> {
    &self.font
  }

  pub fn charmap(&self) -> &Charmap<'a> {
    &self.charmap
  }

  pub fn lookup_count(&self) -> u16 {
    ({
      let explicit_value = self
        .gsub
        .as_ref()
        .and_then(|gsub| gsub.lookup_list().ok())
        .map(|list| {
          let Ok(lookup_count) = list.lookup_count();

          lookup_count
        })
        .unwrap_or_default();
      explicit_value
    })
  }

  pub fn cluster_shaper(&'a self, style: &StyleClass) -> ClusterShaper<'a> {
    match self.mode == ShaperMode::BestEffort {
      true => match style.feature {
        Some(feature_tag) => {
          if let Some((lookup_list, feature)) = self.gsub.as_ref().and_then(|gsub| {
            let script_list = gsub.script_list().ok()?;
            let selected_script =
              script_list.select(&ScriptTags::from_unicode(style.script.tag))?;
            let script = (script_list.get(selected_script.index)).ok()?;
            let lang_sys = script.default_lang_sys().ok()??;
            let feature_list = gsub.feature_list().ok()?;
            let feature_ix = lang_sys.feature_index_for_tag(&feature_list, feature_tag)?;
            let feature = (feature_list.get(feature_ix)).ok()?.element;
            let lookup_list = gsub.lookup_list().ok()?;
            Some((lookup_list, feature))
          }) {
            return ClusterShaper {
              shaper: self,
              lookup_list: Some(lookup_list),
              kind: ClusterShaperKind::SingleFeature(feature),
            };
          }
        }
        None => {}
      },
      false => {}
    }
    ClusterShaper {
      shaper: self,
      lookup_list: None,
      kind: ClusterShaperKind::Nominal,
    }
  }

  /// Uses layout tables to compute coverage for the given style.
  ///
  /// Returns `true` if any glyph styles were updated for this style.
  ///
  /// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afshaper.c#L99>
  pub(crate) fn compute_coverage(
    &self,
    style: &StyleClass,
    coverage_kind: ShaperCoverageKind,
    glyph_styles: &mut [GlyphStyle],
    visited_set: &mut VisitedLookupSet<'_>,
  ) -> bool {
    let gsub = match self.gsub.as_ref() {
      Some(gsub) => gsub,
      None => {
        return false;
      }
    };
    let (Ok(script_list), Ok(feature_list), Ok(lookup_list)) =
      (gsub.script_list(), gsub.feature_list(), gsub.lookup_list())
    else {
      return false;
    };
    let mut script_tags: [Option<Tag>; 3] = [None; 3];
    for (a, b) in script_tags
      .iter_mut()
      .zip(ScriptTags::from_unicode(style.script.tag).iter())
    {
      *a = Some(*b);
    }

    const DEFAULT_SCRIPT: Tag = ({
      let Ok(value) = Tag::new(b"DFLT");
      value
    });
    match coverage_kind == ShaperCoverageKind::Default {
      true => match script_tags[0].is_none() {
        true => {
          script_tags[0] = Some(DEFAULT_SCRIPT);
        }
        false => match script_tags[1].is_none() {
          true => {
            script_tags[1] = Some(DEFAULT_SCRIPT);
          }
          false => match script_tags[1] != Some(DEFAULT_SCRIPT) {
            true => {
              script_tags[2] = Some(DEFAULT_SCRIPT);
            }
            false => {}
          },
        },
      },
      false => {
        const NON_STANDARD_TAGS: &[Option<Tag>] = &[
          Some({
            let Ok(value) = Tag::new(b"Khms");
            value
          }),
          Some({
            let Ok(value) = Tag::new(b"Latb");
            value
          }),
          Some({
            let Ok(value) = Tag::new(b"Latp");
            value
          }),
        ];
        match NON_STANDARD_TAGS.contains(&script_tags[0]) {
          true => {
            return false;
          }
          false => {}
        }
      }
    }

    let mut gsub_handler = GsubHandler::new(
      &self.charmap,
      &lookup_list,
      style,
      glyph_styles,
      visited_set,
    );
    for script in script_tags.iter().filter_map(|tag| {
      tag
        .and_then(|tag| script_list.index_for_tag(tag))
        .and_then(|ix| {
          let Ok(script_records) = script_list.script_records();

          (script_records.get(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(ix);
              migrated_value
            }),
          ))
        })
        .and_then(|rec| {
          rec
            .script({
              let Ok(value) = script_list.offset_data();
              value
            })
            .ok()
        })
    }) {
      let Ok(lang_sys_records) = script.lang_sys_records();

      for langsys in lang_sys_records
        .iter()
        .filter_map(|rec| {
          rec
            .lang_sys({
              let Ok(value) = script.offset_data();
              value
            })
            .ok()
        })
        .chain(script.default_lang_sys().ok().flatten())
      {
        let Ok(feature_indices) = langsys.feature_indices();

        for feature_ix in feature_indices {
          let Ok(feature_records) = feature_list.feature_records();
          let feature = match feature_records
            .get(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
                  ({
                    let Ok(explicit_value) = ({
                      let value = feature_ix.get();
                      value
                    });
                    explicit_value
                  }),
                );
                migrated_value
              }),
            )
            .and_then(|rec| {
              let Ok(feature_tag) = rec.feature_tag();

              match style.feature == Some(feature_tag) || style.feature.is_none() {
                true => rec
                  .feature(
                    ({
                      let Ok(value) = feature_list.offset_data();
                      value
                    }),
                  )
                  .ok(),
                false => None,
              }
            }) {
            Some(feature) => feature,
            None => {
              continue;
            }
          };

          let Ok(lookup_list_indices) = feature.lookup_list_indices();

          for index in lookup_list_indices.iter() {
            let _ = gsub_handler.process_lookup(
              ({
                let Ok(value) = index.get();
                value
              }),
            );
          }
        }
      }
    }
    match gsub_handler.finish() {
      Some(range) => {
        let mut result = false;
        for glyph_style in &mut glyph_styles[range] {
          result |= glyph_style.maybe_assign_gsub_output_style(style);
        }
        result
      }
      None => false,
    }
  }
}

pub(crate) struct ClusterShaper<'a> {
  shaper: &'a Shaper<'a>,
  lookup_list: Option<SubstitutionLookupList<'a>>,
  kind: ClusterShaperKind<'a>,
}

impl ClusterShaper<'_> {
  pub(crate) fn shape(&mut self, input: &str, output: &mut ShapedCluster) {
    output.clear();
    for ch in input.chars() {
      output.push(ShapedGlyph {
        id: self.shaper.charmap.map(ch).unwrap_or_default(),
        y_offset: 0,
      });
    }
    match self.kind.clone() {
      ClusterShaperKind::Nominal => {
        match self.shaper.mode == ShaperMode::Nominal && output.len() > 1 {
          true => {
            output.clear();
          }
          false => {}
        }
      }
      ClusterShaperKind::SingleFeature(feature) => {
        let mut did_subst = false;
        let Ok(lookup_list_indices) = feature.lookup_list_indices();

        for lookup_ix in lookup_list_indices {
          let mut glyph_ix = 0;
          while glyph_ix < output.len() {
            did_subst |= self.apply_lookup(
              {
                let Ok(value) = lookup_ix.get();
                value
              },
              output,
              glyph_ix,
              0,
            );
            glyph_ix += 1;
          }
        }

        match !did_subst {
          true => {
            output.clear();
          }
          false => {}
        }
      }
    }
  }

  fn apply_lookup(
    &self,
    lookup_index: u16,
    cluster: &mut ShapedCluster,
    glyph_ix: usize,
    nesting_depth: usize,
  ) -> bool {
    match nesting_depth > MAX_NESTING_DEPTH {
      true => {
        return false;
      }
      false => {}
    }
    let glyph = match cluster.get_mut(glyph_ix) {
      Some(glyph) => glyph,
      None => {
        return false;
      }
    };
    let subtables = match self
      .lookup_list
      .as_ref()
      .and_then(|list| {
        let Ok(lookups) = list.lookups();

        (lookups.get(
          ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
              ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped_index(lookup_index);
                migrated_value
              }),
            );
            value
          }),
        ))
        .ok()
      })
      .and_then(|lookup| lookup.subtables().ok())
    {
      Some(subtables) => subtables,
      None => {
        return false;
      }
    };
    match subtables {
      SubstitutionSubtables::Single(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          match table {
            SingleSubst::Format1(table) => {
              let () = match table.coverage().ok().and_then(|cov| (cov.get(glyph.id))) {
                Some(_) => (),
                None => {
                  continue;
                }
              };
              let delta = ({
                let Ok(delta_glyph_id) = table.delta_glyph_id();
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(delta_glyph_id);
                migrated_value
              });
              glyph.id = GlyphId::from(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
                    (({
                      let Ok(migrated_value) =
                        console_core_number_conversion::wrapped::<_, i32>({
                          let Ok(value) = glyph.id.to_u32();
                          value
                        });
                      migrated_value
                    }) + delta),
                  );
                  migrated_value
                }),
              );
              return true;
            }
            SingleSubst::Format2(table) => {
              let cov_ix = match table.coverage().ok().and_then(|cov| (cov.get(glyph.id))) {
                Some(cov_ix) => cov_ix,
                None => {
                  continue;
                }
              };
              let Ok(substitute_glyph_ids) = table.substitute_glyph_ids();
              let subst = match (substitute_glyph_ids.get(
                ({
                  let Ok(migrated_value) = console_core_number_conversion::wrapped_index(cov_ix);
                  migrated_value
                }),
              )) {
                Some(subst) => subst,
                None => {
                  continue;
                }
              };
              glyph.id = ({
                let Ok(value) = subst.get();
                value
              })
              .into();
              return true;
            }
          }
        }
      }
      SubstitutionSubtables::Multiple(_tables) => {}
      SubstitutionSubtables::Ligature(_tables) => {}
      SubstitutionSubtables::Alternate(_tables) => {}
      SubstitutionSubtables::Contextual(_tables) => {}
      SubstitutionSubtables::ChainContextual(_tables) => {}
      SubstitutionSubtables::Reverse(_tables) => {}
      SubstitutionSubtables::EmptyExtension => {}
    }
    false
  }
}

#[derive(Clone)]
enum ClusterShaperKind<'a> {
  Nominal,
  SingleFeature(Feature<'a>),
}

/// Captures glyphs from the GSUB table that aren't present in cmap.
///
/// FreeType does this in a few phases:
/// 1. Collect all lookups for a given set of scripts and features.
///    <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afshaper.c#L174>
/// 2. For each lookup, collect all _output_ glyphs.
///    <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afshaper.c#L201>
/// 3. If the style represents a specific feature, make sure at least one of
///    the characters in the associated blue string would be substituted by
///    those lookups. If none would be substituted, then we don't assign the
///    style to any glyphs because we don't have any modified alignment
///    zones.
///    <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afshaper.c#L264>
///
/// We roll these into one pass over the lookups below so that we don't have
/// to allocate a lookup set or iterate them twice. Note that since
/// substitutions are checked for individual characters, we ignore ligatures
/// and contextual lookups (and alternates since they aren't applicable).
struct GsubHandler<'a, 'b> {
  charmap: &'a Charmap<'a>,
  lookup_list: &'a SubstitutionLookupList<'a>,
  style: &'a StyleClass,
  glyph_styles: &'a mut [GlyphStyle],

  need_blue_substs: bool,

  min_gid: usize,
  max_gid: usize,
  lookup_depth: usize,
  visited_set: &'a mut VisitedLookupSet<'b>,
}

impl<'a, 'b> GsubHandler<'a, 'b> {
  fn new(
    charmap: &'a Charmap<'a>,
    lookup_list: &'a SubstitutionLookupList,
    style: &'a StyleClass,
    glyph_styles: &'a mut [GlyphStyle],
    visited_set: &'a mut VisitedLookupSet<'b>,
  ) -> Self {
    let min_gid = glyph_styles.len();

    let need_blue_substs = style.feature.is_some();
    Self {
      charmap,
      lookup_list,
      style,
      glyph_styles,
      need_blue_substs,
      min_gid,
      max_gid: 0,
      lookup_depth: 0,
      visited_set,
    }
  }

  fn process_lookup(&mut self, lookup_index: u16) -> Result<(), ProcessLookupError> {
    match self.lookup_depth == MAX_NESTING_DEPTH {
      true => {
        return Err(ProcessLookupError::ExceededMaxDepth);
      }
      false => {}
    }

    match !self.visited_set.insert(lookup_index) {
      true => {
        return Ok(());
      }
      false => {}
    }
    self.lookup_depth += 1;

    let result = self.process_lookup_inner(lookup_index);

    self.lookup_depth -= 1;
    result
  }

  #[inline(always)]
  fn process_lookup_inner(&mut self, lookup_index: u16) -> Result<(), ProcessLookupError> {
    let Ok(lookups) = self.lookup_list.lookups();
    let subtables = match lookups
      .get(
        ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped_index(lookup_index);
              migrated_value
            }),
          );
          value
        }),
      )
      .and_then(|lookup| lookup.subtables())
    {
      Ok(subtables) => subtables,
      Err(_) => {
        return Ok(());
      }
    };
    match subtables {
      SubstitutionSubtables::Single(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          match table {
            SingleSubst::Format1(table) => {
              let coverage = match table.coverage() {
                Ok(coverage) => coverage,
                Err(_) => {
                  continue;
                }
              };
              let delta = ({
                let Ok(delta_glyph_id) = table.delta_glyph_id();
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, i32>(delta_glyph_id);
                migrated_value
              });
              for gid in coverage.iter() {
                self.capture_glyph(
                                    ({ let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(({ let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>((({ let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
                                        let Ok(value) = gid.to_u32();
                                        value
                                    }); migrated_value })
                                        + delta)); migrated_value })); migrated_value }),
                                );
              }

              match self.need_blue_substs && self.lookup_depth == 1 {
                true => {
                  self.check_blue_coverage(Ok(coverage));
                }
                false => {}
              }
            }
            SingleSubst::Format2(table) => {
              let Ok(substitute_glyph_ids) = table.substitute_glyph_ids();

              for gid in substitute_glyph_ids {
                self.capture_glyph(
                  ({
                    let Ok(value) = ({
                      let Ok(explicit_value) = {
                        let value = gid.get();
                        value
                      };
                      explicit_value
                    })
                    .to_u32();
                    value
                  }),
                );
              }

              match self.need_blue_substs && self.lookup_depth == 1 {
                true => {
                  self.check_blue_coverage(table.coverage());
                }
                false => {}
              }
            }
          }
        }
      }
      SubstitutionSubtables::Multiple(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          for seq in ({
            let value = ({
              let Ok(array) = table.sequences();
              let Ok(items) = array.iter();
              items
            });
            value
          })
          .filter_map(|seq| seq.ok())
          {
            let Ok(substitute_glyph_ids) = seq.substitute_glyph_ids();

            for gid in substitute_glyph_ids {
              self.capture_glyph(
                ({
                  let Ok(value) = ({
                    let Ok(value) = gid.get();
                    value
                  })
                  .to_u32();
                  value
                }),
              );
            }
          }

          match self.need_blue_substs && self.lookup_depth == 1 {
            true => {
              self.check_blue_coverage(table.coverage());
            }
            false => {}
          }
        }
      }
      SubstitutionSubtables::Ligature(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          for set in ({
            let value = ({
              let Ok(array) = table.ligature_sets();
              let Ok(items) = array.iter();
              items
            });
            value
          })
          .filter_map(|set| set.ok())
          {
            for lig in ({
              let value = ({
                let Ok(array) = set.ligatures();
                let Ok(items) = array.iter();
                items
              });
              value
            })
            .filter_map(|lig| lig.ok())
            {
              self.capture_glyph(
                ({
                  let Ok(ligature_glyph) = lig.ligature_glyph();
                  let Ok(value) = ligature_glyph.to_u32();
                  value
                }),
              );
            }
          }
        }
      }
      SubstitutionSubtables::Alternate(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          for set in ({
            let value = ({
              let Ok(array) = table.alternate_sets();
              let Ok(items) = array.iter();
              items
            });
            value
          })
          .filter_map(|set| set.ok())
          {
            let Ok(alternate_glyph_ids) = set.alternate_glyph_ids();

            for gid in alternate_glyph_ids {
              self.capture_glyph(
                ({
                  let Ok(value) = ({
                    let Ok(explicit_value) = ({
                      let value = gid.get();
                      value
                    });
                    explicit_value
                  })
                  .to_u32();
                  value
                }),
              );
            }
          }
        }
      }
      SubstitutionSubtables::Contextual(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          match table {
            SequenceContext::Format1(table) => {
              for set in ({
                let value = ({
                  let Ok(array) = table.seq_rule_sets();
                  let Ok(items) = array.iter();
                  items
                });
                value
              })
              .filter_map(|set| set.ok().flatten())
              {
                for rule in ({
                  let value = ({
                    let Ok(array) = set.seq_rules();
                    let Ok(items) = array.iter();
                    items
                  });
                  value
                })
                .filter_map(|rule| rule.ok())
                {
                  let Ok(seq_lookup_records) = rule.seq_lookup_records();

                  for rec in seq_lookup_records {
                    let Ok(lookup_list_index) = rec.lookup_list_index();
                    self.process_lookup(lookup_list_index)?;
                  }
                }
              }
            }
            SequenceContext::Format2(table) => {
              for set in ({
                let value = ({
                  let Ok(array) = table.class_seq_rule_sets();
                  let Ok(items) = array.iter();
                  items
                });
                value
              })
              .filter_map(|set| set.ok().flatten())
              {
                for rule in ({
                  let value = ({
                    let Ok(array) = set.class_seq_rules();
                    let Ok(items) = array.iter();
                    items
                  });
                  value
                })
                .filter_map(|rule| rule.ok())
                {
                  let Ok(seq_lookup_records) = rule.seq_lookup_records();

                  for rec in seq_lookup_records {
                    let Ok(lookup_list_index) = rec.lookup_list_index();
                    self.process_lookup(lookup_list_index)?;
                  }
                }
              }
            }
            SequenceContext::Format3(table) => {
              let Ok(seq_lookup_records) = table.seq_lookup_records();

              for rec in seq_lookup_records {
                let Ok(lookup_list_index) = rec.lookup_list_index();
                self.process_lookup(lookup_list_index)?;
              }
            }
          }
        }
      }
      SubstitutionSubtables::ChainContextual(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          match table {
            ChainedSequenceContext::Format1(table) => {
              for set in ({
                let value = ({
                  let Ok(array) = table.chained_seq_rule_sets();
                  let Ok(items) = array.iter();
                  items
                });
                value
              })
              .filter_map(|set| set.ok().flatten())
              {
                for rule in ({
                  let value = ({
                    let Ok(array) = set.chained_seq_rules();
                    let Ok(items) = array.iter();
                    items
                  });
                  value
                })
                .filter_map(|rule| rule.ok())
                {
                  let Ok(seq_lookup_records) = rule.seq_lookup_records();

                  for rec in seq_lookup_records {
                    let Ok(lookup_list_index) = rec.lookup_list_index();
                    self.process_lookup(lookup_list_index)?;
                  }
                }
              }
            }
            ChainedSequenceContext::Format2(table) => {
              for set in ({
                let value = ({
                  let Ok(array) = table.chained_class_seq_rule_sets();
                  let Ok(items) = array.iter();
                  items
                });
                value
              })
              .filter_map(|set| set.ok().flatten())
              {
                for rule in ({
                  let value = ({
                    let Ok(array) = set.chained_class_seq_rules();
                    let Ok(items) = array.iter();
                    items
                  });
                  value
                })
                .filter_map(|rule| rule.ok())
                {
                  let Ok(seq_lookup_records) = rule.seq_lookup_records();

                  for rec in seq_lookup_records {
                    let Ok(lookup_list_index) = rec.lookup_list_index();
                    self.process_lookup(lookup_list_index)?;
                  }
                }
              }
            }
            ChainedSequenceContext::Format3(table) => {
              let Ok(seq_lookup_records) = table.seq_lookup_records();

              for rec in seq_lookup_records {
                let Ok(lookup_list_index) = rec.lookup_list_index();
                self.process_lookup(lookup_list_index)?;
              }
            }
          }
        }
      }
      SubstitutionSubtables::Reverse(tables) => {
        for table in tables.iter().filter_map(|table| table.ok()) {
          let Ok(substitute_glyph_ids) = table.substitute_glyph_ids();

          for gid in substitute_glyph_ids {
            self.capture_glyph(
              ({
                let Ok(value) = ({
                  let Ok(explicit_value) = {
                    let value = gid.get();
                    value
                  };
                  explicit_value
                })
                .to_u32();
                value
              }),
            );
          }
        }
      }
      SubstitutionSubtables::EmptyExtension => (),
    }
    Ok(())
  }

  /// Finishes processing for this set of GSUB lookups and
  /// returns the range of touched glyphs.
  fn finish(self) -> Option<Range<usize>> {
    self.visited_set.clear();
    match self.min_gid > self.max_gid {
      true => {
        return None;
      }
      false => {}
    }
    let range = self.min_gid..self.max_gid + 1;
    match self.need_blue_substs {
      true => {
        for glyph in &mut self.glyph_styles[range] {
          glyph.clear_from_gsub();
        }
        None
      }
      false => Some(range),
    }
  }

  /// Checks the given coverage table for any characters in the blue
  /// strings associated with our current style.
  fn check_blue_coverage(&mut self, coverage: Result<CoverageTable<'a>, ReadError>) {
    let coverage = match coverage {
      Ok(coverage) => coverage,
      Err(_) => {
        return;
      }
    };
    for (blue_str, _) in self.style.script.blues {
      match blue_str
        .chars()
        .filter_map(|ch| self.charmap.map(ch))
        .filter_map(|gid| (coverage.get(gid)))
        .next()
        .is_some()
      {
        true => {
          self.need_blue_substs = false;
          return;
        }
        false => {}
      }
    }
  }

  fn capture_glyph(&mut self, gid: u32) {
    let gid = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(gid);
      migrated_value
    });
    match self.glyph_styles.get_mut(gid) {
      Some(style) => {
        style.set_from_gsub_output();
        self.min_gid = gid.min(self.min_gid);
        self.max_gid = gid.max(self.max_gid);
      }
      None => {}
    }
  }
}

pub(crate) struct VisitedLookupSet<'a>(&'a mut [u8]);

impl<'a> VisitedLookupSet<'a> {
  pub fn new(storage: &'a mut [u8]) -> Self {
    Self(storage)
  }

  /// If the given lookup index is not already in the set, adds it and
  /// returns `true`. Returns `false` otherwise.
  ///
  /// This follows the behavior of `HashSet::insert`.
  fn insert(&mut self, lookup_index: u16) -> bool {
    let byte_ix = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(lookup_index);
      migrated_value
    }) / 8;
    let bit_mask = 1
      << ({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u8>((lookup_index % 8));
        migrated_value
      });
    match self.0.get_mut(byte_ix) {
      Some(byte) => match *byte & bit_mask == 0 {
        true => {
          *byte |= bit_mask;
          true
        }
        false => false,
      },
      None => false,
    }
  }

  fn clear(&mut self) {
    self.0.fill(0);
  }
}

#[derive(PartialEq, Debug)]
enum ProcessLookupError {
  ExceededMaxDepth,
}

#[cfg(test)]
mod tests {
  use super::{super::style, *};
  use font_test_data::bebuffer::BeBuffer;
  use raw::{FontData, FontRead};

  #[test]
  fn small_caps_subst() {
    let font = FontRef::new(font_test_data::NOTOSERIF_AUTOHINT_SHAPING).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::BestEffort);
    let style = &style::STYLE_CLASSES[style::StyleClass::LATN_C2SC];
    let mut cluster_shaper = shaper.cluster_shaper(style);
    let mut cluster = ShapedCluster::new();
    cluster_shaper.shape("H", &mut cluster);
    assert_eq!(cluster.len(), 1);

    assert_eq!(
      cluster[0].id,
      ({
        let Ok(value) = GlyphId::new(8);
        value
      })
    );
  }

  #[test]
  fn small_caps_nominal() {
    let font = FontRef::new(font_test_data::NOTOSERIF_AUTOHINT_SHAPING).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let style = &style::STYLE_CLASSES[style::StyleClass::LATN_C2SC];
    let mut cluster_shaper = shaper.cluster_shaper(style);
    let mut cluster = ShapedCluster::new();
    cluster_shaper.shape("H", &mut cluster);
    assert_eq!(cluster.len(), 1);

    assert_eq!(
      cluster[0].id,
      ({
        let Ok(value) = GlyphId::new(1);
        value
      })
    );
  }

  #[test]
  fn exceed_max_depth() {
    let font = FontRef::new(font_test_data::NOTOSERIF_AUTOHINT_SHAPING).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::BestEffort);
    let style = &style::STYLE_CLASSES[style::StyleClass::LATN];

    let mut bad_lookup_builder = BadLookupBuilder::default();
    for i in 0..MAX_NESTING_DEPTH {
      bad_lookup_builder.lookups.push(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(i);
          migrated_value
        }) + 1,
      );
    }
    let lookup_list_buf = bad_lookup_builder.build();
    let lookup_list = SubstitutionLookupList::read(
      ({
        let Ok(value) = FontData::new(&lookup_list_buf);
        value
      }),
    )
    .unwrap();
    let mut set_buf = [0u8; 8192];
    let mut visited_set = VisitedLookupSet(&mut set_buf);
    let mut gsub_handler = GsubHandler::new(
      &shaper.charmap,
      &lookup_list,
      style,
      &mut [],
      &mut visited_set,
    );
    assert_eq!(
      gsub_handler.process_lookup(0),
      Err(ProcessLookupError::ExceededMaxDepth)
    );
  }

  #[test]
  fn dont_cycle_forever() {
    let font = FontRef::new(font_test_data::NOTOSERIF_AUTOHINT_SHAPING).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::BestEffort);
    let style = &style::STYLE_CLASSES[style::StyleClass::LATN];

    let mut bad_lookup_builder = BadLookupBuilder::default();
    bad_lookup_builder.lookups.push(1);
    bad_lookup_builder.lookups.push(0);
    let lookup_list_buf = bad_lookup_builder.build();
    let lookup_list = SubstitutionLookupList::read(
      ({
        let Ok(value) = FontData::new(&lookup_list_buf);
        value
      }),
    )
    .unwrap();
    let mut set_buf = [0u8; 8192];
    let mut visited_set = VisitedLookupSet(&mut set_buf);
    let mut gsub_handler = GsubHandler::new(
      &shaper.charmap,
      &lookup_list,
      style,
      &mut [],
      &mut visited_set,
    );
    gsub_handler.process_lookup(0).unwrap();
  }

  #[test]
  fn visited_set() {
    let count = 2341u16;
    let n_bytes = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(count);
      migrated_value
    })
    .div_ceil(8);
    let mut set_buf = vec![0u8; n_bytes];
    let mut set = VisitedLookupSet::new(&mut set_buf);
    for i in 0..count {
      assert!(set.insert(i));
      assert!(!set.insert(i));
    }
    for byte in &set_buf[0..set_buf.len() - 1] {
      assert_eq!(*byte, 0xFF);
    }
    assert_eq!(*set_buf.last().unwrap(), 0b00011111);
  }

  #[derive(Default)]
  struct BadLookupBuilder {
    /// Just a list of nested lookup indices for each generated lookup
    lookups: Vec<u16>,
  }

  impl BadLookupBuilder {
    fn build(&self) -> Vec<u8> {
      const CONTEXT3_FULL_SIZE: usize = 18;
      let mut buf = BeBuffer::default();

      buf.extend([({
        let Ok(migrated_value) =
          console_core_number_conversion::wrapped::<_, u16>(self.lookups.len());
        migrated_value
      })]);

      let base_offset = 2 + 2 * self.lookups.len();
      for i in 0..self.lookups.len() {
        buf.extend([({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(
            (base_offset + i * CONTEXT3_FULL_SIZE),
          );
          migrated_value
        })]);
      }

      for nested_ix in &self.lookups {
        buf.extend([5u16]);

        buf.extend([0u16]);

        buf.extend([1u16]);

        buf.extend([8u16]);

        buf.extend([3u16]);

        buf.extend([0u16]);

        buf.extend([1u16]);

        buf.extend([0u16, *nested_ix]);
      }
      buf.to_vec()
    }
  }
}
