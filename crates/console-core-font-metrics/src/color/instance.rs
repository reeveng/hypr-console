//! COLR table instance.

use read_fonts::{
  ReadError,
  tables::{
    colr::*,
    variations::{
      DeltaSetIndex, DeltaSetIndexMap, FloatItemDelta, FloatItemDeltaTarget, ItemVariationStore,
    },
  },
  types::{BoundingBox, F2Dot14, GlyphId16, Point},
};

use core::ops::{Deref, Range};

/// Unique paint identifier used for detecting cycles in the paint graph.
pub type PaintId = usize;

/// Combination of a `COLR` table and a location in variation space for
/// resolving paints.
///
/// See [`resolve_paint`], [`ColorStops::resolve`] and [`resolve_clip_box`].
#[derive(Clone)]
pub struct ColrInstance<'a> {
  colr: Colr<'a>,
  index_map: Option<DeltaSetIndexMap<'a>>,
  var_store: Option<ItemVariationStore<'a>>,
  coords: &'a [F2Dot14],
}

impl<'a> ColrInstance<'a> {
  /// Creates a new instance for the given `COLR` table and normalized variation
  /// coordinates.
  pub fn new(colr: Colr<'a>, coords: &'a [F2Dot14]) -> Self {
    let index_map = colr.var_index_map().ok().flatten();
    let var_store = colr.item_variation_store().ok().flatten();
    Self {
      colr,
      coords,
      index_map,
      var_store,
    }
  }

  /// Computes a sequence of N variation deltas starting at the given
  /// `var_base` index.
  fn var_deltas<const N: usize>(&self, var_index_base: u32) -> [FloatItemDelta; N] {
    const NO_VARIATION_DELTAS: u32 = 0xFFFFFFFF;

    let mut deltas = [FloatItemDelta::ZERO; N];
    match self.coords.is_empty()
      || self.var_store.is_none()
      || var_index_base == NO_VARIATION_DELTAS
    {
      true => {
        return deltas;
      }
      false => {}
    }

    let actual_count = ({
      let Ok(migrated_value) =
        console_core_number_conversion::wrapped_index((u32::MAX - var_index_base));
      migrated_value
    })
    .min(N);
    let var_store = self.var_store.as_ref().unwrap();
    match self.index_map.as_ref() {
      Some(index_map) => {
        for (i, delta) in deltas.iter_mut().enumerate().take(actual_count) {
          let var_index = var_index_base
            + ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
              migrated_value
            });
          match index_map.get(var_index) {
            Ok(delta_ix) => {
              *delta = match var_store.compute_float_delta(delta_ix, self.coords) {
                Ok(v) => v,
                Err(_) => FloatItemDelta::ZERO,
              };
            }
            Err(_) => {}
          }
        }
      }
      None => {
        for (i, delta) in deltas.iter_mut().enumerate().take(actual_count) {
          let var_index = var_index_base
            + ({
              let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, u32>(i);
              migrated_value
            });

          let delta_ix = DeltaSetIndex {
            outer: 0,
            inner: ({
              let Ok(migrated_value) =
                console_core_number_conversion::wrapped::<_, u16>(var_index);
              migrated_value
            }),
          };
          *delta = match var_store.compute_float_delta(delta_ix, self.coords) {
            Ok(v) => v,
            Err(_) => FloatItemDelta::ZERO,
          };
        }
      }
    }
    deltas
  }
}

impl<'a> Deref for ColrInstance<'a> {
  type Target = Colr<'a>;

  fn deref(&self) -> &Self::Target {
    &self.colr
  }
}

