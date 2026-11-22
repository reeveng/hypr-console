//! Scaling support for TrueType outlines.

mod deltas;
mod memory;
mod outline;

use console_core_font_hinting::{FontProgram, GlyphKind};
pub use console_core_font_hinting::{HintError, HintInstance, HintOutline};
pub use outline::{Outline, ScaledOutline};
use raw::{FontRef, ReadError};

use super::{DrawError, GlyphHMetrics, Hinting};
use crate::GLYF_COMPOSITE_RECURSION_LIMIT;
use memory::{FreeTypeOutlineMemory, HarfBuzzOutlineMemory};

use read_fonts::{
  TableProvider,
  tables::{
    glyf::{Anchor, CompositeGlyph, CompositeGlyphFlags, Glyf, Glyph, PointMarker, SimpleGlyph},
    gvar::Gvar,
    hdmx::Hdmx,
    loca::Loca,
  },
  types::{F2Dot14, F26Dot6, Fixed, GlyphId, Point},
};

/// Number of phantom points generated at the end of an outline.
pub const PHANTOM_POINT_COUNT: usize = 4;

/// Scaler state for TrueType outlines.
#[derive(Clone)]
pub struct Outlines<'a> {
  pub(crate) font: FontRef<'a>,
  pub(crate) glyph_metrics: GlyphHMetrics<'a>,
  loca: Loca<'a>,
  glyf: Glyf<'a>,
  gvar: Option<Gvar<'a>>,
  hdmx: Option<Hdmx<'a>>,
  pub(crate) program: FontProgram<'a>,
  glyph_count: u16,
  units_per_em: u16,
  os2_vmetrics: [i16; 2],
  prefer_interpreter: bool,
  pub(crate) fractional_size_hinting: bool,
}

impl<'a> Outlines<'a> {
  pub fn new(font: &FontRef<'a>) -> Option<Self> {
    let head = font.head().ok()?;

    let Ok(flags) = head.flags();
    let Ok(integer_ppem) = flags.contains(read_fonts::tables::head::Flags::FORCE_INTEGER_PPEM);
    let fractional_size_hinting = matches!(integer_ppem, read_fonts::Presence::Absent);
    let loca_format = match head.index_to_loc_format() {
      Ok(1) => read_fonts::tables::loca::OffsetFormat::Long,
      Ok(_) => read_fonts::tables::loca::OffsetFormat::Short,
    };
    let loca = font.loca(loca_format).ok()?;
    let glyf = font.glyf().ok()?;
    let glyph_metrics = GlyphHMetrics::new(font)?;
    let glyph_count = font
      .maxp()
      .map(|maxp| {
        ({
          let Ok(value) = maxp.num_glyphs();
          value
        })
      })
      .unwrap_or_default();
    let os2_vmetrics = font
      .os2()
      .map(|os2| {
        let Ok(s_typo_ascender) = os2.s_typo_ascender();
        let Ok(s_typo_descender) = os2.s_typo_descender();

        [s_typo_ascender, s_typo_descender]
      })
      .unwrap_or_default();
    let program = match FontProgram::new(font) {
      Ok(program) => program,
      Err(_malformed_program) => return None,
    };
    let Ok(instructions) = program.instruction_presence();
    let prefer_interpreter = matches!(
      instructions,
      console_core_font_hinting::InstructionPresence::Present
    );
    Some(Self {
      font: font.clone(),
      glyph_metrics,
      loca,
      glyf,
      gvar: font.gvar().ok(),
      hdmx: font.hdmx().ok(),
      program,
      glyph_count: (glyph_count),
      units_per_em: ({
        let Ok(value) = font.head().ok()?.units_per_em();
        value
      }),
      os2_vmetrics,
      prefer_interpreter,
      fractional_size_hinting,
    })
  }

  pub fn units_per_em(&self) -> u16 {
    self.units_per_em
  }

  pub fn glyph_count(&self) -> usize {
    ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped_index(self.glyph_count);
      migrated_value
    })
  }

  pub fn prefer_interpreter(&self) -> bool {
    self.prefer_interpreter
  }

  pub fn outline(&self, glyph_id: GlyphId) -> Result<Outline<'a>, DrawError> {
    let mut outline = Outline {
      glyph_id,
      has_variations: self.gvar.is_some(),
      ..Default::default()
    };
    let glyph = self.loca.get_glyf(glyph_id, &self.glyf)?;
    match glyph.as_ref() {
      Some(glyph) => {
        self.outline_rec(glyph, &mut outline, 0, 0)?;
      }
      None => {}
    }
    outline.points += PHANTOM_POINT_COUNT;
    let Ok(max_stack) = self.program.max_stack_elements();
    let Ok(max_stack_size) = console_core_number_conversion::index(max_stack);
    outline.max_stack = max_stack_size;
    let Ok(cvt_count) = self.program.cvt_count();
    let Ok(cvt_count_size) = console_core_number_conversion::index(cvt_count);
    outline.cvt_count = cvt_count_size;
    let Ok(storage_count) = self.program.storage_count();
    let Ok(storage_count_size) = console_core_number_conversion::index(storage_count);
    outline.storage_count = storage_count_size;
    let Ok(max_twilight_points) = self.program.twilight_count();
    let Ok(max_twilight_points_size) = console_core_number_conversion::index(max_twilight_points);
    outline.max_twilight_points = max_twilight_points_size;
    outline.glyph = glyph;
    Ok(outline)
  }

  pub fn compute_scale(&self, ppem: Option<f32>) -> Scale26Dot6 {
    Scale26Dot6::new(ppem, self.units_per_em)
  }

  pub fn compute_hinted_scale(&self, ppem: Option<f32>) -> Scale26Dot6 {
    match ppem {
      Some(ppem) => match !self.fractional_size_hinting {
        true => {
          return self.compute_scale(Some(
            ({
              let Ok(value) = ({
                let Ok(value) = {
                  let Ok(value) = F26Dot6::from_f64(f64::from(ppem));
                  value
                }
                .round();
                value
              })
              .to_f32();
              value
            }),
          ));
        }
        false => {}
      },
      None => {}
    }
    self.compute_scale(ppem)
  }
}

