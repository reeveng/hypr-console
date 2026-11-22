//! Support for applying embedded hinting instructions.

use super::{
  AdjustedMetrics, DrawError, GlyphStyles, Hinting, LocationRef, NormalizedCoord,
  OutlineCollectionKind, OutlineGlyph, OutlineGlyphCollection, OutlineKind, OutlinePen, Size,
  autohint, cff,
  glyf::{self, FreeTypeScaler},
  pen::PathStyle,
};
use crate::alloc::{boxed::Box, vec::Vec};
use raw::types::Fixed;

/// Configuration settings for a hinting instance.
#[derive(Clone, Default, Debug)]
pub struct HintingOptions {
  /// Specifies the hinting engine to use.
  ///
  /// Defaults to [`Engine::AutoFallback`].
  pub engine: Engine,
  /// Defines the properties of the intended target of a hinted outline.
  ///
  /// Defaults to a target with [`SmoothMode::Normal`] which is equivalent
  /// to `FT_RENDER_MODE_NORMAL` in FreeType.
  pub target: Target,
}

impl From<Target> for HintingOptions {
  fn from(value: Target) -> Self {
    Self {
      engine: Engine::AutoFallback,
      target: value,
    }
  }
}

/// Specifies the backend to use when applying hints.
#[derive(Clone, Default, Debug)]
pub enum Engine {
  /// The TrueType or PostScript interpreter.
  Interpreter,
  /// The automatic hinter that performs just-in-time adjustment of
  /// outlines.
  ///
  /// Glyph styles can be precomputed per font and may be provided here
  /// as an optimization to avoid recomputing them for each instance.
  Auto(Option<GlyphStyles>),
  /// Selects the engine based on the same rules that FreeType uses when
  /// neither of the `FT_LOAD_NO_AUTOHINT` or `FT_LOAD_FORCE_AUTOHINT`
  /// load flags are specified.
  ///
  /// Specifically, PostScript (CFF/CFF2) fonts will always use the hinting
  /// engine in the PostScript interpreter and TrueType fonts will use the
  /// interpreter for TrueType instructions if one of the `fpgm` or `prep`
  /// tables is non-empty, falling back to the automatic hinter otherwise.
  ///
  /// This uses [`OutlineGlyphCollection::prefer_interpreter`] to make a
  /// selection.
  #[default]
  AutoFallback,
}

impl Engine {
  /// Converts the `AutoFallback` variant into either `Interpreter` or
  /// `Auto` based on the given outline set's preference for interpreter
  /// mode.
  fn resolve_auto_fallback(self, outlines: &OutlineGlyphCollection) -> Engine {
    match self {
      Self::Interpreter => Self::Interpreter,
      Self::Auto(styles) => Self::Auto(styles),
      Self::AutoFallback => match outlines.prefer_interpreter() {
        true => Self::Interpreter,
        false => Self::Auto(None),
      },
    }
  }
}

impl From<Engine> for HintingOptions {
  fn from(value: Engine) -> Self {
    Self {
      engine: value,
      target: Default::default(),
    }
  }
}

pub use console_core_font_hinting::{LinearMetrics, SmoothMode, SymmetricRendering, Target};

/// Modes that control hinting when using embedded instructions.
///
/// Only the TrueType interpreter supports all hinting modes.
///
/// # FreeType compatibility
///
/// The following table describes how to map FreeType hinting modes:
///
/// | FreeType mode         | Variant                                                                              |
/// |-----------------------|--------------------------------------------------------------------------------------|
/// | FT_LOAD_TARGET_MONO   | Strong                                                                               |
/// | FT_LOAD_TARGET_NORMAL | Smooth { lcd_subpixel: None, linear_metrics: LinearMetrics::Hint }                        |
/// | FT_LOAD_TARGET_LCD    | Smooth { lcd_subpixel: Some(LcdLayout::Horizontal), linear_metrics: LinearMetrics::Hint } |
/// | FT_LOAD_TARGET_LCD_V  | Smooth { lcd_subpixel: Some(LcdLayout::Vertical), linear_metrics: LinearMetrics::Hint }   |
///
/// Note: `FT_LOAD_TARGET_LIGHT` is equivalent to `FT_LOAD_TARGET_NORMAL` since
/// FreeType 2.7.
///
/// The default value of this type is equivalent to `FT_LOAD_TARGET_NORMAL`.
#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HintingMode {
  /// Strong hinting mode that should only be used for aliased, monochromatic
  /// rasterization.
  ///
  /// Corresponds to `FT_LOAD_TARGET_MONO` in FreeType.
  Strong,
  /// Lighter hinting mode that is intended for anti-aliased rasterization.
  Smooth {
    /// If set, enables support for optimized hinting that takes advantage
    /// of subpixel layouts in LCD displays and corresponds to
    /// `FT_LOAD_TARGET_LCD` or `FT_LOAD_TARGET_LCD_V` in FreeType.
    ///
    /// If unset, corresponds to `FT_LOAD_TARGET_NORMAL` in FreeType.
    lcd_subpixel: Option<LcdLayout>,
    /// Preserving linear metrics prevents adjustment of the outline in the horizontal
    /// direction and preserves inter-glyph spacing.
    ///
    /// This is useful for performing layout without concern that hinting
    /// will modify the advance width of a glyph. Specifically, it means
    /// that layout will not require evaluation of glyph outlines.
    ///
    /// FreeType has no corresponding setting.
    linear_metrics: LinearMetrics,
  },
}

