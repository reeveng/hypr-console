//! Autohinting state for a font instance.

use super::{
  super::{
    AdjustedMetrics, DrawError, OutlineGlyph, OutlineGlyphCollection, OutlinePen, Target,
    pen::PathStyle,
  },
  ScaleFlags,
  metrics::{Scale, UnscaledStyleMetricsSet, fixed_mul, pix_round},
  outline::Outline,
  shape::{Shaper, ShaperMode},
  style::{GlyphStyle, GlyphStyleMap},
};
use crate::{MetadataProvider, attribute::Style, prelude::Size};
use alloc::sync::Arc;
use raw::{
  FontRef, TableProvider,
  types::{F2Dot14, F26Dot6, GlyphId},
};

/// We enable "best effort" mode by default but allow it to be disabled with
/// a feature for testing.
const SHAPER_MODE: ShaperMode = match cfg!(feature = "autohint_shaping") {
  true => ShaperMode::BestEffort,
  false => ShaperMode::Nominal,
};

/// Set of derived glyph styles that are used for automatic hinting.
///
/// These are invariant per font so can be precomputed and reused for multiple
/// instances when requesting automatic hinting with [`Engine::Auto`](super::super::hint::Engine::Auto).
#[derive(Clone, Debug)]
pub struct GlyphStyles(Arc<GlyphStyleMap>);

impl GlyphStyles {
  /// Precomputes the full set of glyph styles for the given outlines.
  pub fn new(outlines: &OutlineGlyphCollection) -> Self {
    match outlines.font() {
      Some(font) => {
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
        let shaper = Shaper::new(font, SHAPER_MODE);
        Self(Arc::new(GlyphStyleMap::new(glyph_count, &shaper)))
      }
      None => Self(Default::default()),
    }
  }

  /// Returns the style for the given glyph.
  pub fn get(&self, gid: GlyphId) -> Option<GlyphStyle> {
    self.0.style(gid)
  }
}

#[derive(Clone)]
pub(crate) struct Instance {
  styles: GlyphStyles,
  metrics: UnscaledStyleMetricsSet,
  target: Target,
  is_fixed_width: bool,
  style: Style,
}

impl Instance {
  pub fn new(
    font: &FontRef,
    outlines: &OutlineGlyphCollection,
    coords: &[F2Dot14],
    target: Target,
    styles: Option<GlyphStyles>,
    lazy_metrics: bool,
  ) -> Self {
    let styles = styles.unwrap_or_else(|| GlyphStyles::new(outlines));
    #[cfg(feature = "std")]
    let metrics = match lazy_metrics {
      true => UnscaledStyleMetricsSet::lazy(&styles.0),
      false => UnscaledStyleMetricsSet::precomputed(font, coords, SHAPER_MODE, &styles.0),
    };
    #[cfg(not(feature = "std"))]
    let metrics = UnscaledStyleMetricsSet::precomputed(font, coords, SHAPER_MODE, &styles.0);
    let is_fixed_width = font
      .post()
      .map(|post| {
        let Ok(is_fixed_pitch) = post.is_fixed_pitch();

        is_fixed_pitch != 0
      })
      .unwrap_or_default();
    let style = font.attributes().style;
    Self {
      styles,
      metrics,
      target,
      is_fixed_width,
      style,
    }
  }

  pub fn draw(
    &self,
    size: Size,
    coords: &[F2Dot14],
    glyph: &OutlineGlyph,
    path_style: PathStyle,
    pen: &mut impl OutlinePen,
  ) -> Result<AdjustedMetrics, DrawError> {
    let font = glyph.font();
    let glyph_id = glyph.glyph_id();
    let style = self
      .styles
      .0
      .style(glyph_id)
      .ok_or(DrawError::GlyphNotFound(glyph_id))?;
    let metrics = (self
      .metrics
      .get(font, coords, SHAPER_MODE, &self.styles.0, glyph_id))
    .ok_or(DrawError::GlyphNotFound(glyph_id))?;
    let units_per_em = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, i32>((glyph.units_per_em()));
      migrated_value
    });
    let scale = Scale::new(
      size.ppem().unwrap_or(units_per_em as f32),
      units_per_em,
      self.style,
      self.target,
      metrics.style_class().script.group,
    );
    let mut outline = Outline::default();
    outline.fill(glyph, coords, super::QuirksMode::default())?;
    let hinted_metrics = super::hint::hint_outline(&mut outline, &metrics, &scale, Some(style));
    let h_advance = outline.advance;
    let mut pp1x = 0;
    let mut pp2x = fixed_mul(h_advance, hinted_metrics.x_scale);
    let light_target = matches!(
      self.target,
      console_core_font_hinting::Target::Smooth {
        mode: console_core_font_hinting::SmoothMode::Light,
        ..
      }
    );
    let preserve_linear_metrics = matches!(
      self.target,
      console_core_font_hinting::Target::Smooth {
        linear_metrics: console_core_font_hinting::LinearMetrics::Preserve,
        ..
      }
    );
    let is_light = light_target || preserve_linear_metrics;

    match !is_light {
      true => {
        if let (true, Some(edge_metrics)) = (
          !scale.flags.contains(ScaleFlags::NO_ADVANCE),
          hinted_metrics.edge_metrics,
        ) {
          let old_rsb = pp2x - edge_metrics.right_opos;
          let old_lsb = edge_metrics.left_opos;
          let new_lsb = edge_metrics.left_pos;
          let mut pp1x_uh = new_lsb - old_lsb;
          let mut pp2x_uh = edge_metrics.right_pos + old_rsb;
          match old_lsb < 24 {
            true => {
              pp1x_uh -= 8;
            }
            false => {}
          }
          match old_rsb < 24 {
            true => {
              pp2x_uh += 8;
            }
            false => {}
          }
          pp1x = pix_round(pp1x_uh);
          pp2x = pix_round(pp2x_uh);
          match pp1x >= new_lsb && old_lsb > 0 {
            true => {
              pp1x -= 64;
            }
            false => {}
          }
          match pp2x <= edge_metrics.right_pos && old_rsb > 0 {
            true => {
              pp2x += 64;
            }
            false => {}
          }
        } else {
          pp1x = pix_round(pp1x);
          pp2x = pix_round(pp2x);
        }
      }
      false => {
        pp1x = pix_round(pp1x);
        pp2x = pix_round(pp2x);
      }
    }
    match pp1x != 0 {
      true => {
        for point in &mut outline.points {
          point.x -= pp1x;
        }
      }
      false => {}
    }
    let advance = match !is_light
      && (self.is_fixed_width || (metrics.digits_have_same_width && style.is_digit()))
    {
      true => fixed_mul(h_advance, scale.x_scale),
      false => match h_advance != 0 {
        true => pp2x - pp1x,
        false => 0,
      },
    };
    outline.to_path(path_style, pen)?;
    Ok(AdjustedMetrics {
      has_overlaps: glyph.has_overlaps().unwrap_or_default(),
      lsb: None,
      advance_width: Some(
        ({
          let Ok(value) = {
            let Ok(value) = F26Dot6::from_bits(pix_round(advance));
            value
          }
          .to_f32();
          value
        }),
      ),
    })
  }
}