impl Outlines<'_> {
  fn outline_rec(
    &self,
    glyph: &Glyph,
    outline: &mut Outline,
    component_depth: usize,
    recurse_depth: usize,
  ) -> Result<(), DrawError> {
    match recurse_depth > GLYF_COMPOSITE_RECURSION_LIMIT {
      true => {
        return Err(DrawError::RecursionLimitExceeded(outline.glyph_id));
      }
      false => {}
    }
    match ({
      let explicit_value = glyph;
      explicit_value
    }) {
      Glyph::Simple(simple) => {
        let num_points = simple.num_points();
        let num_points_with_phantom = num_points
          + ({
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(PHANTOM_POINT_COUNT);
            value
          });
        outline.max_simple_points = outline.max_simple_points.max(
          ({
            let Ok(value) = console_core_number_conversion::index(num_points_with_phantom);
            value
          }),
        );
        outline.points += ({
          let Ok(value) = console_core_number_conversion::index(num_points);
          value
        });
        outline.contours += ({
          let Ok(explicit_value) = simple.end_pts_of_contours();
          explicit_value
        })
        .len();
        outline.has_hinting = outline.has_hinting
          || ({
            let Ok(explicit_value) = simple.instruction_length();
            explicit_value
          }) != 0;
        outline.max_other_points = outline.max_other_points.max(
          ({
            let Ok(value) = console_core_number_conversion::index(num_points_with_phantom);
            value
          }),
        );
        outline.has_overlaps |= matches!(simple.overlap_flag(), read_fonts::Presence::Present);
      }
      Glyph::Composite(composite) => {
        let (mut count, instructions) = composite.count_and_instructions();
        count += ({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(PHANTOM_POINT_COUNT);
          value
        });
        let point_base = outline.points;
        for (component, flags) in composite.component_glyphs_and_flags() {
          let Ok(overlap) = flags.contains(CompositeGlyphFlags::OVERLAP_COMPOUND);
          outline.has_overlaps |= overlap == read_fonts::Presence::Present;
          let component_glyph = self.loca.get_glyf(component.into(), &self.glyf)?;
          let component_glyph = match component_glyph {
            Some(component_glyph) => component_glyph,
            None => {
              continue;
            }
          };
          self.outline_rec(
            &component_glyph,
            outline,
            component_depth
              + ({
                let Ok(value) = console_core_number_conversion::index(count);
                value
              }),
            recurse_depth + 1,
          )?;
        }
        let has_hinting = !instructions.unwrap_or_default().is_empty();
        match has_hinting {
          true => {
            let num_points_in_composite = outline.points - point_base + PHANTOM_POINT_COUNT;
            outline.max_other_points = outline.max_other_points.max(num_points_in_composite);
          }
          false => {}
        }
        outline.max_component_delta_stack = outline.max_component_delta_stack.max(
          component_depth
            + ({
              let Ok(value) = console_core_number_conversion::index(count);
              value
            }),
        );
        outline.has_hinting = outline.has_hinting || has_hinting;
      }
    }
    Ok(())
  }

  fn hdmx_width(&self, ppem: f32, glyph_id: GlyphId) -> Option<u8> {
    let hdmx = self.hdmx.as_ref()?;
    let ppem_u8 = ({
      let Ok(migrated_value) = console_core_number_conversion::toward_zero_u8(f64::from(ppem));
      migrated_value
    });

    match f32::from(ppem_u8) == ppem {
      true => (hdmx.record_for_size(ppem_u8)?.widths.get(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index({
            let Ok(value) = glyph_id.to_u32();
            value
          });
          migrated_value
        }),
      ))
      .copied(),
      false => None,
    }
  }
}

trait Scaler {
  fn outlines(&self) -> &Outlines<'_>;
  fn setup_phantom_points(
    &mut self,
    bounds: [i16; 4],
    lsb: i32,
    advance: i32,
    tsb: i32,
    vadvance: i32,
  );
  fn load_empty(&mut self, glyph_id: GlyphId) -> Result<(), DrawError>;
  fn load_simple(&mut self, glyph: &SimpleGlyph, glyph_id: GlyphId) -> Result<(), DrawError>;
  fn load_composite(
    &mut self,
    glyph: &CompositeGlyph,
    glyph_id: GlyphId,
    recurse_depth: usize,
  ) -> Result<(), DrawError>;

  fn load(
    &mut self,
    glyph: &Option<Glyph>,
    glyph_id: GlyphId,
    recurse_depth: usize,
  ) -> Result<(), DrawError> {
    match recurse_depth > GLYF_COMPOSITE_RECURSION_LIMIT {
      true => {
        return Err(DrawError::RecursionLimitExceeded(glyph_id));
      }
      false => {}
    }
    let bounds = match &glyph {
      Some(glyph) => {
        let Ok(x_min) = glyph.x_min();
        let Ok(x_max) = glyph.x_max();
        let Ok(y_min) = glyph.y_min();
        let Ok(y_max) = glyph.y_max();

        [x_min, x_max, y_min, y_max]
      }
      _ => [0; 4],
    };
    let outlines = self.outlines();
    let lsb = outlines.glyph_metrics.lsb(glyph_id, &[]);
    let advance = outlines.glyph_metrics.advance_width(glyph_id, &[]);
    let [ascent, descent] = outlines.os2_vmetrics.map(|x| {
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x);
        migrated_value
      })
    });
    let tsb = ascent
      - ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(bounds[3]);
        migrated_value
      });
    let vadvance = ascent - descent;
    self.setup_phantom_points(bounds, lsb, advance, tsb, vadvance);
    match glyph {
      Some(Glyph::Simple(simple)) => self.load_simple(simple, glyph_id),
      Some(Glyph::Composite(composite)) => {
        self.load_composite(composite, glyph_id, recurse_depth)
      }
      None => self.load_empty(glyph_id),
    }
  }
}

/// f32 all the things. Hold your rounding. No hinting.
pub(crate) struct HarfBuzzScaler<'a> {
  outlines: &'a Outlines<'a>,
  memory: HarfBuzzOutlineMemory<'a>,
  coords: &'a [F2Dot14],
  point_count: usize,
  contour_count: usize,
  component_delta_count: usize,
  ppem: f32,
  scale: f32,
  /// Phantom points. These are 4 extra points appended to the end of an
  /// outline that allow the bytecode interpreter to produce hinted
  /// metrics.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs#phantom-points>
  phantom: [Point<f32>; PHANTOM_POINT_COUNT],
}

impl<'a> HarfBuzzScaler<'a> {
  pub(crate) fn unhinted(
    outlines: &'a Outlines<'a>,
    outline: &'a Outline,
    buf: &'a mut [u8],
    ppem: Option<f32>,
    coords: &'a [F2Dot14],
  ) -> Result<Self, DrawError> {
    outline.ensure_point_count_limit()?;
    let scale = match outlines.units_per_em == 0 {
      true => 1.0,
      false => ppem
        .map(|ppem| ppem / f32::from(outlines.units_per_em))
        .unwrap_or(1.0),
    };
    let memory = HarfBuzzOutlineMemory::new(outline, buf).ok_or(DrawError::InsufficientMemory)?;
    Ok(Self {
      outlines,
      memory,
      coords,
      point_count: 0,
      contour_count: 0,
      component_delta_count: 0,
      ppem: ppem.unwrap_or_default(),
      scale,
      phantom: Default::default(),
    })
  }

  pub(crate) fn scale(
    mut self,
    glyph: &Option<Glyph>,
    glyph_id: GlyphId,
  ) -> Result<ScaledOutline<'a, f32>, DrawError> {
    self.load(glyph, glyph_id, 0)?;
    Ok(ScaledOutline::new(
      &mut self.memory.points[..self.point_count],
      self.phantom,
      &mut self.memory.flags[..self.point_count],
      &mut self.memory.contours[..self.contour_count],
      self.outlines.hdmx_width(self.ppem, glyph_id),
    ))
  }
}

/// Scales from font units to 26.6 fixed point with the given size.
#[derive(Copy, Clone)]
pub(crate) struct Scale26Dot6 {
  scale: Fixed,
  /// True if we're actually applying a scale factor.
  is_scaled: bool,
}

