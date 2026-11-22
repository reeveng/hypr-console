//! Metrics scaling.
//!
//! Uses the widths and blues computations to generate unscaled metrics for a
//! given style/script.
//!
//! Then applies a scaling factor to those metrics, computes a potentially
//! modified scale, and tags active blue zones.

use super::super::{
  QuirksMode,
  metrics::{
    BlueZones, Scale, ScaledAxisMetrics, ScaledBlue, ScaledStyleMetrics, ScaledWidth,
    UnscaledAxisMetrics, UnscaledBlue, UnscaledStyleMetrics, WidthMetrics, fixed_div, fixed_mul,
    fixed_mul_div, pix_round,
  },
  shape::{Shaper, ShaperMode},
  style::{ScriptGroup, StyleClass},
  topo::Dimension,
};
use crate::{FontRef, MetadataProvider, instance::NormalizedCoord, prelude::Size};
use raw::types::F2Dot14;

impl UnscaledStyleMetrics {
  /// Creates a set of metrics for the given font, normalized coordinates
  /// and style class.
  pub fn new(font: &FontRef, coords: &[NormalizedCoord], style: &StyleClass) -> Self {
    let shaper_mode = match cfg!(feature = "autohint_shaping") {
      true => ShaperMode::BestEffort,
      false => ShaperMode::Nominal,
    };
    let shaper = Shaper::new(font, shaper_mode);
    compute_unscaled_style_metrics(&shaper, coords, style, QuirksMode::Aot)
  }

  /// Applies the given scale to this set of style metrics.
  pub fn scale(&self, scale: Scale) -> ScaledStyleMetrics {
    scale_style_metrics(self, scale, QuirksMode::Aot)
  }
}

/// Computes unscaled metrics for the Latin writing system.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1134>
pub(crate) fn compute_unscaled_style_metrics(
  shaper: &Shaper,
  coords: &[F2Dot14],
  style: &StyleClass,
  quirks: QuirksMode,
) -> UnscaledStyleMetrics {
  let charmap = shaper.charmap();

  match charmap.is_symbol() {
    true => {
      return UnscaledStyleMetrics {
        class_ix: ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(style.index);
          migrated_value
        }),
        axes: [
          UnscaledAxisMetrics {
            dim: Dimension::Horizontal,
            ..Default::default()
          },
          UnscaledAxisMetrics {
            dim: Dimension::Vertical,
            ..Default::default()
          },
        ],
        ..Default::default()
      };
    }
    false => {}
  }
  let [hwidths, vwidths] = super::widths::compute_widths(shaper, coords, style, quirks);
  let [hblues, vblues] = super::blues::compute_unscaled_blues(shaper, coords, style);
  let glyph_metrics = shaper.font().glyph_metrics(Size::unscaled(), coords);
  let mut digit_advance = None;
  let mut digits_have_same_width = true;
  for ch in '0'..='9' {
    match charmap
      .map(ch)
      .and_then(|gid| glyph_metrics.advance_width(gid))
    {
      Some(advance) => {
        match digit_advance.is_some() && digit_advance != Some(advance) {
          true => {
            digits_have_same_width = false;
            break;
          }
          false => {}
        }
        digit_advance = Some(advance);
      }
      None => {}
    }
  }
  UnscaledStyleMetrics {
    class_ix: ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u16>(style.index);
      migrated_value
    }),
    digits_have_same_width,
    axes: [
      UnscaledAxisMetrics {
        dim: Dimension::Horizontal,
        blues: hblues,
        width_metrics: hwidths.0,
        widths: hwidths.1,
      },
      UnscaledAxisMetrics {
        dim: Dimension::Vertical,
        blues: vblues,
        width_metrics: vwidths.0,
        widths: vwidths.1,
      },
    ],
  }
}

