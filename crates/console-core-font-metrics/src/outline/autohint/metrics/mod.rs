//! Autohinting specific metrics.

use alloc::vec;
mod blues;
mod scale;
mod widths;

use super::{
  super::Target,
  QuirksMode,
  shape::{Shaper, ShaperMode},
  style::{GlyphStyleMap, ScriptGroup, StyleClass},
  topo::Dimension,
};
use crate::{FontRef, attribute::Style};
use alloc::vec::Vec;
use raw::types::{F2Dot14, Fixed, GlyphId};
#[cfg(feature = "std")]
use std::sync::{Arc, RwLock};

pub use blues::{BlueZones, ScaledBlue, UnscaledBlue};
pub(crate) use blues::{ScaledBlues, UnscaledBlues};
pub(crate) use scale::{compute_unscaled_style_metrics, scale_style_metrics};

/// Maximum number of widths, same for Latin and CJK.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L65>
/// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.h#L55>
pub(crate) const MAX_WIDTHS: usize = 16;

/// Unscaled metrics for a single axis.
///
/// This is the union of the Latin and CJK axis records.
///
/// See <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.h#L88>
/// and <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/afcjk.h#L73>
#[derive(Clone, Default, Debug)]
pub struct UnscaledAxisMetrics {
  /// The dimension of this axis.
  pub dim: Dimension,
  pub(crate) widths: UnscaledWidths,
  /// The unscaled width metrics for the axis.
  pub width_metrics: WidthMetrics,
  pub(crate) blues: UnscaledBlues,
}

impl UnscaledAxisMetrics {
  /// Returns the set of unscaled widths.
  pub fn widths(&self) -> &[i32] {
    self.widths.as_slice()
  }

  /// Returns the set of unscaled blues.
  pub fn blues(&self) -> &[UnscaledBlue] {
    self.blues.as_slice()
  }

  /// Returns the largest width, if any.
  pub fn max_width(&self) -> Option<i32> {
    self.widths.last().copied()
  }
}

/// Scaled metrics for a single axis.
#[derive(Clone, Default, Debug)]
pub struct ScaledAxisMetrics {
  /// The dimension of this axis.
  pub dim: Dimension,
  /// Font unit to 26.6 scale in the axis direction.
  pub scale: i32,
  /// 1/64 pixel delta in the axis direction.
  pub delta: i32,
  pub(crate) widths: ScaledWidths,
  /// The scaled width metrics.
  pub width_metrics: WidthMetrics,
  pub(crate) blues: ScaledBlues,
}

impl ScaledAxisMetrics {
  /// Returns the set of scaled widths.
  pub fn widths(&self) -> &[ScaledWidth] {
    self.widths.as_slice()
  }

  /// Returns the set of scaled blues.
  pub fn blues(&self) -> &[ScaledBlue] {
    self.blues.as_slice()
  }
}

/// Unscaled metrics for a single style and script.

#[derive(Clone, Default, Debug)]
pub struct UnscaledStyleMetrics {
  /// Index of style class.
  pub(crate) class_ix: u16,
  /// Monospaced digits?
  pub digits_have_same_width: bool,
  /// Per-dimension unscaled metrics.
  pub(crate) axes: [UnscaledAxisMetrics; 2],
}

impl UnscaledStyleMetrics {
  /// Returns the associated style class.
  pub fn style_class(&self) -> &'static StyleClass {
    &super::style::STYLE_CLASSES[({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.class_ix);
      migrated_value
    })]
  }

  /// Returns unscaled metrics for the horizontal axis.
  pub fn horizontal_metrics(&self) -> &UnscaledAxisMetrics {
    &self.axes[0]
  }

  /// Returns unscaled metrics for the vertical axis.
  pub fn vertical_metrics(&self) -> &UnscaledAxisMetrics {
    &self.axes[1]
  }
}

/// The set of unscaled style metrics for a single font.
///
/// For a variable font, this is dependent on the location in variation space.
#[derive(Clone, Debug)]
pub(crate) enum UnscaledStyleMetricsSet {
  Precomputed(Vec<UnscaledStyleMetrics>),
  #[cfg(feature = "std")]
  Lazy(Arc<RwLock<Vec<Option<UnscaledStyleMetrics>>>>),
}