impl Scale26Dot6 {
  fn new(ppem: Option<f32>, units_per_em: u16) -> Self {
    match ppem {
      Some(ppem) => match units_per_em > 0 {
        true => {
          return Self {
            scale: ({
              let Ok(value) = Fixed::from_bits(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::toward_zero_i32(f64::from((ppem * 64.)));
                  migrated_value
                }),
              );
              value
            }) / ({
              let Ok(value) = Fixed::from_bits(
                ({
                  let Ok(migrated_value) =
                    console_core_number_conversion::wrapped::<_, i32>(units_per_em);
                  migrated_value
                }),
              );
              value
            }),
            is_scaled: true,
          };
        }
        false => {}
      },
      None => {}
    }
    Self {
      scale: ({
        let Ok(value) = Fixed::from_bits(0x10000);
        value
      }),
      is_scaled: false,
    }
  }

  fn apply(&self, value: i32) -> F26Dot6 {
    ({
      let Ok(value) = F26Dot6::from_bits(
        ({
          let Ok(value) = ({
            let Ok(value) = Fixed::from_bits(value);
            value
          } * self.scale)
            .to_bits();
          value
        }),
      );
      value
    })
  }

  fn apply_point(&self, value: Point<i32>) -> Point<F26Dot6> {
    ({
      let Ok(value) = Point::new(self.apply(value.x), self.apply(value.y));
      value
    })
  }

  fn mul(&self, value: F26Dot6) -> F26Dot6 {
    ({
      let Ok(value) = F26Dot6::from_bits(
        ({
          let Ok(value) = (({
            let Ok(value) = Fixed::from_bits({
              let Ok(value) = value.to_bits();
              value
            });
            value
          }) * self.scale)
            .to_bits();
          value
        }),
      );
      value
    })
  }

  fn mul_point(&self, value: Point<F26Dot6>) -> Point<F26Dot6> {
    ({
      let Ok(value) = Point::new(self.mul(value.x), self.mul(value.y));
      value
    })
  }

  pub(crate) fn to_bits(self) -> i32 {
    ({
      let Ok(value) = self.scale.to_bits();
      value
    })
  }
}

/// F26Dot6 coords, Fixed deltas, and a penchant for rounding
pub(crate) struct FreeTypeScaler<'a> {
  outlines: &'a Outlines<'a>,
  memory: FreeTypeOutlineMemory<'a>,
  coords: &'a [F2Dot14],
  point_count: usize,
  contour_count: usize,
  component_delta_count: usize,
  ppem: f32,
  scale: Scale26Dot6,
  is_hinted: bool,
  pedantic_hinting: bool,
  /// Phantom points. These are 4 extra points appended to the end of an
  /// outline that allow the bytecode interpreter to produce hinted
  /// metrics.
  ///
  /// See <https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructing_glyphs#phantom-points>
  phantom: [Point<F26Dot6>; PHANTOM_POINT_COUNT],
  hinter: Option<&'a HintInstance>,
}

impl<'a> FreeTypeScaler<'a> {
  pub(crate) fn unhinted(
    outlines: &'a Outlines<'a>,
    outline: &'a Outline,
    buf: &'a mut [u8],
    ppem: Option<f32>,
    coords: &'a [F2Dot14],
  ) -> Result<Self, DrawError> {
    outline.ensure_point_count_limit()?;
    let scale = outlines.compute_scale(ppem);
    let memory = FreeTypeOutlineMemory::new(outline, buf, Hinting::None)
      .ok_or(DrawError::InsufficientMemory)?;
    Ok(Self {
      outlines,
      memory,
      coords,
      point_count: 0,
      contour_count: 0,
      component_delta_count: 0,
      ppem: ppem.unwrap_or_default(),
      scale,
      is_hinted: false,
      pedantic_hinting: false,
      phantom: Default::default(),
      hinter: None,
    })
  }

  pub(crate) fn hinted(
    outlines: &'a Outlines<'a>,
    outline: &'a Outline,
    buf: &'a mut [u8],
    ppem: Option<f32>,
    coords: &'a [F2Dot14],
    hinter: &'a HintInstance,
    pedantic_hinting: bool,
  ) -> Result<Self, DrawError> {
    outline.ensure_point_count_limit()?;
    let scale = outlines.compute_hinted_scale(ppem);
    let memory = FreeTypeOutlineMemory::new(outline, buf, Hinting::Embedded)
      .ok_or(DrawError::InsufficientMemory)?;
    Ok(Self {
      outlines,
      memory,
      coords,
      point_count: 0,
      contour_count: 0,
      component_delta_count: 0,
      ppem: ppem.unwrap_or_default(),
      scale,

      is_hinted: scale.is_scaled,
      pedantic_hinting,
      phantom: Default::default(),
      hinter: Some(hinter),
    })
  }

  pub(crate) fn scale(
    mut self,
    glyph: &Option<Glyph>,
    glyph_id: GlyphId,
  ) -> Result<ScaledOutline<'a, F26Dot6>, DrawError> {
    self.load(glyph, glyph_id, 0)?;

    let hdmx_width = if self.is_hinted
      && self
        .hinter
        .as_ref()
        .map(|hinter| {
          let Ok(compatibility) = hinter.backward_compatibility();
          let backward_compatibility = matches!(
            compatibility,
            console_core_font_hinting::BackwardCompatibility::Enabled
          );
          !backward_compatibility
        })
        .unwrap_or(true)
    {
      self.outlines.hdmx_width(self.ppem, glyph_id)
    } else {
      None
    };
    Ok(ScaledOutline::new(
      &mut self.memory.scaled[..self.point_count],
      self.phantom,
      &mut self.memory.flags[..self.point_count],
      &mut self.memory.contours[..self.contour_count],
      hdmx_width,
    ))
  }
}