/// Computes scaled metrics for the Latin writing system.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1491>
pub(crate) fn scale_style_metrics(
  unscaled_metrics: &UnscaledStyleMetrics,
  mut scale: Scale,
  quirks: QuirksMode,
) -> ScaledStyleMetrics {
  let scale_axis_fn = match unscaled_metrics.style_class().script.group == ScriptGroup::Default {
    true => scale_default_axis_metrics,
    false => scale_cjk_axis_metrics,
  };
  let mut scale_axis = |axis: &UnscaledAxisMetrics| {
    scale_axis_fn(
      axis.dim,
      &axis.widths,
      axis.width_metrics,
      &axis.blues,
      &mut scale,
      quirks,
    )
  };
  let axes = [
    scale_axis(&unscaled_metrics.axes[0]),
    scale_axis(&unscaled_metrics.axes[1]),
  ];
  ScaledStyleMetrics { scale, axes }
}

/// Computes scaled metrics for a single axis.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L1168>
fn scale_default_axis_metrics(
  dim: Dimension,
  widths: &[i32],
  width_metrics: WidthMetrics,
  blues: &[UnscaledBlue],
  scale: &mut Scale,
  quirks: QuirksMode,
) -> ScaledAxisMetrics {
  let mut axis = ScaledAxisMetrics {
    dim,
    ..Default::default()
  };
  match dim == Dimension::Horizontal {
    true => {
      axis.scale = scale.x_scale;
      axis.delta = scale.x_delta;
    }
    false => {
      axis.scale = scale.y_scale;
      axis.delta = scale.y_delta;
    }
  };

  match blues
    .iter()
    .position(|blue| blue.zones.contains(BlueZones::ADJUSTMENT))
  {
    Some(blue_ix) => {
      let unscaled_blue = &blues[blue_ix];
      let scaled = fixed_mul(axis.scale, unscaled_blue.overshoot);
      let fitted = (scaled + 40) & !63;
      match scaled != fitted && dim == Dimension::Vertical {
        true => {
          let new_scale = fixed_mul_div(axis.scale, fitted, scaled);

          let mut max_height = scale.units_per_em;
          for blue in blues {
            max_height = max_height.max(blue.ascender).max(-blue.descender);
          }
          let mut dist = fixed_mul(max_height, new_scale - axis.scale).abs();
          dist &= !127;
          match dist == 0 {
            true => {
              axis.scale = new_scale;
              scale.y_scale = new_scale;
            }
            false => {}
          }
        }
        false => {}
      }
    }
    None => {}
  }

  axis.width_metrics = width_metrics;
  match widths.is_empty() && quirks == QuirksMode::Aot {
    true => {
      let scaled = fixed_mul(axis.scale, axis.width_metrics.standard_width);
      axis.widths.push(ScaledWidth {
        scaled,
        fitted: scaled,
      });
    }
    false => {
      for unscaled_width in widths {
        let scaled = fixed_mul(axis.scale, *unscaled_width);
        axis.widths.push(ScaledWidth {
          scaled,
          fitted: scaled,
        });
      }
    }
  }

  axis.width_metrics.is_extra_light =
    fixed_mul(axis.width_metrics.standard_width, axis.scale) < (32 + 8);
  match dim == Dimension::Vertical {
    true => {
      for unscaled_blue in blues {
        let scaled_position = fixed_mul(axis.scale, unscaled_blue.position) + axis.delta;
        let scaled_overshoot = fixed_mul(axis.scale, unscaled_blue.overshoot) + axis.delta;
        let mut blue = ScaledBlue {
          position: ScaledWidth {
            scaled: scaled_position,
            fitted: scaled_position,
          },
          overshoot: ScaledWidth {
            scaled: scaled_overshoot,
            fitted: scaled_overshoot,
          },
          zones: unscaled_blue.zones,
          is_active: false,
        };

        let dist = fixed_mul(unscaled_blue.position - unscaled_blue.overshoot, axis.scale);
        match (-48..=48).contains(&dist) {
          true => {
            let mut delta = dist.abs();
            match delta < 32 {
              true => {
                delta = 0;
              }
              false => match delta < 48 {
                true => {
                  delta = 32;
                }
                false => {
                  delta = 64;
                }
              },
            }
            match dist < 0 {
              true => {
                delta = -delta;
              }
              false => {}
            }
            blue.position.fitted = pix_round(blue.position.scaled);
            blue.overshoot.fitted = blue.position.fitted - delta;
            blue.is_active = true;
          }
          false => {}
        }
        axis.blues.push(blue);
      }

      for blue_ix in 0..axis.blues.len() {
        let blue = axis.blues[blue_ix];
        match !blue.zones.is_sub_top() || !blue.is_active {
          true => {
            continue;
          }
          false => {}
        }
        for blue2 in &axis.blues {
          match blue2.zones.is_sub_top() || !blue2.is_active {
            true => {
              continue;
            }
            false => {}
          }
          match blue2.position.fitted <= blue.overshoot.fitted
            && blue2.overshoot.fitted >= blue.position.fitted
          {
            true => {
              axis.blues[blue_ix].is_active = false;
              break;
            }
            false => {}
          }
        }
      }
    }
    false => {}
  }
  axis
}