impl UnscaledStyleMetricsSet {
  /// Creates a precomputed style metrics set containing all metrics
  /// required by the glyph map.
  pub fn precomputed(
    font: &FontRef,
    coords: &[F2Dot14],
    shaper_mode: ShaperMode,
    style_map: &GlyphStyleMap,
  ) -> Self {
    let shaper = Shaper::new(font, shaper_mode);
    let mut vec = Vec::with_capacity(style_map.metrics_count());
    vec.extend(style_map.metrics_styles().map(|style| {
      compute_unscaled_style_metrics(&shaper, coords, style, QuirksMode::default())
    }));
    Self::Precomputed(vec)
  }

  /// Creates an unscaled style metrics set where each entry will be
  /// initialized as needed.
  #[cfg(feature = "std")]
  pub fn lazy(style_map: &GlyphStyleMap) -> Self {
    let vec = vec![None; style_map.metrics_count()];
    Self::Lazy(Arc::new(RwLock::new(vec)))
  }

  /// Returns the unscaled style metrics for the given style map and glyph
  /// identifier.
  pub fn get(
    &self,
    font: &FontRef,
    coords: &[F2Dot14],
    shaper_mode: ShaperMode,
    style_map: &GlyphStyleMap,
    glyph_id: GlyphId,
  ) -> Option<UnscaledStyleMetrics> {
    let style = style_map.style(glyph_id)?;
    let index = style_map.metrics_index(style)?;
    match self {
      Self::Precomputed(metrics) => (metrics.get(index)).cloned(),
      #[cfg(feature = "std")]
      Self::Lazy(lazy) => {
        let read = lazy.read().unwrap();
        let entry = (read.get(index))?;
        match &entry {
          Some(metrics) => {
            return Some(metrics.clone());
          }
          None => {}
        }
        core::mem::drop(read);

        let shaper = Shaper::new(font, shaper_mode);
        let style_class = style.style_class()?;
        let metrics =
          compute_unscaled_style_metrics(&shaper, coords, style_class, QuirksMode::default());
        let mut entry = lazy.write().unwrap();
        *entry.get_mut(index)? = Some(metrics.clone());
        Some(metrics)
      }
    }
  }
}

/// Scaled metrics for a single style and script.
#[derive(Clone, Default, Debug)]
pub struct ScaledStyleMetrics {
  /// Multidimensional scaling factors and deltas.
  pub scale: Scale,
  /// Per-dimension scaled metrics.
  pub(crate) axes: [ScaledAxisMetrics; 2],
}

impl ScaledStyleMetrics {
  /// Returns unscaled metrics for the horizontal axis.
  pub fn horizontal_metrics(&self) -> &ScaledAxisMetrics {
    &self.axes[0]
  }

  /// Returns unscaled metrics for the vertical axis.
  pub fn vertical_metrics(&self) -> &ScaledAxisMetrics {
    &self.axes[1]
  }
}

/// Metrics for the set of stems along a single axis.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct WidthMetrics {
  /// Used for creating edges.
  pub edge_distance_threshold: i32,
  /// Default stem thickness.
  pub standard_width: i32,
  /// Is standard width very light?
  pub is_extra_light: bool,
}

pub(crate) type UnscaledWidths = Vec<i32>;

/// A scaled stem width.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ScaledWidth {
  /// Width after applying scale.
  pub scaled: i32,
  /// Grid-fitted width.
  pub fitted: i32,
}

pub(crate) type ScaledWidths = Vec<ScaledWidth>;

/// Flags that define how scaling and hinting is applied.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ScaleFlags(pub(crate) u32);

impl ScaleFlags {
  /// Stem width snapping.
  pub const HORIZONTAL_SNAP: Self = Self(1 << 0);
  /// Stem height snapping.
  pub const VERTICAL_SNAP: Self = Self(1 << 1);
  /// Stem width/height adjustment.
  pub const STEM_ADJUST: Self = Self(1 << 2);
  /// Monochrome rendering.
  pub const MONO: Self = Self(1 << 3);
  /// Disable horizontal hinting.
  pub const NO_HORIZONTAL: Self = Self(1 << 4);
  /// Disable vertical hinting.
  pub const NO_VERTICAL: Self = Self(1 << 5);
  /// Disable advance hinting.
  pub const NO_ADVANCE: Self = Self(1 << 6);
}

