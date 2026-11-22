//! Global font and glyph specific metrics.
//!
//! Metrics are various measurements that define positioning and layout
//! characteristics for a font. They come in two flavors:
//!
//! * Global metrics: these are applicable to all glyphs in a font and generally
//!   define values that are used for the layout of a collection of glyphs. For example,
//!   the ascent, descent and leading values determine the position of the baseline where
//!   a glyph should be rendered as well as the suggested spacing above and below it.
//!
//! * Glyph metrics: these apply to single glyphs. For example, the advance
//!   width value describes the distance between two consecutive glyphs on a line.
//!
//! ### Selecting an "instance"
//! Both global and glyph specific metrics accept two additional pieces of information
//! to select the desired instance of a font:
//! * Size: represented by the [Size] type, this determines the scaling factor that is
//!   applied to all metrics.
//! * Normalized variation coordinates: represented by the [LocationRef] type,
//!   these define the position in design space for a variable font. For a non-variable
//!   font, these coordinates are ignored and you can pass [LocationRef::default()]
//!   as an argument for this parameter.
//!

use read_fonts::{
  FontRef, TableProvider,
  tables::{
    glyf::Glyf, gvar::Gvar, hmtx::LongMetric, hvar::Hvar, loca::Loca, os2::SelectionFlags,
  },
  types::{BigEndian, Fixed, GlyphId},
};

use crate::{
  MetadataProvider,
  outline::{DrawSettings, pen::ControlBoundsPen},
};

use super::instance::{LocationRef, NormalizedCoord, Size};

/// Type for a bounding box with single precision floating point coordinates.
pub type BoundingBox = read_fonts::types::BoundingBox<f32>;

/// Metrics for a text decoration.
///
/// This represents the suggested offset and thickness of an underline
/// or strikeout text decoration.
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Decoration {
  /// Offset to the top of the decoration from the baseline.
  pub offset: f32,
  /// Thickness of the decoration.
  pub thickness: f32,
}

/// Metrics that apply to all glyphs in a font.
///
/// These are retrieved for a specific position in the design space.
///
/// This metrics here are derived from the following tables:
/// * [head](https://learn.microsoft.com/en-us/typography/opentype/spec/head): `units_per_em`, `bounds`
/// * [maxp](https://learn.microsoft.com/en-us/typography/opentype/spec/maxp): `glyph_count`
/// * [post](https://learn.microsoft.com/en-us/typography/opentype/spec/post): `is_monospace`, `italic_angle`, `underline`
/// * [OS/2](https://learn.microsoft.com/en-us/typography/opentype/spec/os2): `average_width`, `cap_height`,
///   `x_height`, `strikeout`, as well as the line metrics: `ascent`, `descent`, `leading` if the `USE_TYPOGRAPHIC_METRICS`
///   flag is set or the `hhea` line metrics are zero (the Windows metrics are used as a last resort).
/// * [hhea](https://learn.microsoft.com/en-us/typography/opentype/spec/hhea): `max_width`, as well as the line metrics:
///   `ascent`, `descent`, `leading` if they are non-zero and the `USE_TYPOGRAPHIC_METRICS` flag is not set in the OS/2 table
///
/// For variable fonts, deltas are computed using the  [MVAR](https://learn.microsoft.com/en-us/typography/opentype/spec/MVAR)
/// table.
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Metrics {
  /// Number of font design units per em unit.
  pub units_per_em: u16,
  /// Number of glyphs in the font.
  pub glyph_count: u16,
  /// True if the font is not proportionally spaced.
  pub is_monospace: bool,
  /// Italic angle in counter-clockwise degrees from the vertical. Zero for upright text,
  /// negative for text that leans to the right.
  pub italic_angle: f32,
  /// Distance from the baseline to the top of the alignment box.
  pub ascent: f32,
  /// Distance from the baseline to the bottom of the alignment box.
  pub descent: f32,
  /// Recommended additional spacing between lines.
  pub leading: f32,
  /// Distance from the baseline to the top of a typical English capital.
  pub cap_height: Option<f32>,
  /// Distance from the baseline to the top of the lowercase "x" or
  /// similar character.
  pub x_height: Option<f32>,
  /// Average width of all non-zero width characters in the font.
  pub average_width: Option<f32>,
  /// Maximum advance width of all characters in the font.
  pub max_width: Option<f32>,
  /// Metrics for an underline decoration.
  pub underline: Option<Decoration>,
  /// Metrics for a strikeout decoration.
  pub strikeout: Option<Decoration>,
  /// Union of minimum and maximum extents for all glyphs in the font.
  pub bounds: Option<BoundingBox>,
}

