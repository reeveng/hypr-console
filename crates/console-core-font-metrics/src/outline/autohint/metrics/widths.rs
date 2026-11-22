//! Latin standard stem width computation.

use super::super::{
  QuirksMode, derived_constant,
  metrics::{self, MAX_WIDTHS, UnscaledWidths, WidthMetrics},
  outline::Outline,
  shape::{ShapedCluster, Shaper},
  style::{ScriptGroup, StyleClass},
  topo::{Axis, Dimension, compute_segments, link_segments},
};
use crate::MetadataProvider;
use raw::{TableProvider, types::F2Dot14};

/// Compute all stem widths and initialize standard width and height for the
/// given script.
///
/// Returns width metrics and unscaled widths for each dimension.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L54>
pub(super) fn compute_widths(
  shaper: &Shaper,
  coords: &[F2Dot14],
  style: &StyleClass,
  quirks: QuirksMode,
) -> [(WidthMetrics, UnscaledWidths); 2] {
  let mut result: [(WidthMetrics, UnscaledWidths); 2] = Default::default();
  let font = shaper.font();
  let glyphs = font.outline_glyphs();
  let units_per_em = font
    .head()
    .map(|head| {
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
          ({
            let Ok(value) = head.units_per_em();
            value
          }),
        );
        migrated_value
      })
    })
    .unwrap_or_default();
  let mut outline = Outline::default();
  let mut axis = Axis::default();
  let mut cluster_shaper = shaper.cluster_shaper(style);
  let mut shaped_cluster = ShapedCluster::default();

  let glyph = style
    .script
    .std_chars
    .split(' ')
    .filter_map(|cluster| {
      cluster_shaper.shape(cluster, &mut shaped_cluster);

      match shaped_cluster.as_slice() {
        [glyph]
          if ({
            let Ok(value) = glyph.id.to_u32();
            value
          }) != 0 =>
        {
          (glyphs.get(glyph.id))
        }
        _ => None,
      }
    })
    .next();
  match glyph {
    Some(glyph) => {
      match outline.fill(&glyph, coords, quirks).is_ok() && !outline.points.is_empty() {
        true => {
          for (dim, (_metrics, widths)) in [Dimension::Horizontal, Dimension::Vertical]
            .into_iter()
            .zip(result.iter_mut())
          {
            axis.reset(dim, outline.orientation);

            compute_segments(&mut outline, &mut axis, ScriptGroup::Default);
            link_segments(&outline, &mut axis, 0, ScriptGroup::Default, None);
            let segments = axis.segments.as_slice();
            for (segment_ix, segment) in segments.iter().enumerate() {
              let segment_ix = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u16>(segment_ix);
                migrated_value
              });
              let link_ix = match segment.link_ix {
                Some(link_ix) => link_ix,
                None => {
                  continue;
                }
              };
              let link = &segments[({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index(link_ix);
                migrated_value
              })];
              match link_ix > segment_ix && link.link_ix == Some(segment_ix) {
                true => {
                  let dist = (({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(segment.pos);
                    migrated_value
                  }) - ({
                    let Ok(migrated_value) =
                      console_core_number_conversion::wrapped::<_, i32>(link.pos);
                    migrated_value
                  }))
                  .abs();
                  match widths.len() < MAX_WIDTHS {
                    true => {
                      widths.push(dist);
                    }
                    false => {
                      break;
                    }
                  }
                }
                false => {}
              }
            }

            match widths.is_empty() {
              true => {
                widths.push(0);
              }
              false => {}
            }

            metrics::sort_and_quantize_widths(widths, units_per_em / 100);
          }
        }
        false => {}
      }
    }
    None => {}
  }
  for (metrics, widths) in result.iter_mut() {
    let stdw = widths
      .first()
      .copied()
      .unwrap_or_else(|| derived_constant(units_per_em, 50));

    metrics.edge_distance_threshold = stdw / 5;
    metrics.standard_width = stdw;
    metrics.is_extra_light = false;
  }
  result
}

#[cfg(test)]
mod tests {
  use super::{
    super::super::{shape::ShaperMode, style},
    *,
  };
  use raw::FontRef;

  #[test]
  fn computed_widths() {
    check_widths(
      font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS,
      super::StyleClass::HEBR,
      [
        (
          WidthMetrics {
            edge_distance_threshold: 10,
            standard_width: 54,
            is_extra_light: false,
          },
          &[54],
        ),
        (
          WidthMetrics {
            edge_distance_threshold: 4,
            standard_width: 21,
            is_extra_light: false,
          },
          &[21, 109],
        ),
      ],
    );
  }

  #[test]
  fn fallback_widths() {
    check_widths(
      font_test_data::CANTARELL_VF_TRIMMED,
      super::StyleClass::LATN,
      [
        (
          WidthMetrics {
            edge_distance_threshold: 4,
            standard_width: 24,
            is_extra_light: false,
          },
          &[],
        ),
        (
          WidthMetrics {
            edge_distance_threshold: 4,
            standard_width: 24,
            is_extra_light: false,
          },
          &[],
        ),
      ],
    );
  }

  #[test]
  fn cjk_computed_widths() {
    check_widths(
      font_test_data::NOTOSERIFTC_AUTOHINT_METRICS,
      super::StyleClass::HANI,
      [
        (
          WidthMetrics {
            edge_distance_threshold: 13,
            standard_width: 65,
            is_extra_light: false,
          },
          &[65],
        ),
        (
          WidthMetrics {
            edge_distance_threshold: 5,
            standard_width: 29,
            is_extra_light: false,
          },
          &[29],
        ),
      ],
    );
  }

  fn check_widths(font_data: &[u8], style_class: usize, expected: [(WidthMetrics, &[i32]); 2]) {
    let font = FontRef::new(font_data).unwrap();
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let script = &style::STYLE_CLASSES[style_class];
    let [(hori_metrics, hori_widths), (vert_metrics, vert_widths)] =
      compute_widths(&shaper, Default::default(), script, QuirksMode::default());
    assert_eq!(hori_metrics, expected[0].0);
    assert_eq!(hori_widths.as_slice(), expected[0].1);
    assert_eq!(vert_metrics, expected[1].0);
    assert_eq!(vert_widths.as_slice(), expected[1].1);
  }
}