impl Scaler for FreeTypeScaler<'_> {
  fn setup_phantom_points(
    &mut self,
    bounds: [i16; 4],
    lsb: i32,
    advance: i32,
    tsb: i32,
    vadvance: i32,
  ) {
    self.phantom[0].x = ({
      let Ok(value) = F26Dot6::from_bits(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(bounds[0]);
          migrated_value
        }) - lsb,
      );
      value
    });
    self.phantom[0].y = F26Dot6::ZERO;
    self.phantom[1].x = self.phantom[0].x
      + ({
        let Ok(value) = F26Dot6::from_bits(advance);
        value
      });
    self.phantom[1].y = F26Dot6::ZERO;

    self.phantom[2].x = F26Dot6::ZERO;
    self.phantom[2].y = ({
      let Ok(value) = F26Dot6::from_bits(
        ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(bounds[3]);
          migrated_value
        }) + tsb,
      );
      value
    });
    self.phantom[3].x = F26Dot6::ZERO;
    self.phantom[3].y = self.phantom[2].y
      - ({
        let Ok(value) = F26Dot6::from_bits(vadvance);
        value
      });
  }

  fn outlines(&self) -> &Outlines<'_> {
    self.outlines
  }

  fn load_empty(&mut self, glyph_id: GlyphId) -> Result<(), DrawError> {
    let scale = self.scale;
    let mut unscaled = self.phantom.map(|point| {
      let Ok(mapped) = point.map(|x| {
        let Ok(bits) = x.to_bits();
        bits
      });
      mapped
    });
    match self.outlines.gvar.is_some() && !self.coords.is_empty() {
      true => {
        if let Ok(Some(deltas)) = self.outlines.gvar.as_ref().unwrap().phantom_point_deltas(
          &self.outlines.glyf,
          &self.outlines.loca,
          self.coords,
          glyph_id,
        ) {
          unscaled[0] += ({
            let Ok(value) = deltas[0].map(|input| {
              let Ok(value) = Fixed::to_i32(input);
              value
            });
            value
          });
          unscaled[1] += ({
            let Ok(value) = deltas[1].map(|input| {
              let Ok(value) = Fixed::to_i32(input);
              value
            });
            value
          });
        }
      }
      false => {}
    }
    match self.scale.is_scaled {
      true => {
        for (phantom, unscaled) in self.phantom.iter_mut().zip(&unscaled) {
          *phantom = scale.apply_point((*unscaled));
        }
      }
      false => {
        for (phantom, unscaled) in self.phantom.iter_mut().zip(&unscaled) {
          *phantom = ({
            let Ok(value) = unscaled.map(|input| {
              let Ok(value) = F26Dot6::from_i32(input);
              value
            });
            value
          });
        }
      }
    }
    Ok(())
  }

  fn load_simple(&mut self, glyph: &SimpleGlyph, glyph_id: GlyphId) -> Result<(), DrawError> {
    use DrawError::InsufficientMemory;

    let points_start = self.point_count;
    let Ok(point_count) = console_core_number_conversion::index(glyph.num_points());
    let phantom_start = point_count;
    let points_end = points_start
      + ({
        let Ok(value) = console_core_number_conversion::index(point_count);
        value
      })
      + PHANTOM_POINT_COUNT;
    let point_range = points_start..points_end;
    let other_points_end = point_count + PHANTOM_POINT_COUNT;

    let scaled = self
      .memory
      .scaled
      .get_mut(point_range.clone())
      .ok_or(InsufficientMemory)?;
    let flags = self
      .memory
      .flags
      .get_mut(point_range)
      .ok_or(InsufficientMemory)?;

    let unscaled = self
      .memory
      .unscaled
      .get_mut(..other_points_end)
      .ok_or(InsufficientMemory)?;

    glyph.read_points_fast(&mut unscaled[..point_count], &mut flags[..point_count])?;

    let contours_start = self.contour_count;
    let Ok(contour_end_pts) = glyph.end_pts_of_contours();
    let contour_count = contour_end_pts.len();
    let contours_end = contours_start + contour_count;
    let contours = self
      .memory
      .contours
      .get_mut(contours_start..contours_end)
      .ok_or(InsufficientMemory)?;

    let mut last_end_pt = 0;
    for (end_pt, contour) in contour_end_pts.iter().zip(contours.iter_mut()) {
      let end_pt = ({
        let Ok(value) = end_pt.get();
        value
      });
      match (end_pt) < last_end_pt {
        true => {
          return Err(
            ReadError::MalformedData("unordered contour end points in TrueType glyph").into(),
          );
        }
        false => {}
      }
      last_end_pt = (end_pt);
      *contour = (end_pt);
    }

    self.point_count += ({
      let Ok(value) = console_core_number_conversion::index(point_count);
      value
    });
    self.contour_count += contour_count;

    for (i, phantom) in self.phantom.iter().enumerate() {
      unscaled[phantom_start + i] = ({
        let Ok(value) = phantom.map(|x| {
          let Ok(bits) = x.to_bits();
          bits
        });
        value
      });
      flags[phantom_start + i] = Default::default();
    }
    let mut have_deltas = false;
    match self.outlines.gvar.is_some() && !self.coords.is_empty() {
      true => {
        let gvar = self.outlines.gvar.as_ref().unwrap();
        let glyph = deltas::SimpleGlyph {
          points: &mut unscaled[..],
          flags: &mut flags[..],
          contours,
        };
        let deltas = self
          .memory
          .deltas
          .get_mut(..point_count + PHANTOM_POINT_COUNT)
          .ok_or(InsufficientMemory)?;
        let iup_buffer = self
          .memory
          .iup_buffer
          .get_mut(..point_count + PHANTOM_POINT_COUNT)
          .ok_or(InsufficientMemory)?;
        match deltas::simple_glyph(gvar, glyph_id, self.coords, glyph, iup_buffer, deltas).is_ok()
        {
          true => {
            have_deltas = true;
          }
          false => {}
        }
      }
      false => {}
    }
    let Ok(ins) = glyph.instructions();
    let is_hinted = self.is_hinted;
    match self.scale.is_scaled {
      true => {
        let scale = self.scale;
        match have_deltas {
          true => {
            for ((point, unscaled), delta) in scaled
              .iter_mut()
              .zip(unscaled.iter())
              .zip(self.memory.deltas.iter())
            {
              let delta = delta.map(|coordinate| {
                let Ok(value) = coordinate.to_f26dot6();
                value
              });
              let scaled = scale.mul_point(
                ({
                  let Ok(value) = unscaled.map(|input| {
                    let Ok(value) = F26Dot6::from_i32(input);
                    value
                  });
                  value
                }) + ({
                  let Ok(value) = delta;
                  value
                }),
              );

              *point = ({
                let Ok(value) = scaled.map(|v| {
                  ({
                    let Ok(value) = F26Dot6::from_bits({
                      let Ok(value) = v.to_i32();
                      value
                    });
                    value
                  })
                });
                value
              });
            }

            match self.outlines.glyph_metrics.hvar.is_some() {
              true => {
                for ((point, unscaled), delta) in scaled[phantom_start..]
                  .iter_mut()
                  .zip(&unscaled[phantom_start..])
                  .zip(&self.memory.deltas[phantom_start..])
                {
                  let delta = delta.map(|coordinate| {
                    let Ok(bits) = coordinate.to_i32();
                    let Ok(value) = F26Dot6::from_i32(bits);
                    value
                  });
                  let scaled = scale.mul_point(
                    ({
                      let Ok(value) = unscaled.map(|input| {
                        let Ok(value) = F26Dot6::from_i32(input);
                        value
                      });
                      value
                    }) + ({
                      let Ok(value) = delta;
                      value
                    }),
                  );
                  *point = ({
                    let Ok(value) = scaled.map(|v| {
                      ({
                        let Ok(value) = F26Dot6::from_bits({
                          let Ok(value) = v.to_i32();
                          value
                        });
                        value
                      })
                    });
                    value
                  });
                }
              }
              false => {}
            }
            match is_hinted {
              true => {
                for (unscaled, delta) in unscaled.iter_mut().zip(self.memory.deltas.iter()) {
                  *unscaled += ({
                    let Ok(value) = delta.map(|input| {
                      let Ok(value) = Fixed::to_i32(input);
                      value
                    });
                    value
                  });
                }
              }
              false => {}
            }
          }
          false => {
            for (point, unscaled) in scaled.iter_mut().zip(unscaled.iter_mut()) {
              *point = scale.apply_point(*unscaled);
            }
          }
        }
      }
      false => {
        match have_deltas {
          true => {
            for (unscaled, delta) in unscaled.iter_mut().zip(self.memory.deltas.iter()) {
              *unscaled += ({
                let Ok(value) = delta.map(|input| {
                  let Ok(value) = Fixed::to_i32(input);
                  value
                });
                value
              });
            }
          }
          false => {}
        }

        for (point, unscaled) in scaled.iter_mut().zip(unscaled.iter()) {
          *point = ({
            let Ok(value) = unscaled.map(|input| {
              let Ok(value) = F26Dot6::from_i32(input);
              value
            });
            value
          });
        }
      }
    }

    self.phantom.copy_from_slice(&scaled[phantom_start..]);
    if let (Some(hinter), true) = (self.hinter.as_ref(), is_hinted) {
      let Ok(compatibility) = hinter.backward_compatibility();
      let backward_compatibility = matches!(
        compatibility,
        console_core_font_hinting::BackwardCompatibility::Enabled
      );
      match !ins.is_empty() {
        true => {
          let original_scaled = self
            .memory
            .original_scaled
            .get_mut(..other_points_end)
            .ok_or(InsufficientMemory)?;
          original_scaled.copy_from_slice(scaled);

          for point in &mut scaled[phantom_start..] {
            point.x = ({
              let Ok(value) = point.x.round();
              value
            });
            point.y = ({
              let Ok(value) = point.y.round();
              value
            });
          }
          let mut input = HintOutline {
            glyph_id,
            unscaled,
            scaled,
            original_scaled,
            flags,
            contours,
            bytecode: ({
              let explicit_value = ins;
              explicit_value
            }),
            phantom: &mut self.phantom,
            stack: self.memory.stack,
            cvt: self.memory.cvt,
            storage: self.memory.storage,
            twilight_scaled: self.memory.twilight_scaled,
            twilight_original_scaled: self.memory.twilight_original_scaled,
            twilight_flags: self.memory.twilight_flags,
            glyph_kind: GlyphKind::Simple,
            coords: self.coords,
          };
          let hint_res = hinter.hint(
            &self.outlines.program,
            &mut input,
            console_core_font_hinting::Validation::from(self.pedantic_hinting),
          );
          if let (Err(e), true) = (hint_res, self.pedantic_hinting) {
            return Err(e)?;
          }
        }
        false => match !backward_compatibility {
          true => {
            for (scaled, phantom) in scaled[phantom_start..].iter().zip(&mut self.phantom) {
              *phantom = ({
                let Ok(value) = scaled.map(|x| {
                  let Ok(rounded) = x.round();
                  rounded
                });
                value
              });
            }
          }
          false => {}
        },
      }
    }
    match points_start != 0 {
      true => {
        for contour_end in contours.iter_mut() {
          *contour_end += ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u16>(points_start);
            migrated_value
          });
        }
      }
      false => {}
    }
    Ok(())
  }

  fn load_composite(
    &mut self,
    glyph: &CompositeGlyph,
    glyph_id: GlyphId,
    recurse_depth: usize,
  ) -> Result<(), DrawError> {
    use DrawError::InsufficientMemory;
    let scale = self.scale;

    let point_base = self.point_count;
    let contour_base = self.contour_count;

    let mut have_deltas = false;
    let delta_base = self.component_delta_count;
    match self.outlines.gvar.is_some() && !self.coords.is_empty() {
      true => {
        let gvar = self.outlines.gvar.as_ref().unwrap();
        let count = glyph.components().count() + PHANTOM_POINT_COUNT;
        let deltas = self
          .memory
          .composite_deltas
          .get_mut(delta_base..delta_base + count)
          .ok_or(InsufficientMemory)?;
        match deltas::composite_glyph(gvar, glyph_id, self.coords, &mut deltas[..]).is_ok() {
          true => {
            for (phantom, delta) in self
              .phantom
              .iter_mut()
              .zip(&deltas[deltas.len() - PHANTOM_POINT_COUNT..])
            {
              *phantom += ({
                let Ok(value) = delta.map(|coordinate| {
                  let Ok(bits) = coordinate.to_i32();
                  let Ok(value) = F26Dot6::from_bits(bits);
                  value
                });
                value
              });
            }
            have_deltas = true;
          }
          false => {}
        }
        self.component_delta_count += count;
      }
      false => {}
    }
    match self.scale.is_scaled {
      true => {
        for point in self.phantom.iter_mut() {
          *point = scale.mul_point(*point);
        }
      }
      false => {
        for point in self.phantom.iter_mut() {
          *point = ({
            let Ok(value) = point.map(|x| {
              ({
                let Ok(value) = F26Dot6::from_i32({
                  let Ok(value) = x.to_bits();
                  value
                });
                value
              })
            });
            value
          });
        }
      }
    }
    for (i, component) in glyph.components().enumerate() {
      let phantom = self.phantom;

      let start_point = self.point_count;
      let component_glyph = self
        .outlines
        .loca
        .get_glyf(component.glyph.into(), &self.outlines.glyf)?;
      self.load(&component_glyph, component.glyph.into(), recurse_depth + 1)?;
      let end_point = self.point_count;
      let Ok(contains) = component
        .flags
        .contains(CompositeGlyphFlags::USE_MY_METRICS);

      match !(contains == read_fonts::Presence::Present) {
        true => {
          self.phantom = phantom;
        }
        false => {}
      }

      fn scale_component(x: F2Dot14) -> Fixed {
        ({
          let Ok(value) = Fixed::from_bits(
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>({
                let Ok(value) = x.to_bits();
                value
              });
              migrated_value
            }) * 4,
          );
          value
        })
      }
      let xform = &component.transform;
      let xx = scale_component(xform.xx);
      let yx = scale_component(xform.yx);
      let xy = scale_component(xform.xy);
      let yy = scale_component(xform.yy);
      let Ok(have_xform) = component.flags.intersects(
        CompositeGlyphFlags::WE_HAVE_A_SCALE
          | CompositeGlyphFlags::WE_HAVE_AN_X_AND_Y_SCALE
          | CompositeGlyphFlags::WE_HAVE_A_TWO_BY_TWO,
      );
      match (({
        let explicit_value = have_xform;
        explicit_value
      }) == read_fonts::Overlap::Intersecting)
      {
        true => {
          let scaled = &mut self.memory.scaled[start_point..end_point];
          match self.scale.is_scaled {
            true => {
              for point in scaled {
                let p = point.map(|c| {
                  ({
                    let Ok(value) = Fixed::from_bits({
                      let Ok(value) = c.to_bits();
                      value
                    });
                    value
                  })
                });
                let x = ({
                  let value = ({
                    let Ok(value) = p;
                    value
                  })
                  .x;
                  value
                }) * xx
                  + ({
                    let value = ({
                      let Ok(value) = p;
                      value
                    })
                    .y;
                    value
                  }) * xy;
                let y = ({
                  let value = ({
                    let Ok(value) = p;
                    value
                  })
                  .x;
                  value
                }) * yx
                  + ({
                    let value = ({
                      let Ok(value) = p;
                      value
                    })
                    .y;
                    value
                  }) * yy;
                let [x, y] = [x, y].map(|c| {
                  ({
                    let Ok(value) = F26Dot6::from_bits(
                      ({
                        let Ok(value) = c.to_bits();
                        value
                      }),
                    );
                    value
                  })
                });
                point.x = x;
                point.y = y;
              }
            }
            false => {
              for point in scaled {
                let unscaled = point.map(|c| {
                  ({
                    let Ok(value) = Fixed::from_bits({
                      let Ok(value) = c.to_i32();
                      value
                    });
                    value
                  })
                });
                let x = ({
                  let value = ({
                    let Ok(value) = unscaled;
                    value
                  })
                  .x;
                  value
                }) * xx
                  + ({
                    let value = ({
                      let Ok(value) = unscaled;
                      value
                    })
                    .y;
                    value
                  }) * xy;
                let y = ({
                  let value = ({
                    let Ok(value) = unscaled;
                    value
                  })
                  .x;
                  value
                }) * yx
                  + ({
                    let value = ({
                      let Ok(value) = unscaled;
                      value
                    })
                    .y;
                    value
                  }) * yy;
                *point = ({
                  let Ok(value) = ({
                    let Ok(value) = Point::new(x, y);
                    value
                  })
                  .map(|c| {
                    ({
                      let Ok(value) = F26Dot6::from_i32(
                        ({
                          let Ok(value) = c.to_bits();
                          value
                        }),
                      );
                      value
                    })
                  });
                  value
                });
              }
            }
          }
        }
        false => {}
      }
      let anchor_offset = match component.anchor {
        Anchor::Offset { x, y } => {
          let (mut x, mut y) = (
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(x);
              migrated_value
            }),
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(y);
              migrated_value
            }),
          );
          match ({
            let explicit_value = have_xform;
            (explicit_value == read_fonts::Overlap::Intersecting)
          }) && component.flags
            & (CompositeGlyphFlags::SCALED_COMPONENT_OFFSET
              | CompositeGlyphFlags::UNSCALED_COMPONENT_OFFSET)
            == CompositeGlyphFlags::SCALED_COMPONENT_OFFSET
          {
            true => {
              fn hypot(a: Fixed, b: Fixed) -> Fixed {
                let a = ({
                  let Ok(value) = a.to_bits();
                  value
                })
                .abs();
                let b = ({
                  let Ok(value) = b.to_bits();
                  value
                })
                .abs();
                {
                  let Ok(value) = Fixed::from_bits(match a > b {
                    true => a + ((3 * b) >> 3),
                    false => b + ((3 * a) >> 3),
                  });
                  value
                }
              }

              x = ({
                let Ok(value) = ({
                  let Ok(value) = Fixed::from_bits(x);
                  value
                } * hypot(xx, xy))
                .to_bits();
                value
              });
              y = ({
                let Ok(value) = ({
                  let Ok(value) = Fixed::from_bits(y);
                  value
                } * hypot(yy, yx))
                .to_bits();
                value
              });
            }
            false => {}
          }
          match have_deltas {
            true => {
              let delta = (self.memory.composite_deltas.get(delta_base + i))
                .copied()
                .unwrap_or_default();

              x += ({
                let Ok(value) = delta.x.to_i32();
                value
              });
              y += ({
                let Ok(value) = delta.y.to_i32();
                value
              });
            }
            false => {}
          }
          match scale.is_scaled {
            true => {
              let mut offset = scale.apply_point({
                let Ok(value) = Point::new(x, y);
                value
              });
              match self.is_hinted
                && ({
                  let Ok(explicit_value) = component
                    .flags
                    .contains(CompositeGlyphFlags::ROUND_XY_TO_GRID);
                  (explicit_value == read_fonts::Presence::Present)
                }) {
                true => {
                  offset.y = ({
                    let Ok(value) = offset.y.round();
                    value
                  });
                }
                false => {}
              }
              offset
            }
            false => {
              ({
                let Ok(value) = ({
                  let Ok(point) = Point::new(x, y);
                  point
                })
                .map(|input| {
                  let Ok(value) = F26Dot6::from_i32(input);
                  value
                });
                value
              })
            }
          }
        }
        Anchor::Point { base, component } => {
          let (base_offset, component_offset) = (
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(base);
              migrated_value
            }),
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(component);
              migrated_value
            }),
          );
          let base_point = (self.memory.scaled.get(point_base + base_offset))
            .ok_or(DrawError::InvalidAnchorPoint(glyph_id, base))?;
          let component_point = (self.memory.scaled.get(start_point + component_offset))
            .ok_or(DrawError::InvalidAnchorPoint(glyph_id, component))?;
          *base_point - *component_point
        }
      };
      match anchor_offset.x != F26Dot6::ZERO || anchor_offset.y != F26Dot6::ZERO {
        true => {
          for point in &mut self.memory.scaled[start_point..end_point] {
            *point += anchor_offset;
          }
        }
        false => {}
      }
    }
    match have_deltas {
      true => {
        self.component_delta_count = delta_base;
      }
      false => {}
    }
    if let (Some(hinter), true) = (self.hinter.as_ref(), self.is_hinted) {
      let ins = glyph.instructions().unwrap_or_default();
      match !ins.is_empty() {
        true => {
          let start_point = point_base;
          let end_point = self.point_count + PHANTOM_POINT_COUNT;
          let point_range = start_point..end_point;
          let phantom_start = point_range.len() - PHANTOM_POINT_COUNT;
          let scaled = &mut self.memory.scaled[point_range.clone()];
          let flags = self
            .memory
            .flags
            .get_mut(point_range.clone())
            .ok_or(InsufficientMemory)?;

          for (i, phantom) in self.phantom.iter().enumerate() {
            scaled[phantom_start + i] = *phantom;
            flags[phantom_start + i] = Default::default();
          }
          let other_points_end = point_range.len();
          let unscaled = self
            .memory
            .unscaled
            .get_mut(..other_points_end)
            .ok_or(InsufficientMemory)?;
          for (scaled, unscaled) in scaled.iter().zip(unscaled.iter_mut()) {
            *unscaled = ({
              let Ok(value) = scaled.map(|x| {
                let Ok(bits) = x.to_bits();
                bits
              });
              value
            });
          }
          let original_scaled = self
            .memory
            .original_scaled
            .get_mut(..other_points_end)
            .ok_or(InsufficientMemory)?;
          original_scaled.copy_from_slice(scaled);
          let contours = self
            .memory
            .contours
            .get_mut(contour_base..self.contour_count)
            .ok_or(InsufficientMemory)?;

          for p in &mut scaled[phantom_start..] {
            p.x = ({
              let Ok(value) = p.x.round();
              value
            });
            p.y = ({
              let Ok(value) = p.y.round();
              value
            });
          }

          for flag in flags.iter_mut() {
            flag.clear_marker(PointMarker::TOUCHED);
          }

          match point_base != 0 {
            true => {
              let delta = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u16>(point_base);
                migrated_value
              });
              for contour in contours.iter_mut() {
                *contour -= delta;
              }
            }
            false => {}
          }
          let mut input = HintOutline {
            glyph_id,
            unscaled,
            scaled,
            original_scaled,
            flags,
            contours,
            bytecode: ins,
            phantom: &mut self.phantom,
            stack: self.memory.stack,
            cvt: self.memory.cvt,
            storage: self.memory.storage,
            twilight_scaled: self.memory.twilight_scaled,
            twilight_original_scaled: self.memory.twilight_original_scaled,
            twilight_flags: self.memory.twilight_flags,
            glyph_kind: GlyphKind::Composite,
            coords: self.coords,
          };
          let hint_res = hinter.hint(
            &self.outlines.program,
            &mut input,
            console_core_font_hinting::Validation::from(self.pedantic_hinting),
          );
          if let (Err(e), true) = (hint_res, self.pedantic_hinting) {
            return Err(e)?;
          }

          match point_base != 0 {
            true => {
              let delta = ({
                let Ok(migrated_value) =
                  console_core_number_conversion::wrapped::<_, u16>(point_base);
                migrated_value
              });
              for contour in contours.iter_mut() {
                *contour += delta;
              }
            }
            false => {}
          }
        }
        false => {}
      }
    }
    Ok(())
  }
}