impl Metrics {
  /// Creates new metrics for the given font, size, and location in
  /// normalized variation space.
  pub fn new<'a>(font: &FontRef<'a>, size: Size, location: impl Into<LocationRef<'a>>) -> Self {
    let head = font.head();
    let mut metrics = Metrics {
      units_per_em: ({
        let value = head
          .map(|head| {
            ({
              let Ok(value) = head.units_per_em();
              value
            })
          })
          .unwrap_or_default();
        value
      }),
      ..Default::default()
    };
    let coords = location.into().effective_coords();
    let scale = size.linear_scale(metrics.units_per_em);
    match font.head() {
      Ok(head) => {
        metrics.bounds = Some(BoundingBox {
          x_min: f32::from(
            ({
              let Ok(value) = head.x_min();
              value
            }),
          ) * scale,
          y_min: f32::from(
            ({
              let Ok(value) = head.y_min();
              value
            }),
          ) * scale,
          x_max: f32::from(
            ({
              let Ok(value) = head.x_max();
              value
            }),
          ) * scale,
          y_max: f32::from(
            ({
              let Ok(value) = head.y_max();
              value
            }),
          ) * scale,
        });
      }
      Err(_) => {}
    }
    match font.maxp() {
      Ok(maxp) => {
        metrics.glyph_count = ({
          let Ok(value) = maxp.num_glyphs();
          value
        });
      }
      Err(_) => {}
    }
    match font.post() {
      Ok(post) => {
        let Ok(is_fixed_pitch) = post.is_fixed_pitch();
        metrics.is_monospace = is_fixed_pitch != 0;
        metrics.italic_angle = ({
          let Ok(italic_angle) = post.italic_angle();
          let Ok(value) = italic_angle.to_f64();
          value
        }) as f32;
        metrics.underline = Some(Decoration {
          offset: f32::from(
            ({
              let Ok(underline_position) = post.underline_position();
              let Ok(value) = underline_position.to_i16();
              value
            }),
          ) * scale,
          thickness: f32::from(
            ({
              let Ok(underline_thickness) = post.underline_thickness();
              let Ok(value) = underline_thickness.to_i16();
              value
            }),
          ) * scale,
        });
      }
      Err(_) => {}
    }
    let hhea = font.hhea();
    match &hhea {
      Ok(hhea) => {
        metrics.max_width = Some(
          f32::from(
            ({
              let Ok(value) = {
                let Ok(value) = hhea.advance_width_max();
                value
              }
              .to_u16();
              value
            }),
          ) * scale,
        );
      }
      Err(_) => {}
    }