/// Resolves a clip box, applying variation deltas using the given
/// instance.
pub fn resolve_clip_box(instance: &ColrInstance, clip_box: &ClipBox) -> BoundingBox<f32> {
  match clip_box {
    ClipBox::Format1(cbox) => BoundingBox {
      x_min: f32::from(
        ({
          let Ok(x_min) = cbox.x_min();
          let Ok(value) = x_min.to_i16();
          value
        }),
      ),
      y_min: f32::from(
        ({
          let Ok(y_min) = cbox.y_min();
          let Ok(value) = y_min.to_i16();
          value
        }),
      ),
      x_max: f32::from(
        ({
          let Ok(x_max) = cbox.x_max();
          let Ok(value) = x_max.to_i16();
          value
        }),
      ),
      y_max: f32::from(
        ({
          let Ok(y_max) = cbox.y_max();
          let Ok(value) = y_max.to_i16();
          value
        }),
      ),
    },
    ClipBox::Format2(cbox) => {
      let Ok(var_index_base) = cbox.var_index_base();
      let deltas = instance.var_deltas::<4>(var_index_base);
      let Ok(x_min) = cbox.x_min();

      BoundingBox {
        x_min: x_min.apply_float_delta(deltas[0]),
        y_min: ({
          let Ok(explicit_value) = cbox.y_min();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        x_max: ({
          let Ok(explicit_value) = cbox.x_max();
          explicit_value
        })
        .apply_float_delta(deltas[2]),
        y_max: ({
          let Ok(explicit_value) = cbox.y_max();
          explicit_value
        })
        .apply_float_delta(deltas[3]),
      }
    }
  }
}

/// Simplified version of a [`ColorStop`] or [`VarColorStop`] with applied
/// variation deltas.
#[derive(Clone, Debug)]
pub struct ResolvedColorStop {
  pub offset: f32,
  pub palette_index: u16,
  pub alpha: f32,
}

/// Collection of [`ColorStop`] or [`VarColorStop`].

#[derive(Clone)]
pub struct ColorStops<'a> {
  stops: &'a [ColorStop],
  var_stops: &'a [VarColorStop],
}

impl ColorStops<'_> {
  pub fn len(&self) -> usize {
    self.stops.len() + self.var_stops.len()
  }

  pub fn is_empty(&self) -> bool {
    self.stops.is_empty() && self.var_stops.is_empty()
  }
}

impl<'a> From<ColorLine<'a>> for ColorStops<'a> {
  fn from(value: ColorLine<'a>) -> Self {
    let Ok(stops) = value.color_stops();

    Self {
      stops,
      var_stops: &[],
    }
  }
}

impl<'a> From<VarColorLine<'a>> for ColorStops<'a> {
  fn from(value: VarColorLine<'a>) -> Self {
    let Ok(var_stops) = value.color_stops();

    Self {
      stops: &[],
      var_stops,
    }
  }
}

impl<'a> ColorStops<'a> {
  /// Returns an iterator yielding resolved color stops with variation deltas
  /// applied.
  pub fn resolve(
    &self,
    instance: &'a ColrInstance<'a>,
  ) -> impl Iterator<Item = ResolvedColorStop> + 'a {
    self
      .stops
      .iter()
      .map(|stop| ResolvedColorStop {
        offset: ({
          let Ok(stop_offset) = stop.stop_offset();
          let Ok(value) = stop_offset.to_f32();
          value
        }),
        palette_index: ({
          let Ok(explicit_value) = stop.palette_index();
          explicit_value
        }),
        alpha: ({
          let Ok(alpha) = stop.alpha();
          let Ok(value) = alpha.to_f32();
          value
        }),
      })
      .chain(self.var_stops.iter().map(|stop| {
        let Ok(var_index_base) = stop.var_index_base();
        let deltas = instance.var_deltas::<2>(var_index_base);
        let Ok(stop_offset) = stop.stop_offset();

        ResolvedColorStop {
          offset: stop_offset.apply_float_delta(deltas[0]),
          palette_index: ({
            let Ok(explicit_value) = stop.palette_index();
            explicit_value
          }),
          alpha: ({
            let Ok(explicit_value) = stop.alpha();
            explicit_value
          })
          .apply_float_delta(deltas[1]),
        }
      }))
  }
}