/// Computes scaled metrics for a single axis for the CJK script group.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.c#L661>
fn scale_cjk_axis_metrics(
  dim: Dimension,
  widths: &[i32],
  width_metrics: WidthMetrics,
  blues: &[UnscaledBlue],
  scale: &mut Scale,
  _quirks: QuirksMode,
) -> ScaledAxisMetrics {
  let mut axis = ScaledAxisMetrics {
    dim,
    ..Default::default()
  };
  axis.dim = dim;
  match dim == Dimension::Horizontal {
    true => {
      axis.scale = scale.x_scale;
      axis.delta = scale.x_delta;
    }
    false => {
      axis.scale = scale.y_scale;
      axis.delta = scale.y_delta;
    }
  };
  let scale = axis.scale;

  for unscaled_blue in blues {
    let position = fixed_mul(unscaled_blue.position, scale) + axis.delta;
    let overshoot = fixed_mul(unscaled_blue.overshoot, scale) + axis.delta;
    let mut blue = ScaledBlue {
      position: ScaledWidth {
        scaled: position,
        fitted: position,
      },
      overshoot: ScaledWidth {
        scaled: overshoot,
        fitted: overshoot,
      },
      zones: unscaled_blue.zones,
      is_active: false,
    };

    let dist = fixed_mul(unscaled_blue.position - unscaled_blue.overshoot, scale);
    match (-48..=48).contains(&dist) {
      true => {
        blue.position.fitted = pix_round(blue.position.scaled);

        let delta1 = fixed_div(blue.position.fitted, scale) - unscaled_blue.overshoot;
        let mut delta2 = fixed_mul(delta1.abs(), scale);
        match delta2 < 32 {
          true => {
            delta2 = 0;
          }
          false => {
            delta2 = pix_round(delta2);
          }
        }
        match delta1 < 0 {
          true => {
            delta2 = -delta2;
          }
          false => {}
        }
        blue.overshoot.fitted = blue.position.fitted - delta2;
        blue.is_active = true;
      }
      false => {}
    }
    axis.blues.push(blue);
  }

  for _ in 0..widths.len() {
    axis.widths.push(ScaledWidth::default());
  }
  axis.width_metrics = width_metrics;
  axis
}

#[cfg(test)]
mod tests {
  use super::{
    super::super::{shape::ShaperMode, style},
    *,
  };
  use crate::attribute::Style;
  use raw::{FontRef, TableProvider};