    let os2 = font.os2().ok();
    let mut used_typo_metrics = false;
    match &os2 {
      Some(os2) => {
        let Ok(fs_selection) = os2.fs_selection();
        let Ok(contains) = fs_selection.contains(SelectionFlags::USE_TYPO_METRICS);

        match (contains == read_fonts::Presence::Present) {
          true => {
            let Ok(s_typo_ascender) = os2.s_typo_ascender();
            metrics.ascent = f32::from(s_typo_ascender) * scale;
            let Ok(s_typo_descender) = os2.s_typo_descender();
            metrics.descent = f32::from(s_typo_descender) * scale;
            let Ok(s_typo_line_gap) = os2.s_typo_line_gap();
            metrics.leading = f32::from(s_typo_line_gap) * scale;
            used_typo_metrics = true;
          }
          false => {}
        }
        let Ok(x_avg_char_width) = os2.x_avg_char_width();
        metrics.average_width = Some(f32::from(x_avg_char_width) * scale);
        let Ok(s_cap_height) = os2.s_cap_height();
        metrics.cap_height = s_cap_height.map(|v| f32::from(v) * scale);
        let Ok(sx_height) = os2.sx_height();
        metrics.x_height = sx_height.map(|v| f32::from(v) * scale);
        let Ok(y_strikeout_position) = os2.y_strikeout_position();
        metrics.strikeout = Some(Decoration {
          offset: f32::from(y_strikeout_position) * scale,
          thickness: f32::from(
            ({
              let Ok(explicit_value) = os2.y_strikeout_size();
              explicit_value
            }),
          ) * scale,
        });
      }
      None => {}
    }
    match !used_typo_metrics {
      true => {
        match font.hhea() {
          Ok(hhea) => {
            metrics.ascent = f32::from(
              ({
                let Ok(value) = ({
                  let Ok(value) = hhea.ascender();
                  value
                })
                .to_i16();
                value
              }),
            ) * scale;
            metrics.descent = f32::from(
              ({
                let Ok(value) = ({
                  let Ok(value) = hhea.descender();
                  value
                })
                .to_i16();
                value
              }),
            ) * scale;
            metrics.leading = f32::from(
              ({
                let Ok(value) = ({
                  let Ok(value) = hhea.line_gap();
                  value
                })
                .to_i16();
                value
              }),
            ) * scale;
          }
          Err(_) => {}
        }
        match metrics.ascent == 0.0 && metrics.descent == 0.0 {
          true => match &os2 {
            Some(os2) => {
              let Ok(s_typo_ascender) = os2.s_typo_ascender();

              match s_typo_ascender != 0
                || ({
                  let Ok(explicit_value) = os2.s_typo_descender();
                  explicit_value
                }) != 0
              {
                true => {
                  let Ok(os2_s_typo_ascender) = os2.s_typo_ascender();
                  metrics.ascent = f32::from(os2_s_typo_ascender) * scale;
                  let Ok(s_typo_descender) = os2.s_typo_descender();
                  metrics.descent = f32::from(s_typo_descender) * scale;
                  let Ok(s_typo_line_gap) = os2.s_typo_line_gap();
                  metrics.leading = f32::from(s_typo_line_gap) * scale;
                }
                false => {
                  let Ok(us_win_ascent) = os2.us_win_ascent();
                  metrics.ascent = f32::from(us_win_ascent) * scale;

                  let Ok(us_win_descent) = os2.us_win_descent();
                  metrics.descent = -(f32::from(us_win_descent) * scale);
                }
              }
            }
            None => {}
          },
          false => {}
        }
      }
      false => {}
    }
    if let (Ok(mvar), true) = (font.mvar(), !coords.is_empty()) {
      use read_fonts::tables::mvar::tags::*;
      let metric_delta = |tag| {
        ({
          let Ok(value) = mvar.metric_delta(tag, coords).unwrap_or_default().to_f64();
          value
        }) as f32
          * scale
      };
      metrics.ascent += metric_delta(HASC);
      metrics.descent += metric_delta(HDSC);
      metrics.leading += metric_delta(HLGP);
      match &mut metrics.cap_height {
        Some(cap_height) => {
          *cap_height += metric_delta(CPHT);
        }
        None => {}
      }
      match &mut metrics.x_height {
        Some(x_height) => {
          *x_height += metric_delta(XHGT);
        }
        None => {}
      }
      match &mut metrics.underline {
        Some(underline) => {
          underline.offset += metric_delta(UNDO);
          underline.thickness += metric_delta(UNDS);
        }
        None => {}
      }
      match &mut metrics.strikeout {
        Some(strikeout) => {
          strikeout.offset += metric_delta(STRO);
          strikeout.thickness += metric_delta(STRS);
        }
        None => {}
      }
    }
    metrics
  }
}