impl Scaler for HarfBuzzScaler<'_> {
  fn setup_phantom_points(
    &mut self,
    bounds: [i16; 4],
    lsb: i32,
    advance: i32,
    tsb: i32,
    vadvance: i32,
  ) {
    self.phantom[0].x = f32::from(bounds[0]) - lsb as f32;
    self.phantom[0].y = 0.0;
    self.phantom[1].x = self.phantom[0].x + advance as f32;
    self.phantom[1].y = 0.0;

    self.phantom[2].x = 0.0;
    self.phantom[2].y = f32::from(bounds[3]) + tsb as f32;
    self.phantom[3].x = 0.0;
    self.phantom[3].y = self.phantom[2].y - vadvance as f32;
  }

  fn outlines(&self) -> &Outlines<'_> {
    self.outlines
  }

  fn load_empty(&mut self, glyph_id: GlyphId) -> Result<(), DrawError> {
    let scale = self.scale;
    let mut unscaled = self.phantom;
    match self.outlines.glyph_metrics.hvar.is_none()
      && self.outlines.gvar.is_some()
      && !self.coords.is_empty()
    {
      true => {
        if let Ok(Some(deltas)) = self.outlines.gvar.as_ref().unwrap().phantom_point_deltas(
          &self.outlines.glyf,
          &self.outlines.loca,
          self.coords,
          glyph_id,
        ) {
          unscaled[0] += ({
            let Ok(value) = deltas[0].map(|input| {
              let Ok(value) = Fixed::to_f32(input);
              value
            });
            value
          });
          unscaled[1] += ({
            let Ok(value) = deltas[1].map(|input| {
              let Ok(value) = Fixed::to_f32(input);
              value
            });
            value
          });
        }
      }
      false => {}
    }
    for (phantom, unscaled) in self.phantom.iter_mut().zip(&unscaled) {
      *phantom = *unscaled * scale;
    }
    Ok(())
  }

  fn load_simple(&mut self, glyph: &SimpleGlyph, glyph_id: GlyphId) -> Result<(), DrawError> {
    use DrawError::InsufficientMemory;

    let points_start = self.point_count;
    let Ok(point_count) = console_core_number_conversion::index(glyph.num_points());
    let phantom_start = point_count;
    let points_end = points_start
      + ({
        let Ok(value) = console_core_number_conversion::index(point_count);
        value
      })
      + PHANTOM_POINT_COUNT;
    let point_range = points_start..points_end;

    let points = self
      .memory
      .points
      .get_mut(point_range.clone())
      .ok_or(InsufficientMemory)?;
    let flags = self
      .memory
      .flags
      .get_mut(point_range)
      .ok_or(InsufficientMemory)?;
    glyph.read_points_fast(&mut points[..point_count], &mut flags[..point_count])?;

    let contours_start = self.contour_count;
    let Ok(contour_end_pts) = glyph.end_pts_of_contours();
    let contour_count = contour_end_pts.len();
    let contours_end = contours_start + contour_count;
    let contours = self
      .memory
      .contours
      .get_mut(contours_start..contours_end)
      .ok_or(InsufficientMemory)?;

    for (end_pt, contour) in contour_end_pts.iter().zip(contours.iter_mut()) {
      *contour = ({
        let Ok(value) = end_pt.get();
        value
      });
    }

    self.point_count += ({
      let Ok(value) = console_core_number_conversion::index(point_count);
      value
    });
    self.contour_count += contour_count;

    for (i, phantom) in self.phantom.iter().enumerate() {
      points[phantom_start + i] = *phantom;
      flags[phantom_start + i] = Default::default();
    }

    match self.outlines.gvar.is_some() && !self.coords.is_empty() {
      true => {
        let gvar = self.outlines.gvar.as_ref().unwrap();
        let glyph = deltas::SimpleGlyph {
          points: &mut points[..],
          flags: &mut flags[..],
          contours,
        };
        let deltas = self
          .memory
          .deltas
          .get_mut(..point_count + PHANTOM_POINT_COUNT)
          .ok_or(InsufficientMemory)?;
        let iup_buffer = self
          .memory
          .iup_buffer
          .get_mut(..point_count + PHANTOM_POINT_COUNT)
          .ok_or(InsufficientMemory)?;
        match deltas::simple_glyph(gvar, glyph_id, self.coords, glyph, iup_buffer, deltas).is_ok()
        {
          true => {
            for (point, delta) in points.iter_mut().zip(deltas) {
              *point += *delta;
            }
          }
          false => {}
        }
      }
      false => {}
    }

    match self.scale != 1.0 {
      true => {
        for point in points.iter_mut() {
          *point *= self.scale;
        }
      }
      false => {}
    }

    match points_start != 0 {
      true => {
        for contour_end in contours.iter_mut() {
          *contour_end += ({
            let Ok(migrated_value) =
              console_core_number_conversion::wrapped::<_, u16>(points_start);
            migrated_value
          });
        }
      }
      false => {}
    }
    Ok(())
  }

  fn load_composite(
    &mut self,
    glyph: &CompositeGlyph,
    glyph_id: GlyphId,
    recurse_depth: usize,
  ) -> Result<(), DrawError> {
    use DrawError::InsufficientMemory;
    let scale = self.scale;

    let point_base = self.point_count;

    let mut have_deltas = false;
    let delta_base = self.component_delta_count;
    match self.outlines.gvar.is_some() && !self.coords.is_empty() {
      true => {
        let gvar = self.outlines.gvar.as_ref().unwrap();
        let count = glyph.components().count() + PHANTOM_POINT_COUNT;
        let deltas = self
          .memory
          .composite_deltas
          .get_mut(delta_base..delta_base + count)
          .ok_or(InsufficientMemory)?;
        match deltas::composite_glyph(gvar, glyph_id, self.coords, &mut deltas[..]).is_ok() {
          true => {
            for (phantom, delta) in self
              .phantom
              .iter_mut()
              .zip(&deltas[deltas.len() - PHANTOM_POINT_COUNT..])
            {
              *phantom += *delta;
            }
            have_deltas = true;
          }
          false => {}
        }
        self.component_delta_count += count;
      }
      false => {}
    }
    match scale != 1.0 {
      true => {
        for point in self.phantom.iter_mut() {
          *point *= scale;
        }
      }
      false => {}
    }
    for (i, component) in glyph.components().enumerate() {
      let phantom = self.phantom;

      let start_point = self.point_count;
      let component_glyph = self
        .outlines
        .loca
        .get_glyf(component.glyph.into(), &self.outlines.glyf)?;
      self.load(&component_glyph, component.glyph.into(), recurse_depth + 1)?;
      let end_point = self.point_count;
      let Ok(contains) = component
        .flags
        .contains(CompositeGlyphFlags::USE_MY_METRICS);

      match !(contains == read_fonts::Presence::Present) {
        true => {
          self.phantom = phantom;
        }
        false => {}
      }
      let Ok(have_xform) = component.flags.intersects(
        CompositeGlyphFlags::WE_HAVE_A_SCALE
          | CompositeGlyphFlags::WE_HAVE_AN_X_AND_Y_SCALE
          | CompositeGlyphFlags::WE_HAVE_A_TWO_BY_TWO,
      );
      let mut transform = match (({
        let explicit_value = have_xform;
        explicit_value
      }) == read_fonts::Overlap::Intersecting)
      {
        true => {
          let xform = &component.transform;
          [
            xform.xx,
            xform.yx,
            xform.xy,
            xform.yy,
            F2Dot14::ZERO,
            F2Dot14::ZERO,
          ]
          .map(|x| {
            ({
              let Ok(value) = x.to_f32();
              value
            })
          })
        }
        false => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
      };

      let anchor_offset = match component.anchor {
        Anchor::Offset { x, y } => {
          let (mut x, mut y) = (f32::from(x), f32::from(y));
          match ({
            let explicit_value = have_xform;
            (explicit_value == read_fonts::Overlap::Intersecting)
          }) && component.flags
            & (CompositeGlyphFlags::SCALED_COMPONENT_OFFSET
              | CompositeGlyphFlags::UNSCALED_COMPONENT_OFFSET)
            == CompositeGlyphFlags::SCALED_COMPONENT_OFFSET
          {
            true => {
              x *= hypot(transform[0], transform[2]);
              y *= hypot(transform[1], transform[3]);
            }
            false => {}
          }
          ({
            let Ok(value) = Point::new(x, y);
            value
          }) + (self.memory.composite_deltas.get(delta_base + i))
            .copied()
            .unwrap_or_default()
        }
        Anchor::Point { base, component } => {
          let (base_offset, component_offset) = (
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(base);
              migrated_value
            }),
            ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(component);
              migrated_value
            }),
          );
          let base_point = (self.memory.points.get(point_base + base_offset))
            .ok_or(DrawError::InvalidAnchorPoint(glyph_id, base))?;
          let component_point = (self.memory.points.get(start_point + component_offset))
            .ok_or(DrawError::InvalidAnchorPoint(glyph_id, component))?;
          *base_point - *component_point
        }
      };
      transform[4] = anchor_offset.x;
      transform[5] = anchor_offset.y;

      let points = &mut self.memory.points[start_point..end_point];
      for point in points.iter_mut() {
        *point = map_point(transform, *point);
      }
    }
    match have_deltas {
      true => {
        self.component_delta_count = delta_base;
      }
      false => {}
    }
    Ok(())
  }
}