impl Default for HintingMode {
  fn default() -> Self {
    Self::Smooth {
      lcd_subpixel: None,
      linear_metrics: LinearMetrics::Hint,
    }
  }
}

impl From<HintingMode> for HintingOptions {
  fn from(value: HintingMode) -> Self {
    let target = match value {
      HintingMode::Strong => Target::Mono,
      HintingMode::Smooth {
        lcd_subpixel,
        linear_metrics,
      } => {
        let mode = match lcd_subpixel {
          Some(LcdLayout::Horizontal) => SmoothMode::Lcd,
          Some(LcdLayout::Vertical) => SmoothMode::VerticalLcd,
          None => SmoothMode::Normal,
        };
        Target::Smooth {
          mode,
          linear_metrics,
          symmetric_rendering: SymmetricRendering::Enabled,
        }
      }
    };
    target.into()
  }
}

/// Specifies direction of pixel layout for LCD based subpixel hinting.
#[doc(hidden)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LcdLayout {
  /// Subpixels are ordered horizontally.
  ///
  /// Corresponds to `FT_LOAD_TARGET_LCD` in FreeType.
  Horizontal,
  /// Subpixels are ordered vertically.
  ///
  /// Corresponds to `FT_LOAD_TARGET_LCD_V` in FreeType.
  Vertical,
}

/// Hinting instance that uses information embedded in the font to perform
/// grid-fitting.
#[derive(Clone)]
pub struct HintingInstance {
  size: Size,
  coords: Vec<NormalizedCoord>,
  target: Target,
  kind: HinterKind,
}