/// Glyph specific metrics.
#[derive(Clone)]
pub struct GlyphMetrics<'a> {
  font: FontRef<'a>,
  size: Size,
  glyph_count: u32,
  fixed_scale: FixedScaleFactor,
  h_metrics: &'a [LongMetric],
  default_advance_width: u16,
  lsbs: &'a [BigEndian<i16>],
  hvar: Option<Hvar<'a>>,
  gvar: Option<Gvar<'a>>,
  loca_glyf: Option<(Loca<'a>, Glyf<'a>)>,
  coords: &'a [NormalizedCoord],
}

impl<'a> GlyphMetrics<'a> {
  /// Creates new glyph metrics from the given font, size, and location in
  /// normalized variation space.
  pub fn new(font: &FontRef<'a>, size: Size, location: impl Into<LocationRef<'a>>) -> Self {
    let glyph_count = font
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
    let upem = font
      .head()
      .map(|head| {
        ({
          let Ok(value) = head.units_per_em();
          value
        })
      })
      .unwrap_or_default();
    let fixed_scale = FixedScaleFactor(size.fixed_linear_scale((upem)));
    let coords = location.into().effective_coords();
    let (h_metrics, default_advance_width, lsbs) = font
      .hmtx()
      .map(|hmtx| {
        let h_metrics = ({
          let Ok(value) = hmtx.h_metrics();
          value
        });
        let default_advance_width = (h_metrics)
          .last()
          .map(|m| {
            ({
              let Ok(value) = m.advance.get();
              value
            })
          })
          .unwrap_or(0);
        let Ok(lsbs) = hmtx.left_side_bearings();
        (h_metrics, default_advance_width, lsbs)
      })
      .unwrap_or_default();
    let hvar = font.hvar().ok();
    let gvar = font.gvar().ok();
    let loca_glyf = if let (Ok(loca), Ok(glyf)) = (font.loca(None), font.glyf()) {
      Some((loca, glyf))
    } else {
      None
    };
    Self {
      font: font.clone(),
      size,
      glyph_count,
      fixed_scale,
      h_metrics: (h_metrics),
      default_advance_width: (default_advance_width),
      lsbs: (lsbs),
      hvar,
      gvar,
      loca_glyf,
      coords,
    }
  }

  /// Returns the number of available glyphs in the font.
  pub fn glyph_count(&self) -> u32 {
    self.glyph_count
  }