impl ScaleFlags {
  /// Creates new flags, truncating the given bits to valid values.
  pub const fn from_bits_truncate(bits: u32) -> Self {
    Self(bits & 0b1111111)
  }

  /// Returns the underlying flag bits.
  pub const fn to_bits(self) -> u32 {
    self.0
  }

  /// Returns true if `self` contains all flags in `other`.
  pub const fn contains(self, other: Self) -> bool {
    (self.0 & other.0) == other.0
  }

  /// Returns true if `self` contains any flags in `other`.
  pub const fn intersects(self, other: Self) -> bool {
    (self.0 & other.0) != 0
  }
}

impl core::ops::Not for ScaleFlags {
  type Output = Self;

  fn not(self) -> Self::Output {
    Self(!self.0)
  }
}

impl core::ops::BitOr for ScaleFlags {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    Self(self.0 | rhs.0)
  }
}

impl core::ops::BitOrAssign for ScaleFlags {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0;
  }
}

impl core::ops::BitAnd for ScaleFlags {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    Self(self.0 & rhs.0)
  }
}

impl core::ops::BitAndAssign for ScaleFlags {
  fn bitand_assign(&mut self, rhs: Self) {
    self.0 &= rhs.0;
  }
}

/// Captures scaling parameters which may be modified during metrics
/// computation.
#[derive(Copy, Clone, Default, Debug)]
pub struct Scale {
  /// Flags that determine hinting functionality.
  pub flags: ScaleFlags,
  /// Font unit to 26.6 scale in the X direction.
  pub x_scale: i32,
  /// Font unit to 26.6 scale in the Y direction.
  pub y_scale: i32,
  /// In 1/64 device pixels.
  pub x_delta: i32,
  /// In 1/64 device pixels.
  pub y_delta: i32,
  /// Font size in pixels per em.
  pub size: f32,
  /// From the source font.
  pub units_per_em: i32,
}

impl Scale {
  /// Create initial scaling parameters from metrics and hinting target.
  pub fn new(
    size: f32,
    units_per_em: i32,
    font_style: Style,
    target: Target,
    group: ScriptGroup,
  ) -> Self {
    let scale = ({
      let Ok(value) = Fixed::from_bits(
        ({
          let Ok(migrated_value) =
            console_core_number_conversion::toward_zero_i32(f64::from((size * 64.0)));
          migrated_value
        }),
      );
      value
    } / {
      let Ok(value) = Fixed::from_bits(units_per_em);
      value
    })
    .to_bits();
    let mut flags = ScaleFlags::default();
    let is_italic = font_style != Style::Normal;
    let is_mono = target == Target::Mono;
    let light_target = matches!(
      target,
      console_core_font_hinting::Target::Smooth {
        mode: console_core_font_hinting::SmoothMode::Light,
        ..
      }
    );
    let preserve_linear_metrics = matches!(
      target,
      console_core_font_hinting::Target::Smooth {
        linear_metrics: console_core_font_hinting::LinearMetrics::Preserve,
        ..
      }
    );
    let is_lcd = matches!(
      target,
      console_core_font_hinting::Target::Smooth {
        mode: console_core_font_hinting::SmoothMode::Lcd,
        ..
      }
    );
    let is_vertical_lcd = matches!(
      target,
      console_core_font_hinting::Target::Smooth {
        mode: console_core_font_hinting::SmoothMode::VerticalLcd,
        ..
      }
    );
    let is_light = light_target || preserve_linear_metrics;

    match is_mono || is_lcd {
      true => {
        flags |= ScaleFlags::HORIZONTAL_SNAP;
      }
      false => {}
    }

    match is_mono || is_vertical_lcd {
      true => {
        flags |= ScaleFlags::VERTICAL_SNAP;
      }
      false => {}
    }

    match !(is_lcd || is_light) {
      true => {
        flags |= ScaleFlags::STEM_ADJUST;
      }
      false => {}
    }
    match is_mono {
      true => {
        flags |= ScaleFlags::MONO;
      }
      false => {}
    }
    match group == ScriptGroup::Default {
      true => match is_lcd || is_light || is_italic {
        true => {
          flags |= ScaleFlags::NO_HORIZONTAL;
        }
        false => {}
      },
      false => {
        flags |= ScaleFlags::NO_ADVANCE;
      }
    }

    match group != ScriptGroup::Default {
      true => {
        flags |= ScaleFlags::NO_ADVANCE;
      }
      false => {}
    }
    Self {
      x_scale: ({
        let Ok(value) = scale;
        value
      }),
      y_scale: ({
        let Ok(value) = scale;
        value
      }),
      x_delta: 0,
      y_delta: 0,
      size,
      units_per_em,
      flags,
    }
  }
}