/// Magnitude of the vector (x, y)
fn hypot(x: f32, y: f32) -> f32 {
  #[cfg(feature = "libm")]
  let length = libm::hypotf(x, y);
  #[cfg(not(feature = "libm"))]
  let length = x.hypot(y);
  length
}

fn map_point(transform: [f32; 6], p: Point<f32>) -> Point<f32> {
  Point {
    x: transform[0] * p.x + transform[2] * p.y + transform[4],
    y: transform[1] * p.x + transform[3] * p.y + transform[5],
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::MetadataProvider;
  use raw::{
    FontRead, FontRef, TableProvider,
    tables::{
      glyf::{CompositeGlyphFlags, Glyf, SimpleGlyphFlags},
      loca::Loca,
    },
  };

  #[test]
  fn overlap_flags() {
    let font = FontRef::new(font_test_data::VAZIRMATN_VAR).unwrap();
    let scaler = Outlines::new(&font).unwrap();
    let glyph_count = ({
      let Ok(value) = font.maxp().unwrap().num_glyphs();
      value
    });

    let expected_gids_with_overlap = vec![2, 3];
    assert_eq!(
      expected_gids_with_overlap,
      (0..glyph_count)
        .filter(|gid| scaler.outline(GlyphId::from(*gid)).unwrap().has_overlaps)
        .collect::<Vec<_>>()
    );
  }

  #[test]
  fn interpreter_preference() {
    let font = FontRef::new(font_test_data::COLRV0V1).unwrap();
    let outlines = Outlines::new(&font).unwrap();

    assert!(!outlines.prefer_interpreter());

    let font = FontRef::new(font_test_data::TTHINT_SUBSET).unwrap();
    let outlines = Outlines::new(&font).unwrap();

    assert!(outlines.prefer_interpreter());
  }

  #[test]
  fn empty_glyph_advance() {
    let font = FontRef::new(font_test_data::HVAR_WITH_TRUNCATED_ADVANCE_INDEX_MAP).unwrap();
    let outlines = Outlines::new(&font).unwrap();
    let coords = [({
      let Ok(value) = F2Dot14::from_f32(0.5);
      value
    })];
    let ppem = Some(24.0);
    let gid = font.charmap().map(' ').unwrap();
    let outline = outlines.outline(gid).unwrap();

    assert!(outline.glyph.is_none());
    let mut buf = [0u8; 128];
    let scaler = FreeTypeScaler::unhinted(&outlines, &outline, &mut buf, ppem, &coords).unwrap();
    let scaled = scaler.scale(&outline.glyph, gid).unwrap();
    let advance = scaled.adjusted_advance_width();
    assert!(advance != F26Dot6::ZERO);
  }

  #[test]
  fn empty_glyphs_have_phantom_points_too() {
    let font = FontRef::new(font_test_data::HVAR_WITH_TRUNCATED_ADVANCE_INDEX_MAP).unwrap();
    let outlines = Outlines::new(&font).unwrap();
    let gid = font.charmap().map(' ').unwrap();
    let outline = outlines.outline(gid).unwrap();
    assert!(outline.glyph.is_none());
    assert_eq!(outline.points, PHANTOM_POINT_COUNT);
  }

  #[test]
  fn composite_with_too_many_points() {
    let font = FontRef::new(font_test_data::GLYF_COMPONENTS).unwrap();
    let mut outlines = Outlines::new(&font).unwrap();

    let mut glyf_buf = font_test_data::bebuffer::BeBuffer::default();

    let simple_glyph_point_count = 40000;
    glyf_buf.extend([1u16]);
    glyf_buf.extend([0i16; 4]);
    glyf_buf.extend([({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped::<_, u16>((simple_glyph_point_count - 1));
      migrated_value
    })]);
    glyf_buf.extend([0u16]);
    for _ in 0..simple_glyph_point_count {
      glyf_buf.extend([SimpleGlyphFlags::X_SHORT_VECTOR | SimpleGlyphFlags::Y_SHORT_VECTOR]);
    }

    for _ in 0..simple_glyph_point_count * 2 {
      glyf_buf.extend([0u8]);
    }
    let Ok(glyph0_end) = glyf_buf.len();

    glyf_buf.extend([-1i16]);
    glyf_buf.extend([0i16; 4]);
    for i in 0..2 {
      let flags = match i == 0 {
        true => CompositeGlyphFlags::MORE_COMPONENTS | CompositeGlyphFlags::ARGS_ARE_XY_VALUES,
        false => CompositeGlyphFlags::ARGS_ARE_XY_VALUES,
      };
      glyf_buf.extend([flags]);
      glyf_buf.extend([0u16]);
      glyf_buf.extend([0u8; 2]);
    }
    let Ok(glyph1_end) = glyf_buf.len();
    let Ok(glyf_data) = glyf_buf.data();
    outlines.glyf = Glyf::read(glyf_data.into()).unwrap();

    let mut loca_buf = font_test_data::bebuffer::BeBuffer::default();
    loca_buf.extend([
      0u32,
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph0_end);
        migrated_value
      }),
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(glyph1_end);
        migrated_value
      }),
    ]);
    let Ok(loca_data) = loca_buf.data();
    outlines.loca = Loca::read(loca_data.into(), read_fonts::tables::loca::OffsetFormat::Long).unwrap();
    let gid = ({
      let Ok(value) = GlyphId::new(1);
      value
    });
    let outline = outlines.outline(gid).unwrap();
    let mut mem_buf = vec![0u8; outline.required_buffer_size(Default::default())];

    assert!(
      outline.points
        > ({
          let Ok(migrated_value) = console_core_number_conversion::wrapped_index(u16::MAX);
          migrated_value
        })
    );
    let result = FreeTypeScaler::unhinted(&outlines, &outline, &mut mem_buf, None, &[]);

    assert!(matches!(result, Err(DrawError::TooManyPoints(_))));
  }

  #[test]
  fn fractional_size_hinting() {
    let font = FontRef::from_index(font_test_data::TINOS_SUBSET, 0).unwrap();
    let outlines = Outlines::new(&font).unwrap();

    assert!(!outlines.fractional_size_hinting);

    for size in [10.0, 10.2, 10.5, 10.8, 11.0] {
      assert_eq!(
        outlines.compute_hinted_scale(Some(size)).scale,
        outlines.compute_hinted_scale(Some(size.round())).scale
      );
    }
  }
}