  /// Returns the advance width for the specified glyph.
  ///
  /// If normalized coordinates were provided when constructing glyph metrics and
  /// an `HVAR` table is present, applies the appropriate delta.
  ///
  /// Returns `None` if `glyph_id >= self.glyph_count()` or the underlying font
  /// data is invalid.
  pub fn advance_width(&self, glyph_id: GlyphId) -> Option<f32> {
    if ({
      let Ok(value) = glyph_id.to_u32();
      value
    }) >= self.glyph_count
    {
      return None;
    }
    let mut advance = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
        ({
          let value = self
            .h_metrics
            .get(
              ({
                let Ok(migrated_value) = console_core_number_conversion::wrapped_index({
                  let Ok(value) = glyph_id.to_u32();
                  value
                });
                migrated_value
              }),
            )
            .map(|metric| {
              let Ok(advance) = metric.advance();
              advance
            })
            .unwrap_or(self.default_advance_width);
          value
        }),
      );
      migrated_value
    });
    match &self.hvar {
      Some(hvar) => {
        advance += hvar
          .advance_width_delta(glyph_id, self.coords)
          .map(|delta| {
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::toward_zero_i32(f64::from(
                  ({
                    let Ok(value) = delta.to_f64();
                    value
                  }),
                ));
              migrated_value
            })
          })
          .unwrap_or(0);
      }
      None => match self.gvar.is_some() {
        true => {
          advance += self.metric_deltas_from_gvar(glyph_id).unwrap_or_default()[1];
        }
        false => {}
      },
    }
    Some(self.fixed_scale.apply(advance))
  }

  /// Returns the left side bearing for the specified glyph.
  ///
  /// If normalized coordinates were provided when constructing glyph metrics and
  /// an `HVAR` table is present, applies the appropriate delta.
  ///
  /// Returns `None` if `glyph_id >= self.glyph_count()` or the underlying font
  /// data is invalid.
  pub fn left_side_bearing(&self, glyph_id: GlyphId) -> Option<f32> {
    if ({
      let Ok(value) = glyph_id.to_u32();
      value
    }) >= self.glyph_count
    {
      return None;
    }
    let gid_index = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        ({
          let Ok(value) = glyph_id.to_u32();
          value
        }),
      );
      migrated_value
    });
    let mut lsb = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
        let value = self
          .h_metrics
          .get(gid_index)
          .map(|metric| {
            let Ok(bearing) = metric.side_bearing();
            bearing
          })
          .unwrap_or_else(|| {
            (self
              .lsbs
              .get(gid_index.saturating_sub(self.h_metrics.len())))
            .map(|lsb| {
              ({
                let Ok(value) = lsb.get();
                value
              })
            })
            .unwrap_or_default()
          });
        value
      });
      migrated_value
    });
    match &self.hvar {
      Some(hvar) => {
        lsb += hvar
          .lsb_delta(glyph_id, self.coords)
          .map(|delta| {
            ({
              let Ok(migrated_value) =
                console_core_number_conversion::toward_zero_i32(f64::from(
                  ({
                    let Ok(value) = delta.to_f64();
                    value
                  }),
                ));
              migrated_value
            })
          })
          .unwrap_or(0);
      }
      None => match self.gvar.is_some() {
        true => {
          lsb += self.metric_deltas_from_gvar(glyph_id).unwrap_or_default()[0];
        }
        false => {}
      },
    }
    Some(self.fixed_scale.apply(lsb))
  }

  /// Returns the bounding box for the specified glyph.
  ///
  /// Returns `None` if `glyph_id >= self.glyph_count()`, the underlying font
  /// data is invalid.
  pub fn bounds(&self, glyph_id: GlyphId) -> Option<BoundingBox> {
    match self.gvar.is_some() || self.font.cff().ok().is_some() || self.font.cff2().ok().is_some()
    {
      true => {
        return self.bounds_from_outline(glyph_id);
      }
      false => {}
    }
    let (loca, glyf) = self.loca_glyf.as_ref()?;
    Some(match loca.get_glyf(glyph_id, glyf).ok()? {
      Some(glyph) => BoundingBox {
        x_min: self.fixed_scale.apply(
          ({
            let Ok(x_min) = glyph.x_min();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_min);
            migrated_value
          }),
        ),
        y_min: self.fixed_scale.apply(
          ({
            let Ok(y_min) = glyph.y_min();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_min);
            migrated_value
          }),
        ),
        x_max: self.fixed_scale.apply(
          ({
            let Ok(x_max) = glyph.x_max();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x_max);
            migrated_value
          }),
        ),
        y_max: self.fixed_scale.apply(
          ({
            let Ok(y_max) = glyph.y_max();
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y_max);
            migrated_value
          }),
        ),
      },

      None => BoundingBox::default(),
    })
  }
}

impl GlyphMetrics<'_> {
  fn metric_deltas_from_gvar(&self, glyph_id: GlyphId) -> Option<[i32; 2]> {
    let (loca, glyf) = self.loca_glyf.as_ref()?;
    let mut deltas = self
      .gvar
      .as_ref()?
      .phantom_point_deltas(glyf, loca, self.coords, glyph_id)
      .ok()
      .flatten()?;
    deltas[1] -= deltas[0];
    Some([deltas[0], deltas[1]].map(|delta| {
      ({
        let Ok(value) = delta.x.to_i32();
        value
      })
    }))
  }

  fn bounds_from_outline(&self, glyph_id: GlyphId) -> Option<BoundingBox> {
    match (self.font.outline_glyphs().get(glyph_id)) {
      Some(outline) => {
        let settings = DrawSettings::unhinted(self.size, self.coords);
        let mut pen = ControlBoundsPen::default();
        outline.draw(settings, &mut pen).ok()?;
        pen.bounding_box()
      }
      None => None,
    }
  }
}