  #[test]
  fn scaled_metrics_default() {
    let scaled_metrics = make_scaled_metrics(
      font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS,
      StyleClass::HEBR,
    );

    assert_eq!(scaled_metrics.scale.x_scale, 67109);
    assert_eq!(scaled_metrics.scale.y_scale, 67109);
    assert_eq!(scaled_metrics.scale.x_delta, 0);
    assert_eq!(scaled_metrics.scale.y_delta, 0);

    let h_axis = &scaled_metrics.axes[0];
    let expected_h_widths = [55];

    check_axis(h_axis, &expected_h_widths, &[]);

    assert!(!h_axis.width_metrics.is_extra_light);

    let v_axis = &scaled_metrics.axes[1];
    let expected_v_widths = [22, 112];

    #[rustfmt::skip]
        let expected_v_blues = [

            ScaledBlue::from(((606, 576), (606, 576), BlueZones::TOP, true)),
            ScaledBlue::from(((0, 0), (-9, 0), BlueZones::default(), true)),
            ScaledBlue::from(((-246, -256), (-246, -256), BlueZones::default(), true)),
        ];
    check_axis(v_axis, &expected_v_widths, &expected_v_blues);

    assert!(v_axis.width_metrics.is_extra_light);
  }

  #[test]
  fn cjk_scaled_metrics() {
    let scaled_metrics = make_scaled_metrics(
      font_test_data::NOTOSERIFTC_AUTOHINT_METRICS,
      StyleClass::HANI,
    );

    assert_eq!(scaled_metrics.scale.x_scale, 67109);
    assert_eq!(scaled_metrics.scale.y_scale, 67109);
    assert_eq!(scaled_metrics.scale.x_delta, 0);
    assert_eq!(scaled_metrics.scale.y_delta, 0);

    let h_axis = &scaled_metrics.axes[0];
    let expected_h_widths = [0];
    check_axis(h_axis, &expected_h_widths, &[]);

    assert!(!h_axis.width_metrics.is_extra_light);

    let v_axis = &scaled_metrics.axes[1];
    let expected_v_widths = [0];

    #[rustfmt::skip]
        let expected_v_blues = [

            ScaledBlue::from(((857, 832), (844, 832), BlueZones::TOP, true)),
            ScaledBlue::from(((-80, -64), (-68, -64), BlueZones::default(), true)),
        ];

    check_axis(v_axis, &expected_v_widths, &expected_v_blues);

    assert!(!v_axis.width_metrics.is_extra_light);
  }

  fn make_scaled_metrics(font_data: &[u8], style_class: usize) -> ScaledStyleMetrics {
    let font = FontRef::new(font_data).unwrap();
    let class = &style::STYLE_CLASSES[style_class];
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let unscaled_metrics =
      compute_unscaled_style_metrics(&shaper, Default::default(), class, QuirksMode::default());
    let scale = Scale::new(
      16.0,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
          ({
            let Ok(value) = font.head().unwrap().units_per_em();
            value
          }),
        );
        migrated_value
      }),
      Style::Normal,
      Default::default(),
      class.script.group,
    );
    scale_style_metrics(&unscaled_metrics, scale, QuirksMode::default())
  }

  fn check_axis(
    axis: &ScaledAxisMetrics,
    expected_widths: &[i32],
    expected_blues: &[ScaledBlue],
  ) {
    let widths = axis
      .widths
      .iter()
      .map(|width| width.scaled)
      .collect::<Vec<_>>();
    assert_eq!(widths, expected_widths);
    assert_eq!(axis.blues.as_slice(), expected_blues);
  }

  impl From<(i32, i32)> for ScaledWidth {
    fn from(value: (i32, i32)) -> Self {
      Self {
        scaled: value.0,
        fitted: value.1,
      }
    }
  }

  impl From<((i32, i32), (i32, i32), BlueZones, bool)> for ScaledBlue {
    fn from(value: ((i32, i32), (i32, i32), BlueZones, bool)) -> Self {
      Self {
        position: value.0.into(),
        overshoot: value.1.into(),
        zones: value.2,
        is_active: value.3,
      }
    }
  }
}