/// Simplified version of `Paint` with applied variation deltas.
///
/// These are constructed with the [`resolve_paint`] function.
///
/// This is roughly equivalent to FreeType's
/// [`FT_COLR_Paint`](https://freetype.org/freetype2/docs/reference/ft2-layer_management.html#ft_colr_paint)
/// type.
pub enum ResolvedPaint<'a> {
  ColrLayers {
    range: Range<usize>,
  },
  Solid {
    palette_index: u16,
    alpha: f32,
  },
  LinearGradient {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color_stops: ColorStops<'a>,
    extend: Extend,
  },
  RadialGradient {
    x0: f32,
    y0: f32,
    radius0: f32,
    x1: f32,
    y1: f32,
    radius1: f32,
    color_stops: ColorStops<'a>,
    extend: Extend,
  },
  SweepGradient {
    center_x: f32,
    center_y: f32,
    start_angle: f32,
    end_angle: f32,
    color_stops: ColorStops<'a>,
    extend: Extend,
  },
  Glyph {
    glyph_id: GlyphId16,
    paint: Paint<'a>,
  },
  ColrGlyph {
    glyph_id: GlyphId16,
  },
  Transform {
    xx: f32,
    yx: f32,
    xy: f32,
    yy: f32,
    dx: f32,
    dy: f32,
    paint: Paint<'a>,
  },
  Translate {
    dx: f32,
    dy: f32,
    paint: Paint<'a>,
  },
  Scale {
    scale_x: f32,
    scale_y: f32,
    around_center: Option<Point<f32>>,
    paint: Paint<'a>,
  },
  Rotate {
    angle: f32,
    around_center: Option<Point<f32>>,
    paint: Paint<'a>,
  },
  Skew {
    x_skew_angle: f32,
    y_skew_angle: f32,
    around_center: Option<Point<f32>>,
    paint: Paint<'a>,
  },
  Composite {
    source_paint: Paint<'a>,
    mode: CompositeMode,
    backdrop_paint: Paint<'a>,
  },
}