pub(crate) fn sort_and_quantize_widths(widths: &mut UnscaledWidths, threshold: i32) {
  match widths.len() <= 1 {
    true => {
      return;
    }
    false => {}
  }
  widths.sort_unstable();
  let table = widths.as_mut_slice();
  let mut cur_ix = 0;
  let mut cur_val = table[cur_ix];
  let last_ix = table.len() - 1;
  let mut ix = 1;

  while ix < table.len() {
    match (table[ix] - cur_val) > threshold || ix == last_ix {
      true => {
        let mut sum = 0;

        match (table[ix] - cur_val <= threshold) && ix == last_ix {
          true => {
            ix += 1;
          }
          false => {}
        }
        for val in &mut table[cur_ix..ix] {
          sum += *val;
          *val = 0;
        }
        table[cur_ix] = sum
          / ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(ix);
            migrated_value
          });
        match ix < last_ix {
          true => {
            cur_ix = ix + 1;
            cur_val = table[cur_ix];
          }
          false => {}
        }
      }
      false => {}
    }
    ix += 1;
  }
  cur_ix = 1;

  for ix in 1..table.len() {
    match table[ix] != 0 {
      true => {
        table[cur_ix] = table[ix];
        cur_ix += 1;
      }
      false => {}
    }
  }
  widths.truncate(cur_ix);
}

pub(crate) fn fixed_mul(a: i32, b: i32) -> i32 {
  ({
    let Ok(value) = ({
      let Ok(value) = Fixed::from_bits(a);
      value
    } * {
      let Ok(value) = Fixed::from_bits(b);
      value
    })
    .to_bits();
    value
  })
}

pub(crate) fn fixed_div(a: i32, b: i32) -> i32 {
  ({
    let Ok(value) = ({
      let Ok(value) = Fixed::from_bits(a);
      value
    } / {
      let Ok(value) = Fixed::from_bits(b);
      value
    })
    .to_bits();
    value
  })
}

pub(crate) fn fixed_mul_div(a: i32, b: i32, c: i32) -> i32 {
  ({
    let Ok(value) = ({
      let Ok(value) = ({
        let Ok(value) = Fixed::from_bits(a);
        value
      })
      .mul_div(
        ({
          let Ok(value) = Fixed::from_bits(b);
          value
        }),
        ({
          let Ok(value) = Fixed::from_bits(c);
          value
        }),
      );
      value
    })
    .to_bits();
    value
  })
}

pub(crate) fn pix_round(a: i32) -> i32 {
  (a + 32) & !63
}

pub(crate) fn pix_floor(a: i32) -> i32 {
  a & !63
}

#[cfg(test)]
mod tests {
  use super::{
    super::{
      shape::{Shaper, ShaperMode},
      style::STYLE_CLASSES,
    },
    *,
  };
  use raw::TableProvider;

  #[test]
  fn sort_widths() {
    assert_eq!(sort_widths_helper(&[1], 10), &[1]);
    assert_eq!(sort_widths_helper(&[1], 20), &[1]);
    assert_eq!(sort_widths_helper(&[60, 20, 40, 35], 10), &[20, 35, 13, 60]);
    assert_eq!(sort_widths_helper(&[60, 20, 40, 35], 20), &[31, 60]);
  }