#[derive(Copy, Clone)]
struct FixedScaleFactor(Fixed);

impl FixedScaleFactor {
  #[inline(always)]
  fn apply(self, value: i32) -> f32 {
    ({
      let Ok(value) = ({
        let Ok(value) = self.0.mul_div(
          ({
            let Ok(value) = Fixed::from_bits(value);
            value
          }),
          ({
            let Ok(value) = Fixed::from_bits(64);
            value
          }),
        );
        value
      })
      .to_f32();
      value
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use font_test_data::{NOTO_SANS_JP_CFF, SIMPLE_GLYF, VAZIRMATN_VAR};
  use read_fonts::FontRef;

  #[test]
  fn metrics() {
    let font = FontRef::new(SIMPLE_GLYF).unwrap();
    let metrics = font.metrics(Size::unscaled(), LocationRef::default());
    let expected = Metrics {
      units_per_em: 1024,
      glyph_count: 3,
      bounds: Some(BoundingBox {
        x_min: 51.0,
        y_min: -250.0,
        x_max: 998.0,
        y_max: 950.0,
      }),
      average_width: Some(1275.0),
      max_width: None,
      x_height: Some(512.0),
      cap_height: Some(717.0),
      is_monospace: false,
      italic_angle: 0.0,
      ascent: 950.0,
      descent: -250.0,
      leading: 0.0,
      underline: None,
      strikeout: Some(Decoration {
        offset: 307.0,
        thickness: 51.0,
      }),
    };
    assert_eq!(metrics, expected);
  }

  #[test]
  fn metrics_missing_os2() {
    let font = FontRef::new(VAZIRMATN_VAR).unwrap();
    let metrics = font.metrics(Size::unscaled(), LocationRef::default());
    let expected = Metrics {
      units_per_em: 2048,
      glyph_count: 4,
      bounds: Some(BoundingBox {
        x_min: 29.0,
        y_min: 0.0,
        x_max: 1310.0,
        y_max: 1847.0,
      }),
      average_width: None,
      max_width: Some(1336.0),
      x_height: None,
      cap_height: None,
      is_monospace: false,
      italic_angle: 0.0,
      ascent: 2100.0,
      descent: -1100.0,
      leading: 0.0,
      underline: None,
      strikeout: None,
    };
    assert_eq!(metrics, expected);
  }

  #[test]
  fn glyph_metrics() {
    let font = FontRef::new(VAZIRMATN_VAR).unwrap();
    let glyph_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());

    let expected = &[
      (908.0, 100.0),
      (1336.0, 29.0),
      (1336.0, 29.0),
      (633.0, 57.0),
    ];
    let result = (0..4)
      .map(|i| {
        let gid = ({
          let Ok(value) = GlyphId::new(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
              migrated_value
            }),
          );
          value
        });
        let advance_width = glyph_metrics.advance_width(gid).unwrap();
        let lsb = glyph_metrics.left_side_bearing(gid).unwrap();
        (advance_width, lsb)
      })
      .collect::<Vec<_>>();
    assert_eq!(expected, &result[..]);
  }

  /// Asserts that the results generated with Size::unscaled() and
  /// Size::new(upem) are equal.
  ///
  /// See <https://github.com/googlefonts/fontations/issues/590#issuecomment-1711595882>
  #[test]
  fn glyph_metrics_unscaled_matches_upem_scale() {
    let font = FontRef::new(VAZIRMATN_VAR).unwrap();
    let upem = f32::from(
      ({
        let Ok(value) = font.head().unwrap().units_per_em();
        value
      }),
    );
    let unscaled_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let upem_metrics = font.glyph_metrics(Size::new(upem), LocationRef::default());
    for i in 0..unscaled_metrics.glyph_count() {
      let gid = ({
        let Ok(value) = GlyphId::new(i);
        value
      });
      assert_eq!(
        unscaled_metrics.advance_width(gid),
        upem_metrics.advance_width(gid)
      );
      assert_eq!(
        unscaled_metrics.left_side_bearing(gid),
        upem_metrics.left_side_bearing(gid)
      );
    }
  }