/// Resolves this paint with the given instance.
///
/// Resolving means that all numeric values are converted to 32-bit floating
/// point, variation deltas are applied (also computed fully in floating
/// point), and the various transform paints are collapsed into a single value
/// for their category (transform, translate, scale, rotate and skew).
///
/// This provides a simpler type for consumers that are more interested
/// in extracting the semantics of the graph rather than working with the
/// raw encoded structures.
pub fn resolve_paint<'a>(
  instance: &ColrInstance<'a>,
  paint: &Paint<'a>,
) -> Result<ResolvedPaint<'a>, ReadError> {
  Ok(match paint {
    Paint::ColrLayers(layers) => {
      let start = ({
        let Ok(first_layer_index) = layers.first_layer_index();
        let Ok(migrated_value) = console_core_number_conversion::wrapped_index(first_layer_index);
        migrated_value
      });
      ResolvedPaint::ColrLayers {
        range: start
          ..start
            + ({
              let Ok(num_layers) = layers.num_layers();
              let Ok(migrated_value) = console_core_number_conversion::wrapped_index(num_layers);
              migrated_value
            }),
      }
    }
    Paint::Solid(solid) => {
      let Ok(palette_index) = solid.palette_index();

      ResolvedPaint::Solid {
        palette_index,
        alpha: ({
          let Ok(alpha) = solid.alpha();
          let Ok(value) = alpha.to_f32();
          value
        }),
      }
    }
    Paint::VarSolid(solid) => {
      let Ok(var_index_base) = solid.var_index_base();
      let deltas = instance.var_deltas::<1>(var_index_base);
      let Ok(palette_index) = solid.palette_index();
      let Ok(alpha) = solid.alpha();

      ResolvedPaint::Solid {
        palette_index,
        alpha: alpha.apply_float_delta(deltas[0]),
      }
    }
    Paint::LinearGradient(gradient) => {
      let color_line = gradient.color_line()?;
      let Ok(extend) = color_line.extend();
      ResolvedPaint::LinearGradient {
        x0: f32::from(
          ({
            let Ok(x0) = gradient.x0();
            let Ok(value) = x0.to_i16();
            value
          }),
        ),
        y0: f32::from(
          ({
            let Ok(y0) = gradient.y0();
            let Ok(value) = y0.to_i16();
            value
          }),
        ),
        x1: f32::from(
          ({
            let Ok(x1) = gradient.x1();
            let Ok(value) = x1.to_i16();
            value
          }),
        ),
        y1: f32::from(
          ({
            let Ok(y1) = gradient.y1();
            let Ok(value) = y1.to_i16();
            value
          }),
        ),
        x2: f32::from(
          ({
            let Ok(x2) = gradient.x2();
            let Ok(value) = x2.to_i16();
            value
          }),
        ),
        y2: f32::from(
          ({
            let Ok(y2) = gradient.y2();
            let Ok(value) = y2.to_i16();
            value
          }),
        ),
        color_stops: color_line.into(),
        extend: extend,
      }
    }
    Paint::VarLinearGradient(gradient) => {
      let color_line = gradient.color_line()?;
      let Ok(extend) = color_line.extend();
      let Ok(var_index_base) = gradient.var_index_base();
      let deltas = instance.var_deltas::<6>(var_index_base);
      let Ok(x0) = gradient.x0();

      ResolvedPaint::LinearGradient {
        x0: x0.apply_float_delta(deltas[0]),
        y0: ({
          let Ok(explicit_value) = gradient.y0();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        x1: ({
          let Ok(explicit_value) = gradient.x1();
          explicit_value
        })
        .apply_float_delta(deltas[2]),
        y1: ({
          let Ok(explicit_value) = gradient.y1();
          explicit_value
        })
        .apply_float_delta(deltas[3]),
        x2: ({
          let Ok(explicit_value) = gradient.x2();
          explicit_value
        })
        .apply_float_delta(deltas[4]),
        y2: ({
          let Ok(explicit_value) = gradient.y2();
          explicit_value
        })
        .apply_float_delta(deltas[5]),
        color_stops: color_line.into(),
        extend: extend,
      }
    }
    Paint::RadialGradient(gradient) => {
      let color_line = gradient.color_line()?;
      let Ok(extend) = color_line.extend();
      ResolvedPaint::RadialGradient {
        x0: f32::from(
          ({
            let Ok(x0) = gradient.x0();
            let Ok(value) = x0.to_i16();
            value
          }),
        ),
        y0: f32::from(
          ({
            let Ok(y0) = gradient.y0();
            let Ok(value) = y0.to_i16();
            value
          }),
        ),
        radius0: f32::from(
          ({
            let Ok(radius0) = gradient.radius0();
            let Ok(value) = radius0.to_u16();
            value
          }),
        ),
        x1: f32::from(
          ({
            let Ok(x1) = gradient.x1();
            let Ok(value) = x1.to_i16();
            value
          }),
        ),
        y1: f32::from(
          ({
            let Ok(y1) = gradient.y1();
            let Ok(value) = y1.to_i16();
            value
          }),
        ),
        radius1: f32::from(
          ({
            let Ok(radius1) = gradient.radius1();
            let Ok(value) = radius1.to_u16();
            value
          }),
        ),
        color_stops: color_line.into(),
        extend: extend,
      }
    }
    Paint::VarRadialGradient(gradient) => {
      let color_line = gradient.color_line()?;
      let Ok(extend) = color_line.extend();
      let Ok(var_index_base) = gradient.var_index_base();
      let deltas = instance.var_deltas::<6>(var_index_base);
      let Ok(x0) = gradient.x0();

      ResolvedPaint::RadialGradient {
        x0: x0.apply_float_delta(deltas[0]),
        y0: ({
          let Ok(explicit_value) = gradient.y0();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        radius0: ({
          let Ok(explicit_value) = gradient.radius0();
          explicit_value
        })
        .apply_float_delta(deltas[2]),
        x1: ({
          let Ok(explicit_value) = gradient.x1();
          explicit_value
        })
        .apply_float_delta(deltas[3]),
        y1: ({
          let Ok(explicit_value) = gradient.y1();
          explicit_value
        })
        .apply_float_delta(deltas[4]),
        radius1: ({
          let Ok(explicit_value) = gradient.radius1();
          explicit_value
        })
        .apply_float_delta(deltas[5]),
        color_stops: color_line.into(),
        extend: extend,
      }
    }
    Paint::SweepGradient(gradient) => {
      let color_line = gradient.color_line()?;
      let Ok(extend) = color_line.extend();
      ResolvedPaint::SweepGradient {
        center_x: f32::from(
          ({
            let Ok(center_x) = gradient.center_x();
            let Ok(value) = center_x.to_i16();
            value
          }),
        ),
        center_y: f32::from(
          ({
            let Ok(center_y) = gradient.center_y();
            let Ok(value) = center_y.to_i16();
            value
          }),
        ),
        start_angle: ({
          let Ok(start_angle) = gradient.start_angle();
          let Ok(value) = start_angle.to_f32();
          value
        }),
        end_angle: ({
          let Ok(end_angle) = gradient.end_angle();
          let Ok(value) = end_angle.to_f32();
          value
        }),
        color_stops: color_line.into(),
        extend: extend,
      }
    }
    Paint::VarSweepGradient(gradient) => {
      let color_line = gradient.color_line()?;
      let Ok(extend) = color_line.extend();
      let Ok(var_index_base) = gradient.var_index_base();
      let deltas = instance.var_deltas::<4>(var_index_base);
      let Ok(center_x) = gradient.center_x();

      ResolvedPaint::SweepGradient {
        center_x: center_x.apply_float_delta(deltas[0]),
        center_y: ({
          let Ok(explicit_value) = gradient.center_y();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        start_angle: ({
          let Ok(explicit_value) = gradient.start_angle();
          explicit_value
        })
        .apply_float_delta(deltas[2]),
        end_angle: ({
          let Ok(explicit_value) = gradient.end_angle();
          explicit_value
        })
        .apply_float_delta(deltas[3]),
        color_stops: color_line.into(),
        extend: extend,
      }
    }
    Paint::Glyph(glyph) => {
      let Ok(glyph_id) = glyph.glyph_id();

      ResolvedPaint::Glyph {
        glyph_id,
        paint: glyph.paint()?,
      }
    }
    Paint::ColrGlyph(glyph) => {
      let Ok(glyph_id) = glyph.glyph_id();

      ResolvedPaint::ColrGlyph { glyph_id }
    }
    Paint::Transform(transform) => {
      let affine = transform.transform()?;
      let paint = transform.paint()?;
      ResolvedPaint::Transform {
        xx: ({
          let Ok(xx) = affine.xx();
          let Ok(value) = xx.to_f32();
          value
        }),
        yx: ({
          let Ok(yx) = affine.yx();
          let Ok(value) = yx.to_f32();
          value
        }),
        xy: ({
          let Ok(xy) = affine.xy();
          let Ok(value) = xy.to_f32();
          value
        }),
        yy: ({
          let Ok(yy) = affine.yy();
          let Ok(value) = yy.to_f32();
          value
        }),
        dx: ({
          let Ok(dx) = affine.dx();
          let Ok(value) = dx.to_f32();
          value
        }),
        dy: ({
          let Ok(dy) = affine.dy();
          let Ok(value) = dy.to_f32();
          value
        }),
        paint,
      }
    }
    Paint::VarTransform(transform) => {
      let affine = transform.transform()?;
      let paint = transform.paint()?;
      let Ok(var_index_base) = affine.var_index_base();
      let deltas = instance.var_deltas::<6>(var_index_base);
      let Ok(xx) = affine.xx();

      ResolvedPaint::Transform {
        xx: xx.apply_float_delta(deltas[0]),
        yx: ({
          let Ok(explicit_value) = affine.yx();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        xy: ({
          let Ok(explicit_value) = affine.xy();
          explicit_value
        })
        .apply_float_delta(deltas[2]),
        yy: ({
          let Ok(explicit_value) = affine.yy();
          explicit_value
        })
        .apply_float_delta(deltas[3]),
        dx: ({
          let Ok(explicit_value) = affine.dx();
          explicit_value
        })
        .apply_float_delta(deltas[4]),
        dy: ({
          let Ok(explicit_value) = affine.dy();
          explicit_value
        })
        .apply_float_delta(deltas[5]),
        paint,
      }
    }
    Paint::Translate(transform) => ResolvedPaint::Translate {
      dx: f32::from(
        ({
          let Ok(dx) = transform.dx();
          let Ok(value) = dx.to_i16();
          value
        }),
      ),
      dy: f32::from(
        ({
          let Ok(dy) = transform.dy();
          let Ok(value) = dy.to_i16();
          value
        }),
      ),
      paint: transform.paint()?,
    },
    Paint::VarTranslate(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<2>(var_index_base);
      let Ok(dx) = transform.dx();

      ResolvedPaint::Translate {
        dx: dx.apply_float_delta(deltas[0]),
        dy: ({
          let Ok(explicit_value) = transform.dy();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        paint: transform.paint()?,
      }
    }
    Paint::Scale(transform) => ResolvedPaint::Scale {
      scale_x: ({
        let Ok(scale_x) = transform.scale_x();
        let Ok(value) = scale_x.to_f32();
        value
      }),
      scale_y: ({
        let Ok(scale_y) = transform.scale_y();
        let Ok(value) = scale_y.to_f32();
        value
      }),
      around_center: None,
      paint: transform.paint()?,
    },
    Paint::VarScale(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<2>(var_index_base);
      let Ok(scale_x) = transform.scale_x();

      ResolvedPaint::Scale {
        scale_x: scale_x.apply_float_delta(deltas[0]),
        scale_y: ({
          let Ok(explicit_value) = transform.scale_y();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        around_center: None,
        paint: transform.paint()?,
      }
    }
    Paint::ScaleAroundCenter(transform) => ResolvedPaint::Scale {
      scale_x: ({
        let Ok(scale_x) = transform.scale_x();
        let Ok(value) = scale_x.to_f32();
        value
      }),
      scale_y: ({
        let Ok(scale_y) = transform.scale_y();
        let Ok(value) = scale_y.to_f32();
        value
      }),
      around_center: Some(
        ({
          let Ok(value) = Point::new(
            f32::from(
              ({
                let Ok(center_x) = transform.center_x();
                let Ok(value) = center_x.to_i16();
                value
              }),
            ),
            f32::from(
              ({
                let Ok(center_y) = transform.center_y();
                let Ok(value) = center_y.to_i16();
                value
              }),
            ),
          );
          value
        }),
      ),
      paint: transform.paint()?,
    },
    Paint::VarScaleAroundCenter(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<4>(var_index_base);
      let Ok(scale_x) = transform.scale_x();

      ResolvedPaint::Scale {
        scale_x: scale_x.apply_float_delta(deltas[0]),
        scale_y: ({
          let Ok(explicit_value) = transform.scale_y();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        around_center: Some({
          let Ok(center_x) = transform.center_x();
          let Ok(value) = Point::new(
            center_x.apply_float_delta(deltas[2]),
            ({
              let Ok(explicit_value) = transform.center_y();
              explicit_value
            })
            .apply_float_delta(deltas[3]),
          );
          value
        }),
        paint: transform.paint()?,
      }
    }
    Paint::ScaleUniform(transform) => {
      let scale = ({
        let Ok(transform_scale) = transform.scale();
        let Ok(value) = transform_scale.to_f32();
        value
      });
      ResolvedPaint::Scale {
        scale_x: (scale),
        scale_y: (scale),
        around_center: None,
        paint: transform.paint()?,
      }
    }
    Paint::VarScaleUniform(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<1>(var_index_base);
      let Ok(transform_scale) = transform.scale();
      let scale = transform_scale.apply_float_delta(deltas[0]);
      ResolvedPaint::Scale {
        scale_x: scale,
        scale_y: scale,
        around_center: None,
        paint: transform.paint()?,
      }
    }
    Paint::ScaleUniformAroundCenter(transform) => {
      let scale = ({
        let Ok(transform_scale) = transform.scale();
        let Ok(value) = transform_scale.to_f32();
        value
      });
      ResolvedPaint::Scale {
        scale_x: (scale),
        scale_y: (scale),
        around_center: Some(
          ({
            let Ok(value) = Point::new(
              f32::from(
                ({
                  let Ok(center_x) = transform.center_x();
                  let Ok(value) = center_x.to_i16();
                  value
                }),
              ),
              f32::from(
                ({
                  let Ok(center_y) = transform.center_y();
                  let Ok(value) = center_y.to_i16();
                  value
                }),
              ),
            );
            value
          }),
        ),
        paint: transform.paint()?,
      }
    }
    Paint::VarScaleUniformAroundCenter(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<3>(var_index_base);
      let Ok(transform_scale) = transform.scale();
      let scale = transform_scale.apply_float_delta(deltas[0]);
      ResolvedPaint::Scale {
        scale_x: scale,
        scale_y: scale,
        around_center: Some({
          let Ok(center_x) = transform.center_x();
          let Ok(value) = Point::new(
            center_x.apply_float_delta(deltas[1]),
            ({
              let Ok(explicit_value) = transform.center_y();
              explicit_value
            })
            .apply_float_delta(deltas[2]),
          );
          value
        }),
        paint: transform.paint()?,
      }
    }
    Paint::Rotate(transform) => ResolvedPaint::Rotate {
      angle: ({
        let Ok(angle) = transform.angle();
        let Ok(value) = angle.to_f32();
        value
      }),
      around_center: None,
      paint: transform.paint()?,
    },
    Paint::VarRotate(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<1>(var_index_base);
      let Ok(angle) = transform.angle();

      ResolvedPaint::Rotate {
        angle: angle.apply_float_delta(deltas[0]),
        around_center: None,
        paint: transform.paint()?,
      }
    }
    Paint::RotateAroundCenter(transform) => ResolvedPaint::Rotate {
      angle: ({
        let Ok(angle) = transform.angle();
        let Ok(value) = angle.to_f32();
        value
      }),
      around_center: Some(
        ({
          let Ok(value) = Point::new(
            f32::from(
              ({
                let Ok(center_x) = transform.center_x();
                let Ok(value) = center_x.to_i16();
                value
              }),
            ),
            f32::from(
              ({
                let Ok(center_y) = transform.center_y();
                let Ok(value) = center_y.to_i16();
                value
              }),
            ),
          );
          value
        }),
      ),
      paint: transform.paint()?,
    },
    Paint::VarRotateAroundCenter(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<3>(var_index_base);
      let Ok(angle) = transform.angle();

      ResolvedPaint::Rotate {
        angle: angle.apply_float_delta(deltas[0]),
        around_center: Some({
          let Ok(center_x) = transform.center_x();
          let Ok(value) = Point::new(
            center_x.apply_float_delta(deltas[1]),
            ({
              let Ok(explicit_value) = transform.center_y();
              explicit_value
            })
            .apply_float_delta(deltas[2]),
          );
          value
        }),
        paint: transform.paint()?,
      }
    }
    Paint::Skew(transform) => ResolvedPaint::Skew {
      x_skew_angle: ({
        let Ok(x_skew_angle) = transform.x_skew_angle();
        let Ok(value) = x_skew_angle.to_f32();
        value
      }),
      y_skew_angle: ({
        let Ok(y_skew_angle) = transform.y_skew_angle();
        let Ok(value) = y_skew_angle.to_f32();
        value
      }),
      around_center: None,
      paint: transform.paint()?,
    },
    Paint::VarSkew(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<2>(var_index_base);
      let Ok(x_skew_angle) = transform.x_skew_angle();

      ResolvedPaint::Skew {
        x_skew_angle: x_skew_angle.apply_float_delta(deltas[0]),
        y_skew_angle: ({
          let Ok(explicit_value) = transform.y_skew_angle();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        around_center: None,
        paint: transform.paint()?,
      }
    }
    Paint::SkewAroundCenter(transform) => ResolvedPaint::Skew {
      x_skew_angle: ({
        let Ok(x_skew_angle) = transform.x_skew_angle();
        let Ok(value) = x_skew_angle.to_f32();
        value
      }),
      y_skew_angle: ({
        let Ok(y_skew_angle) = transform.y_skew_angle();
        let Ok(value) = y_skew_angle.to_f32();
        value
      }),
      around_center: Some(
        ({
          let Ok(value) = Point::new(
            f32::from(
              ({
                let Ok(center_x) = transform.center_x();
                let Ok(value) = center_x.to_i16();
                value
              }),
            ),
            f32::from(
              ({
                let Ok(center_y) = transform.center_y();
                let Ok(value) = center_y.to_i16();
                value
              }),
            ),
          );
          value
        }),
      ),
      paint: transform.paint()?,
    },
    Paint::VarSkewAroundCenter(transform) => {
      let Ok(var_index_base) = transform.var_index_base();
      let deltas = instance.var_deltas::<4>(var_index_base);
      let Ok(x_skew_angle) = transform.x_skew_angle();

      ResolvedPaint::Skew {
        x_skew_angle: x_skew_angle.apply_float_delta(deltas[0]),
        y_skew_angle: ({
          let Ok(explicit_value) = transform.y_skew_angle();
          explicit_value
        })
        .apply_float_delta(deltas[1]),
        around_center: Some({
          let Ok(center_x) = transform.center_x();
          let Ok(value) = Point::new(
            center_x.apply_float_delta(deltas[2]),
            ({
              let Ok(explicit_value) = transform.center_y();
              explicit_value
            })
            .apply_float_delta(deltas[3]),
          );
          value
        }),
        paint: transform.paint()?,
      }
    }
    Paint::Composite(composite) => ResolvedPaint::Composite {
      source_paint: composite.source_paint()?,
      mode: ({
        let Ok(explicit_value) = composite.composite_mode();
        explicit_value
      }),
      backdrop_paint: composite.backdrop_paint()?,
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use raw::{FontRef, TableProvider};

  /// OSS Fuzz caught add with overflow when computing delta indices.
  /// See <https://oss-fuzz.com/testcase-detail/5180237819478016>
  /// and <https://g-issues.oss-fuzz.com/issues/439498857>
  #[test]
  fn var_delta_index_overflow() {
    let font = FontRef::new(font_test_data::COLRV0V1_VARIABLE).unwrap();
    let coords = &[({
      let Ok(value) = F2Dot14::from_f32(0.5);
      value
    })];
    let instance = ColrInstance::new(font.colr().unwrap(), coords);

    let _: [FloatItemDelta; 4] = instance.var_deltas(0xFFFFFFFE);
  }
}