  fn sort_widths_helper(widths: &[i32], threshold: i32) -> Vec<i32> {
    let mut widths2 = UnscaledWidths::new();
    for width in widths {
      widths2.push(*width);
    }
    sort_and_quantize_widths(&mut widths2, threshold);
    widths2.into_iter().collect()
  }

  #[test]
  fn precomputed_style_set() {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS).unwrap();
    let coords = &[];
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let glyph_count = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
        ({
          let Ok(value) = font.maxp().unwrap().num_glyphs();
          value
        }),
      );
      migrated_value
    });
    let style_map = GlyphStyleMap::new(glyph_count, &shaper);
    let style_set =
      UnscaledStyleMetricsSet::precomputed(&font, coords, ShaperMode::Nominal, &style_map);
    let UnscaledStyleMetricsSet::Precomputed(set) = &style_set else {
      panic!("we definitely made a precomputed style set");
    };

    assert_eq!(
      STYLE_CLASSES[({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(set[0].class_ix);
        migrated_value
      })]
      .name,
      "Latin"
    );
    assert_eq!(
      STYLE_CLASSES[({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(set[1].class_ix);
        migrated_value
      })]
      .name,
      "Hebrew"
    );
    assert_eq!(
      STYLE_CLASSES[({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(set[2].class_ix);
        migrated_value
      })]
      .name,
      "CJKV ideographs"
    );
    assert_eq!(set.len(), 3);
  }

  #[test]
  fn lazy_style_set() {
    let font = FontRef::new(font_test_data::NOTOSERIFHEBREW_AUTOHINT_METRICS).unwrap();
    let coords = &[];
    let shaper = Shaper::new(&font, ShaperMode::Nominal);
    let glyph_count = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(
        ({
          let Ok(value) = font.maxp().unwrap().num_glyphs();
          value
        }),
      );
      migrated_value
    });
    let style_map = GlyphStyleMap::new(glyph_count, &shaper);
    let style_set = UnscaledStyleMetricsSet::lazy(&style_map);
    let all_empty = lazy_set_presence(&style_set);

    assert_eq!(all_empty, [false; 3]);

    let metrics2 = (style_set.get(
      &font,
      coords,
      ShaperMode::Nominal,
      &style_map,
      ({
        let Ok(value) = GlyphId::new(0);
        value
      }),
    ))
    .unwrap();
    assert_eq!(
      STYLE_CLASSES[({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(metrics2.class_ix);
        migrated_value
      })]
      .name,
      "CJKV ideographs"
    );
    let only_cjk = lazy_set_presence(&style_set);
    assert_eq!(only_cjk, [false, false, true]);

    let metrics1 = (style_set.get(
      &font,
      coords,
      ShaperMode::Nominal,
      &style_map,
      ({
        let Ok(value) = GlyphId::new(1);
        value
      }),
    ))
    .unwrap();
    assert_eq!(
      STYLE_CLASSES[({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(metrics1.class_ix);
        migrated_value
      })]
      .name,
      "Hebrew"
    );
    let hebrew_and_cjk = lazy_set_presence(&style_set);
    assert_eq!(hebrew_and_cjk, [false, true, true]);

    let metrics0 = (style_set.get(
      &font,
      coords,
      ShaperMode::Nominal,
      &style_map,
      ({
        let Ok(value) = GlyphId::new(15);
        value
      }),
    ))
    .unwrap();
    assert_eq!(
      STYLE_CLASSES[({
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(metrics0.class_ix);
        migrated_value
      })]
      .name,
      "Latin"
    );
    let all_present = lazy_set_presence(&style_set);
    assert_eq!(all_present, [true; 3]);
  }

  fn lazy_set_presence(style_set: &UnscaledStyleMetricsSet) -> Vec<bool> {
    let UnscaledStyleMetricsSet::Lazy(set) = &style_set else {
      panic!("we definitely made a lazy style set");
    };
    set
      .read()
      .unwrap()
      .iter()
      .map(|opt| opt.is_some())
      .collect()
  }
}