impl HintingInstance {
  /// Creates a new embedded hinting instance for the given outline
  /// collection, size, location in variation space and hinting mode.
  pub fn new<'a>(
    outline_glyphs: &OutlineGlyphCollection,
    size: Size,
    location: impl Into<LocationRef<'a>>,
    options: impl Into<HintingOptions>,
  ) -> Result<Self, DrawError> {
    let options = options.into();
    let mut hinter = Self {
      size: Size::unscaled(),
      coords: vec![],
      target: options.target,
      kind: HinterKind::None,
    };
    hinter.reconfigure(outline_glyphs, size, location, options)?;
    Ok(hinter)
  }

  /// Returns the currently configured size.
  pub fn size(&self) -> Size {
    self.size
  }

  /// Returns the currently configured normalized location in variation space.
  pub fn location(&self) -> LocationRef<'_> {
    LocationRef::new(&self.coords)
  }

  /// Returns the currently configured hinting target.
  pub fn target(&self) -> Target {
    self.target
  }

  /// Resets the hinter state for a new font instance with the given
  /// outline collection and settings.
  pub fn reconfigure<'a>(
    &mut self,
    outlines: &OutlineGlyphCollection,
    size: Size,
    location: impl Into<LocationRef<'a>>,
    options: impl Into<HintingOptions>,
  ) -> Result<(), DrawError> {
    self.size = size;
    self.coords.clear();
    self
      .coords
      .extend_from_slice(location.into().effective_coords());
    let options = options.into();
    self.target = options.target;
    let engine = options.engine.resolve_auto_fallback(outlines);

    let current_kind = core::mem::replace(&mut self.kind, HinterKind::None);
    match engine {
      Engine::Interpreter => match &outlines.kind {
        OutlineCollectionKind::Glyf(glyf) => {
          let mut hint_instance = match current_kind {
            HinterKind::Glyf(instance) => instance,
            _ => Box::<glyf::HintInstance>::default(),
          };
          let ppem = size.ppem();
          let scale = glyf.compute_hinted_scale(ppem).to_bits();

          let rounded_ppem = (({
            let Ok(value) = ({
              let Ok(value) = Fixed::from_bits(scale);
              value
            } * {
              let Ok(value) = Fixed::from_bits(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i32>((glyf.units_per_em()));
                  migrated_value
                }),
              );
              value
            })
            .to_bits();
            value
          }) + 32)
            >> 6;
          hint_instance.reconfigure(
            &glyf.program,
            console_core_font_hinting::Scale {
              factor: ({
                let Ok(value) = Fixed::from_bits(scale);
                value
              }),
              ppem: rounded_ppem,
            },
            self.target,
            &self.coords,
          )?;
          self.kind = HinterKind::Glyf(hint_instance);
        }
        OutlineCollectionKind::Cff(cff) => {
          let mut subfonts = match current_kind {
            HinterKind::Cff(subfonts) => subfonts,
            _ => vec![],
          };
          subfonts.clear();
          let ppem = size.ppem();
          for i in 0..cff.subfont_count() {
            subfonts.push(cff.subfont(i, ppem, &self.coords)?);
          }
          self.kind = HinterKind::Cff(subfonts);
        }
        OutlineCollectionKind::Varc(..) => {
          self.kind = HinterKind::Varc;
        }
        OutlineCollectionKind::None => {}
      },
      Engine::Auto(styles) => {
        let font = match outlines.font() {
          Some(font) => font,
          None => {
            return Ok(());
          }
        };
        let instance =
          autohint::Instance::new(font, outlines, &self.coords, self.target, styles, true);
        self.kind = HinterKind::Auto(instance);
      }
      _ => {}
    }
    Ok(())
  }

  /// Returns true if hinting should actually be applied for this instance.
  ///
  /// Some TrueType fonts disable hinting dynamically based on the instance
  /// configuration.
  pub fn is_enabled(&self) -> bool {
    match &self.kind {
      HinterKind::Glyf(instance) => {
        let Ok(execution) = instance.execution();
        matches!(execution, console_core_font_hinting::Execution::Enabled)
      }
      HinterKind::Cff(_) | HinterKind::Auto(_) | HinterKind::Varc => true,
      _ => false,
    }
  }

  pub(super) fn draw(
    &self,
    glyph: &OutlineGlyph,
    memory: Option<&mut [u8]>,
    path_style: PathStyle,
    pen: &mut impl OutlinePen,
    is_pedantic: bool,
  ) -> Result<AdjustedMetrics, DrawError> {
    let ppem = self.size.ppem();
    let coords = self.coords.as_slice();
    match (&self.kind, &glyph.kind) {
      (HinterKind::Auto(instance), _) => instance.draw(self.size, coords, glyph, path_style, pen),
      (HinterKind::Glyf(instance), OutlineKind::Glyf(glyf, outline)) => {
        match matches!(path_style, PathStyle::HarfBuzz) {
          true => {
            return Err(DrawError::HarfBuzzHintingUnsupported);
          }
          false => {}
        }
        super::with_temporary_memory(glyph, Hinting::Embedded, memory, |buf| {
          let scaled_outline =
            FreeTypeScaler::hinted(glyf, outline, buf, ppem, coords, instance, is_pedantic)?
              .scale(&outline.glyph, outline.glyph_id)?;
          scaled_outline.to_path(path_style, pen)?;
          Ok(AdjustedMetrics {
            has_overlaps: outline.has_overlaps,
            lsb: Some({
              let Ok(value) = scaled_outline.adjusted_lsb().to_f32();
              value
            }),

            advance_width: Some(
              ({
                let Ok(value) = ({
                  let Ok(value) = scaled_outline.adjusted_advance_width().round();
                  value
                })
                .to_f32();
                value
              }),
            ),
          })
        })
      }
      (HinterKind::Cff(subfonts), OutlineKind::Cff(cff, glyph_id, subfont_ix)) => {
        let subfont = match (subfonts.get(
          ({
            let Ok(migrated_value) = console_core_number_conversion::wrapped_index(*subfont_ix);
            migrated_value
          }),
        )) {
          Some(subfont) => subfont,
          None => {
            return Err(DrawError::NoSources);
          }
        };
        let advance_width = cff.draw(subfont, *glyph_id, &self.coords, true, pen)?;
        Ok(AdjustedMetrics {
          has_overlaps: false,
          lsb: None,
          advance_width,
        })
      }
      (HinterKind::Varc, OutlineKind::Varc(varc, outline)) => {
        super::with_temporary_memory(glyph, Hinting::None, memory, |buf| {
          varc.draw(outline, buf, self.size, &self.coords, path_style, pen)?;
          Ok(AdjustedMetrics::default())
        })
      }
      _ => Err(DrawError::NoSources),
    }
  }
}