  #[test]
  fn glyph_metrics_var() {
    let font = FontRef::new(VAZIRMATN_VAR).unwrap();
    let coords = &[({
      let Ok(value) = NormalizedCoord::from_f32(-0.8);
      value
    })];
    let glyph_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::new(coords));

    let expected = &[
      (908.0, 100.0),
      (1246.0, 29.0),
      (1246.0, 29.0),
      (556.0, 57.0),
    ];
    let result = (0..4)
      .map(|i| {
        let gid = ({
          let Ok(value) = GlyphId::new(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
              migrated_value
            }),
          );
          value
        });
        let advance_width = glyph_metrics.advance_width(gid).unwrap();
        let lsb = glyph_metrics.left_side_bearing(gid).unwrap();
        (advance_width, lsb)
      })
      .collect::<Vec<_>>();
    assert_eq!(expected, &result[..]);

    let coords = &[({
      let Ok(value) = NormalizedCoord::from_f32(-1.0);
      value
    })];
    let glyph_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::new(coords));
    let bounds = glyph_metrics
      .bounds(
        ({
          let Ok(value) = GlyphId::new(1);
          value
        }),
      )
      .unwrap();
    assert_eq!(
      bounds,
      BoundingBox {
        x_min: 33.0,
        y_min: 0.0,
        x_max: 1189.0,
        y_max: 1456.0
      }
    );
  }

  #[test]
  fn glyph_metrics_cff() {
    let font = FontRef::new(NOTO_SANS_JP_CFF).unwrap();
    let glyph_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
    let bounds = glyph_metrics
      .bounds(
        ({
          let Ok(value) = GlyphId::new(34);
          value
        }),
      )
      .unwrap();
    assert_eq!(
      bounds,
      BoundingBox {
        x_min: 4.0,
        y_min: 0.0,
        x_max: 604.0,
        y_max: 733.0
      }
    );
  }

  #[test]
  fn glyph_metrics_missing_hvar() {
    let font = FontRef::new(VAZIRMATN_VAR).unwrap();
    let glyph_count = ({
      let Ok(value) = font.maxp().unwrap().num_glyphs();
      value
    });

    for coord in [-1.0, -0.8, 0.0, 0.75, 1.0] {
      let coords = &[({
        let Ok(value) = NormalizedCoord::from_f32(coord);
        value
      })];
      let location = LocationRef::new(coords);
      let glyph_metrics = font.glyph_metrics(Size::unscaled(), location);
      let mut glyph_metrics_no_hvar = glyph_metrics.clone();

      glyph_metrics_no_hvar.hvar = None;
      for gid in 0..glyph_count {
        let gid = GlyphId::from(gid);
        assert_eq!(
          glyph_metrics.advance_width(gid),
          glyph_metrics_no_hvar.advance_width(gid)
        );
        assert_eq!(
          glyph_metrics.left_side_bearing(gid),
          glyph_metrics_no_hvar.left_side_bearing(gid)
        );
      }
    }
  }

  /// Ensure our fixed point scaling code matches FreeType for advances.
  ///
  /// <https://github.com/googlefonts/fontations/issues/590>
  #[test]
  fn match_freetype_glyph_metric_scaling() {
    let font_unit_advances = [639, 561, 524, 258];
    #[allow(clippy::excessive_precision)]
    let scaled_advances = [
      15.33595275878906250000,
      13.46395874023437500000,
      12.57595825195312500000,
      6.19198608398437500000,
    ];
    let fixed_scale = FixedScaleFactor(Size::new(24.0).fixed_linear_scale(1000));
    for (font_unit_advance, expected_scaled_advance) in
      font_unit_advances.iter().zip(scaled_advances)
    {
      let scaled_advance = fixed_scale.apply(*font_unit_advance);
      assert_eq!(scaled_advance, expected_scaled_advance);
    }
  }
}