#[derive(Clone)]
enum HinterKind {
  /// Represents a hinting instance that is associated with an empty outline
  /// collection.
  None,
  Glyf(Box<glyf::HintInstance>),
  Cff(Vec<cff::Subfont>),
  Varc,
  Auto(autohint::Instance),
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    FontRef, GlyphId, MetadataProvider,
    outline::{
      DrawSettings,
      pen::{NullPen, SvgPen},
    },
    raw::TableProvider,
  };

  #[test]
  fn ignore_hdmx_when_back_compat_enabled() {
    let font = FontRef::new(font_test_data::TINOS_SUBSET).unwrap();
    let outlines = font.outline_glyphs();

    let gid = font.charmap().map('"').unwrap();
    let font_size = 16;
    let hinter = HintingInstance::new(
      &outlines,
      Size::new(f32::from(font_size)),
      LocationRef::default(),
      HintingOptions::default(),
    )
    .unwrap();
    let HinterKind::Glyf(tt_hinter) = &hinter.kind else {
      panic!("this is definitely a TrueType hinter");
    };

    assert_eq!(
      tt_hinter.backward_compatibility(),
      Ok(console_core_font_hinting::BackwardCompatibility::Enabled)
    );
    let outline = outlines.get(gid).unwrap();
    let metrics = outline.draw(&hinter, &mut NullPen).unwrap();

    let scaler_advance = metrics.advance_width.unwrap();
    assert_eq!(scaler_advance, 7.0);
    let hdmx_advance = font
      .hdmx()
      .unwrap()
      .record_for_size(font_size)
      .unwrap()
      .widths()[({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(
        ({
          let Ok(value) = gid.to_u32();
          value
        }),
      );
      migrated_value
    })];
    assert_eq!(hdmx_advance, 5);
  }

  #[test]
  fn round_advance_when_prep_disables_hinting() {
    let font = FontRef::new(font_test_data::TINOS_SUBSET).unwrap();
    let outlines = font.outline_glyphs();
    let gid = font.charmap().map('"').unwrap();
    let size = Size::new(16.0);
    let location = LocationRef::default();
    let mut hinter =
      HintingInstance::new(&outlines, size, location, HintingOptions::default()).unwrap();
    let HinterKind::Glyf(tt_hinter) = &mut hinter.kind else {
      panic!("this is definitely a TrueType hinter");
    };
    tt_hinter.simulate_prep_flag_suppress_hinting();
    let outline = outlines.get(gid).unwrap();

    let metrics = outline.draw(&hinter, &mut NullPen).unwrap();
    assert_eq!(metrics.advance_width, Some(7.0));

    let metrics = outline
      .draw(DrawSettings::unhinted(size, location), &mut NullPen)
      .unwrap();
    assert_eq!(metrics.advance_width, Some(6.53125));
  }

  #[test]
  fn hint_fractional_font_size() {
    let font = FontRef::new(font_test_data::COUSINE_HINT_SUBSET).unwrap();
    let outlines = font.outline_glyphs();
    let gid = ({
      let Ok(value) = GlyphId::new(1);
      value
    });
    let size = Size::new(24.8);
    let location = LocationRef::default();
    let hinter =
      HintingInstance::new(&outlines, size, location, HintingOptions::default()).unwrap();
    let outline = outlines.get(gid).unwrap();
    let mut pen = SvgPen::new();
    outline.draw(&hinter, &mut pen).unwrap();
    assert_eq!(
      pen.to_string(),
      "M12.65625,11.015625 Q11.296875,11.421875 10.078125,11.421875 Q8.140625,11.421875 6.9375,9.9375 Q5.734375,8.46875 5.734375,6.1875 L5.734375,0 L3.5625,0 L3.5625,8.421875 Q3.5625,9.328125 3.390625,10.5625 Q3.234375,11.8125 2.9375,13 L5,13 Q5.484375,11.34375 5.578125,10 L5.640625,10 Q6.25,11.25 6.828125,11.828125 Q7.40625,12.40625 8.203125,12.703125 Q9.015625,13 10.15625,13 Q11.421875,13 12.65625,13 Z"
    );
  }
}
