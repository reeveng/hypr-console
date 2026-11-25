use crate::*;
use alloc::borrow::Cow;
use console_core_never::Never;
use console_core_number_conversion::{
  nearest_f32_from_i64, nearest_f32_from_u64, toward_zero_i32,
};

use console_core_paths::{IsIdentityAnswer, PathStroker, SCALAR_MAX, Scalar, ScalarProximity};
use console_core_size::Size;

use crate::geom::{PixelPosition, ScreenIntRect};
use crate::mask::SubMaskRef;
use crate::pipeline::{RasterPipelineBlitter, RasterPipelineBuilder};
use crate::pixmap::SubPixmapMut;
use crate::scan;

use crate::geom::IntSizeExt;
#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum FillRule {
  #[default]
  Winding,

  EvenOdd,
}

#[doc = "Coverage applied to the edges of a drawn path."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageMode {
  Antialiased,
  Binary,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Paint<'a> {
  pub shader: Shader<'a>,

  pub blend_mode: BlendMode,

  pub coverage: CoverageMode,

  pub colorspace: ColorSpace,

  pub precision: PipelinePrecision,
}

impl Default for Paint<'_> {
  fn default() -> Self {
    Paint {
      shader: Shader::SolidColor(Color::BLACK),
      blend_mode: BlendMode::default(),
      coverage: CoverageMode::Antialiased,
      colorspace: ColorSpace::default(),
      precision: PipelinePrecision::Automatic,
    }
  }
}

impl Paint<'_> {
  pub fn set_color(
    &mut self,
    color: Color,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.shader = Shader::SolidColor(color);

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the existing API follows RGBA channel order"
    )
  )]
  pub fn set_color_rgba8(
    &mut self,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(explicit_value) = self.set_color({
        let Ok(explicit_value) = Color::from_rgba8(r, g, b, a);
        explicit_value
      });
      explicit_value
    }

    Ok(())
  }

  pub fn is_solid_color(
    &self,
  ) -> core::result::Result<crate::IsSolidColorAnswer, console_core_never::Never> {
    Ok(match self.shader {
      Shader::SolidColor(_) => crate::IsSolidColorAnswer::Yes,
      Shader::LinearGradient(_)
      | Shader::RadialGradient(_)
      | Shader::SweepGradient(_)
      | Shader::Pattern(_) => crate::IsSolidColorAnswer::No,
    })
  }
}

impl Pixmap {
  pub fn fill_rect(
    &mut self,
    rect: FloatRectangle,
    paint: &Paint,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> core::result::Result<(), console_core_never::Never> {
    ({
      let Ok(explicit_value) = ({
        let Ok(explicit_value) = self.as_mut();
        explicit_value
      })
      .fill_rect(rect, paint, transform, mask);
      explicit_value
    });

    Ok(())
  }

  pub fn fill_path(
    &mut self,
    path: &Path,
    paint: &Paint,
    fill_rule: FillRule,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> core::result::Result<(), console_core_never::Never> {
    ({
      let Ok(explicit_value) = ({
        let Ok(explicit_value) = self.as_mut();
        explicit_value
      })
      .fill_path(path, paint, fill_rule, transform, mask);
      explicit_value
    });

    Ok(())
  }

  pub fn stroke_path(
    &mut self,
    path: &Path,
    paint: &Paint,
    stroke: &Stroke,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> core::result::Result<(), console_core_never::Never> {
    ({
      let Ok(explicit_value) = ({
        let Ok(explicit_value) = self.as_mut();
        explicit_value
      })
      .stroke_path(path, paint, stroke, transform, mask);
      explicit_value
    });

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the existing API takes the horizontal then vertical destination coordinate"
    )
  )]
  pub fn draw_pixmap(
    &mut self,
    x: i32,
    y: i32,
    pixmap: PixmapRef,
    paint: &PixmapPaint,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> core::result::Result<(), console_core_never::Never> {
    ({
      let Ok(explicit_value) = ({
        let Ok(explicit_value) = self.as_mut();
        explicit_value
      })
      .draw_pixmap(x, y, pixmap, paint, transform, mask);
      explicit_value
    });

    Ok(())
  }

  pub fn apply_mask(
    &mut self,
    mask: &Mask,
  ) -> core::result::Result<(), console_core_never::Never> {
    ({
      let Ok(explicit_value) = ({
        let Ok(explicit_value) = self.as_mut();
        explicit_value
      })
      .apply_mask(mask);
      explicit_value
    });

    Ok(())
  }
}

impl PixmapMut<'_> {
  pub fn fill_rect(
    &mut self,
    rect: FloatRectangle,
    paint: &Paint,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      matches!(
        {
          let Ok(explicit_value) = transform.is_identity();
          explicit_value
        },
        console_core_paths::IsIdentityAnswer::Yes
      ) && !matches!(
        {
          let Ok(size) = self.size();
          let Ok(explicit_value) = DrawTiler::required(size);
          explicit_value
        },
        crate::RequiredAnswer::Yes
      ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let Ok(size) = self.size();
        let Ok(bounds) = size.at_origin();
        let clip = match bounds {
          Some(bounds_2) => bounds_2,
          None => return Ok(()),
        };

        let mask_3 = mask.map(|mask_2| {
          let Ok(explicit_value) = mask_2.as_submask();
          explicit_value
        });
        let mut subpix = {
          let Ok(explicit_value) = self.as_subpixmap();
          explicit_value
        };
        let Ok(raster_pipeline_blitter) = RasterPipelineBlitter::new(paint, mask_3, &mut subpix);

        let mut blitter = match raster_pipeline_blitter {
          Some(v) => v,
          None => return Ok(()),
        };

        match paint.coverage {
          CoverageMode::Antialiased => {
            ({
              let Ok(explicit_value) = scan::fill_rect_aa(&rect, &clip, &mut blitter);
              explicit_value
            });
          }
          CoverageMode::Binary => {
            ({
              let Ok(explicit_value) = scan::fill_rect(&rect, &clip, &mut blitter);
              explicit_value
            });
          }
        }
      }
      ::core::option::Option::None => {
        let path = {
          let Ok(explicit_value) = PathBuilder::from_rect(rect);
          explicit_value
        };
        ({
          let Ok(explicit_value) =
            self.fill_path(&path, paint, FillRule::Winding, transform, mask);
          explicit_value
        });
      }
    }

    Ok(())
  }

  pub fn fill_path(
    &mut self,
    path: &Path,
    paint: &Paint,
    fill_rule: FillRule,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> Result<(), Never> {
    let Ok(candidate) = filled_path(path, transform);
    let region = match candidate {
      Some(value) => value,
      None => return Ok(()),
    };
    let Ok(identity) = transform.is_identity();
    let mut transformed_paint;
    let style = match identity {
      IsIdentityAnswer::Yes => paint,
      IsIdentityAnswer::No => {
        transformed_paint = paint.clone();
        let Ok(()) = transformed_paint.shader.transform(transform);

        &transformed_paint
      }
    };

    self.paint_path(&region, style, PathPaint::Fill(fill_rule), mask)
  }

  pub fn stroke_path(
    &mut self,
    path: &Path,
    paint: &Paint,
    stroke: &Stroke,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> Result<(), Never> {
    match (stroke.width < 0.0).then_some(()) {
      Some(()) => {
        console_core_logging::warn!("negative stroke width isn't allowed");

        return Ok(());
      }
      None => {}
    }

    let Ok(resolution) = PathStroker::compute_resolution_scale(&transform);
    let dashed;
    let geometry = match stroke.dash {
      Some(ref pattern) => {
        let Ok(candidate) = path.dash(pattern, resolution);
        dashed = match candidate {
          Some(value) => value,
          None => {
            console_core_logging::warn!("path dashing failed");

            return Ok(());
          }
        };

        &dashed
      }
      None => path,
    };
    let Ok(hairline) = treat_as_hairline(paint, stroke, transform);

    match hairline {
      Some(coverage) => self.paint_hairline(
        geometry,
        paint,
        HairlineOptions {
          coverage,
          cap: stroke.line_cap,
        },
        transform,
        mask,
      ),
      None => {
        let Ok(candidate) = geometry.stroke(stroke, resolution);

        match candidate {
          Some(outline) => self.fill_path(&outline, paint, FillRule::Winding, transform, mask),
          None => {
            console_core_logging::warn!("path stroking failed");

            Ok(())
          }
        }
      }
    }
  }

  fn paint_hairline(
    &mut self,
    path: &Path,
    paint: &Paint,
    options: HairlineOptions,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> Result<(), Never> {
    let mut style = paint.clone();
    let Ok(pre_scale) = style.blend_mode.should_pre_scale_coverage();

    match (options.coverage.to_bits() != 1.0f32.to_bits(), pre_scale) {
      (true, crate::ShouldPreScaleCoverageAnswer::Yes) => {
        let Ok(scale) = toward_zero_i32(f64::from(options.coverage * 256.0));
        let new_alpha = 255i32.wrapping_mul(scale).wrapping_shr(8);
        let Ok(alpha) = nearest_f32_from_i64(i64::from(new_alpha));
        let Ok(()) = style.shader.apply_opacity(alpha / 255.0);
      }
      _ => {}
    }

    let Ok(candidate) = transformed_path(path, transform);
    let geometry = match candidate {
      Some(value) => value,
      None => return Ok(()),
    };
    let Ok(identity) = transform.is_identity();

    match identity {
      IsIdentityAnswer::No => {
        let Ok(()) = style.shader.transform(transform);
      }
      IsIdentityAnswer::Yes => {}
    }

    self.paint_path(&geometry, &style, PathPaint::Hairline(options.cap), mask)
  }

  fn paint_path(
    &mut self,
    path: &Path,
    paint: &Paint,
    operation: PathPaint,
    mask: Option<&Mask>,
  ) -> Result<(), Never> {
    let Ok(size) = self.size();
    let Ok(tiler) = DrawTiler::new(size);

    match tiler {
      Some(tiles) => self.paint_tiles(path, paint, operation, mask, tiles),
      None => {
        let Ok(mut canvas) = self.as_subpixmap();
        let submask = mask.map(|mask_2| {
          let Ok(value) = mask_2.as_submask();

          value
        });

        operation.draw(path, paint, submask, &mut canvas)
      }
    }
  }

  fn paint_tiles(
    &mut self,
    path: &Path,
    paint: &Paint,
    operation: PathPaint,
    mask: Option<&Mask>,
    tiles: DrawTiler,
  ) -> Result<(), Never> {
    let mut geometry = path.clone();
    let mut style = paint.clone();

    for tile in tiles {
      let Ok([to_tile, from_tile]) = tile_transforms(&tile);
      let Ok(candidate) = geometry.transform(to_tile);
      let local = match candidate {
        Some(value) => value,
        None => {
          console_core_logging::warn!("path transformation failed");

          return Ok(());
        }
      };
      let Ok(()) = style.shader.transform(to_tile);
      let Ok(bounds) = tile.to_int_rect();
      let Ok(subpixmap) = self.subpixmap(bounds);

      match subpixmap {
        Some(mut surface) => {
          let tile_mask = mask.and_then(|whole| {
            let Ok(mask_2) = whole.submask(bounds);

            mask_2
          });
          let Ok(()) = operation.draw(&local, &style, tile_mask, &mut surface);
        }
        None => {}
      }

      let Ok(restored) = local.transform(from_tile);
      geometry = match restored {
        Some(value) => value,
        None => return Ok(()),
      };
      let Ok(()) = style.shader.transform(from_tile);
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the existing API takes the horizontal then vertical destination coordinate"
    )
  )]
  pub fn draw_pixmap(
    &mut self,
    x: i32,
    y: i32,
    pixmap: PixmapRef,
    paint: &PixmapPaint,
    transform: Transform,
    mask: Option<&Mask>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(size) = pixmap.size();
    let Ok(bounds) = size.to_int_rect(x, y);
    let bounds_3 = match bounds {
      Some(bounds_2) => bounds_2,
      None => return Ok(()),
    };
    let Ok(rect) = bounds_3.to_rect();

    let patt_transform = {
      let Ok(explicit_value) = Transform::from_translation(::console_core_paths::Point {
        x: match ::console_core_number_conversion::nearest_f32_from_i64(
          <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(x),
        ) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
        y: match ::console_core_number_conversion::nearest_f32_from_i64(
          <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(y),
        ) {
          ::core::result::Result::Ok(_converted_number) => _converted_number,
        },
      });
      explicit_value
    };

    const OPAQUE_OPACITY: u32 = 1.0_f32.to_bits();

    let direct = mask.is_none()
      && paint.blend_mode == BlendMode::SourceOver
      && paint.quality == FilterQuality::Nearest
      && paint.opacity.to_bits() == OPAQUE_OPACITY;

    match direct.then_some(()) {
      Some(()) => {
        let Ok(drawn) = crate::nearest_pixmap::draw(self, pixmap, rect, [patt_transform, transform]);

        match drawn {
          crate::nearest_pixmap::Drawing::Drawn => return Ok(()),
          crate::nearest_pixmap::Drawing::Unsupported => {}
        }
      }
      None => {}
    }

    let paint_2 = Paint {
      shader: ({
        let Ok(explicit_value) = Pattern::new(
          pixmap,
          SpreadMode::Pad,
          paint.quality,
          paint.opacity,
          patt_transform,
        );
        explicit_value
      }),
      blend_mode: paint.blend_mode,
      coverage: CoverageMode::Binary,
      precision: PipelinePrecision::Automatic,
      colorspace: ColorSpace::default(),
    };

    ({
      let Ok(explicit_value) = self.fill_rect(rect, &paint_2, transform, mask);
      explicit_value
    });

    Ok(())
  }

  pub fn apply_mask(
    &mut self,
    mask: &Mask,
  ) -> core::result::Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      ({
        let Ok(explicit_value) = self.size();
        explicit_value
      }) != ({
        let Ok(explicit_value) = mask.size();
        explicit_value
      }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        console_core_logging::warn!("Pixmap and Mask are expected to have the same size");
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(source) = PixmapRef::from_bytes(&[0, 0, 0, 0], 1, 1);
    let pixmap_src = match source {
      Some(pixmap) => pixmap,
      None => return Ok(()),
    };

    let mut p = {
      let Ok(explicit_value) = RasterPipelineBuilder::new();
      explicit_value
    };
    ({
      let Ok(explicit_value) = p.push(pipeline::Stage::LoadMaskU8);
      explicit_value
    });
    ({
      let Ok(explicit_value) = p.push(pipeline::Stage::LoadDestination);
      explicit_value
    });
    ({
      let Ok(explicit_value) = p.push(pipeline::Stage::DestinationIn);
      explicit_value
    });
    ({
      let Ok(explicit_value) = p.push(pipeline::Stage::Store);
      explicit_value
    });
    let mut p_2 = {
      let Ok(explicit_value) = p.compile();
      explicit_value
    };
    let Ok(size) = self.size();
    let Ok(bounds) = size.at_origin();
    let rect = match bounds {
      Some(bounds_2) => bounds_2,
      None => return Ok(()),
    };
    ({
      let Ok(explicit_value) = p_2.run(
        &rect,
        pipeline::AAMaskCtx::default(),
        {
          let Ok(explicit_value) = ({
            let Ok(explicit_value) = mask.as_submask();
            explicit_value
          })
          .mask_ctx();
          explicit_value
        },
        pixmap_src,
        &mut ({
          let Ok(explicit_value) = self.as_subpixmap();
          explicit_value
        }),
      );
      explicit_value
    });

    Ok(())
  }
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "coverage is the geometric fraction of a pixel covered by a hairline"
  )
)]
struct HairlineOptions {
  coverage: f32,
  cap: LineCap,
}

enum PathPaint {
  Fill(FillRule),
  Hairline(LineCap),
}

impl PathPaint {
  fn draw(
    &self,
    path: &Path,
    paint: &Paint,
    mask: Option<SubMaskRef>,
    pixmap: &mut SubPixmapMut,
  ) -> Result<(), Never> {
    let Ok(bounds) = pixmap.size.at_origin();
    let clip = match bounds {
      Some(rect) => rect,
      None => return Ok(()),
    };
    let Ok(backend) = RasterPipelineBlitter::new(paint, mask, pixmap);
    let mut blitter = match backend {
      Some(value) => value,
      None => return Ok(()),
    };

    match (self, paint.coverage) {
      (PathPaint::Fill(rule), _) => {
        scan::fill_path(path, *rule, paint.coverage, &clip, &mut blitter)
      }
      (PathPaint::Hairline(cap), CoverageMode::Antialiased) => {
        scan::hairline_aa::stroke_path(path, *cap, &clip, &mut blitter)
      }
      (PathPaint::Hairline(cap), CoverageMode::Binary) => {
        scan::hairline::stroke_path(path, *cap, &clip, &mut blitter)
      }
    }
  }
}

fn transformed_path(path: &Path, transform: Transform) -> Result<Option<Cow<'_, Path>>, Never> {
  let Ok(identity) = transform.is_identity();

  match identity {
    IsIdentityAnswer::Yes => Ok(Some(Cow::Borrowed(path))),
    IsIdentityAnswer::No => {
      let Ok(candidate) = path.clone().transform(transform);

      match candidate {
        Some(value) => Ok(Some(Cow::Owned(value))),
        None => {
          console_core_logging::warn!("path transformation failed");

          Ok(None)
        }
      }
    }
  }
}

pub(crate) fn filled_path(
  path: &Path,
  transform: Transform,
) -> Result<Option<Cow<'_, Path>>, Never> {
  let Ok(candidate) = transformed_path(path, transform);
  let geometry = match candidate {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(bounds) = geometry.bounds();
  let Ok(width) = bounds.width();
  let Ok(height) = bounds.height();

  match (width.is_nearly_zero(), height.is_nearly_zero()) {
    (ScalarProximity::WithinTolerance, _) | (_, ScalarProximity::WithinTolerance) => {
      console_core_logging::warn!("empty paths and horizontal/vertical lines cannot be filled");

      return Ok(None);
    }
    (ScalarProximity::OutsideTolerance, ScalarProximity::OutsideTolerance) => {}
  }

  let Ok(oversized) = is_too_big_for_math(&geometry);

  match oversized {
    crate::IsTooBigForMathAnswer::Yes => {
      console_core_logging::warn!("path coordinates are too big");

      Ok(None)
    }
    crate::IsTooBigForMathAnswer::No => Ok(Some(geometry)),
  }
}

pub(crate) fn tile_transforms(tile: &ScreenIntRect) -> Result<[Transform; 2], Never> {
  let Ok(column) = tile.x();
  let Ok(row) = tile.y();
  let Ok(x) = nearest_f32_from_u64(u64::from(column));
  let Ok(y) = nearest_f32_from_u64(u64::from(row));
  let Ok(to_tile) = Transform::from_translation(Point { x: -x, y: -y });
  let Ok(from_tile) = Transform::from_translation(Point { x, y });

  Ok([to_tile, from_tile])
}

fn treat_as_hairline(
  paint: &Paint,
  stroke: &Stroke,
  mut ts: Transform,
) -> core::result::Result<Option<f32>, console_core_never::Never> {
  fn fast_len(p: Point) -> core::result::Result<f32, console_core_never::Never> {
    Ok({
      let mut x = p.x.abs();
      let mut y = p.y.abs();

      match ::core::primitive::bool::then_some(x < y, ()) {
        ::core::option::Option::Some(()) => {
          core::mem::swap(&mut x, &mut y);
        }
        ::core::option::Option::None => {}
      }

      x + y.half()
    })
  }

  match ::core::primitive::f32::classify(stroke.width) {
    ::core::num::FpCategory::Zero => {
      return Ok(Some(1.0));
    }
    ::core::num::FpCategory::Nan
    | ::core::num::FpCategory::Infinite
    | ::core::num::FpCategory::Subnormal
    | ::core::num::FpCategory::Normal => {}
  }

  match paint.coverage {
    CoverageMode::Binary => {
      return Ok(None);
    }
    CoverageMode::Antialiased => {}
  }

  ts.tx = 0.0;
  ts.ty = 0.0;

  let mut points = [
    ({
      let Ok(explicit_value) = Point::from_xy(stroke.width, 0.0);
      explicit_value
    }),
    ({
      let Ok(explicit_value) = Point::from_xy(0.0, stroke.width);
      explicit_value
    }),
  ];
  ({
    let Ok(explicit_value) = ts.map_points(&mut points);
    explicit_value
  });

  let len0 = {
    let Ok(explicit_value) = fast_len(match points {
      [_explicit_element, ..] => _explicit_element,
    });
    explicit_value
  };
  let len1 = {
    let Ok(explicit_value) = fast_len(match points {
      [_, _explicit_element, ..] => _explicit_element,
    });
    explicit_value
  };

  match ::core::primitive::bool::then_some(len0 <= 1.0 && len1 <= 1.0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Some(len0.ave(len1)));
    }
    ::core::option::Option::None => {}
  }

  Ok(None)
}

pub(crate) fn is_too_big_for_math(
  path: &Path,
) -> core::result::Result<crate::IsTooBigForMathAnswer, console_core_never::Never> {
  let decided = {
    const SCALE_DOWN_TO_ALLOW_FOR_SMALL_MULTIPLIES: f32 = 0.25;
    const MAX: f32 = SCALAR_MAX * SCALE_DOWN_TO_ALLOW_FOR_SMALL_MULTIPLIES;

    let b = {
      let Ok(explicit_value) = path.bounds();
      explicit_value
    };

    !(({
      let Ok(explicit_value) = b.left();
      explicit_value
    }) >= -MAX
      && ({
        let Ok(explicit_value) = b.top();
        explicit_value
      }) >= -MAX
      && ({
        let Ok(explicit_value) = b.right();
        explicit_value
      }) <= MAX
      && ({
        let Ok(explicit_value) = b.bottom();
        explicit_value
      }) <= MAX)
  };

  Ok(match ::core::primitive::bool::then_some(decided, ()) {
    ::core::option::Option::Some(()) => crate::IsTooBigForMathAnswer::Yes,
    ::core::option::Option::None => crate::IsTooBigForMathAnswer::No,
  })
}

pub(crate) struct DrawTiler {
  extent: Size<u32>,
  next: Option<PixelPosition>,
}

impl DrawTiler {
  const MAX_DIMENSIONS: u32 = 8192 - 1;

  pub(crate) fn required(size: IntSize) -> Result<crate::RequiredAnswer, Never> {
    let Ok((width, height)) = size.dimensions();

    Ok(
      match (width > Self::MAX_DIMENSIONS || height > Self::MAX_DIMENSIONS).then_some(()) {
        Some(()) => crate::RequiredAnswer::Yes,
        None => crate::RequiredAnswer::No,
      },
    )
  }

  pub(crate) fn new(size: IntSize) -> Result<Option<Self>, Never> {
    let Ok(required) = Self::required(size);
    let Ok((width, height)) = size.dimensions();

    Ok(match required {
      crate::RequiredAnswer::Yes => Some(Self {
        extent: Size { width, height },
        next: Some(PixelPosition { x: 0, y: 0 }),
      }),
      crate::RequiredAnswer::No => None,
    })
  }
}

impl Iterator for DrawTiler {
  type Item = ScreenIntRect;

  fn next(&mut self) -> Option<Self::Item> {
    let position = self.next.take()?;
    let width = self
      .extent
      .width
      .saturating_sub(position.x)
      .min(Self::MAX_DIMENSIONS);
    let height = self
      .extent
      .height
      .saturating_sub(position.y)
      .min(Self::MAX_DIMENSIONS);
    let right = position.x.saturating_add(width);
    let bottom = position.y.saturating_add(height);
    let Ok(tile) = ScreenIntRect::new(position, Size { width, height });
    self.next = match (right < self.extent.width, bottom < self.extent.height, tile) {
      (true, _, Some(_)) => Some(PixelPosition {
        x: right,
        y: position.y,
      }),
      (false, true, Some(_)) => Some(PixelPosition { x: 0, y: bottom }),
      _ => None,
    };

    tile
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  const MAX_DIM: u32 = DrawTiler::MAX_DIMENSIONS;
  const REST: u32 = 10000 - MAX_DIM;

  fn tiler(
    TilerArguments {
      image_width,
      image_height,
    }: TilerArguments,
  ) -> Result<DrawTiler, TestFailure> {
    let Ok(size) = IntSize::from_wh(image_width, image_height);
    let checked_value = size.required()?;
    let Ok(made) = DrawTiler::new(checked_value);

    made.required()
  }

  struct TilerArguments {
    image_width: u32,
    image_height: u32,
  }

  #[test]
  fn skip() -> Result<(), TestFailure> {
    let Ok(size) = IntSize::from_wh(100, 500);
    let checked_value = size.required()?;
    let Ok(tiler) = DrawTiler::new(checked_value);
    assert!(tiler.is_none());

    Ok(())
  }

  #[test]
  fn horizontal() -> Result<(), TestFailure> {
    let mut iter = tiler(self::TilerArguments {
      image_width: 10000,
      image_height: 500,
    })?;
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: 0, y: 0 },
        Size {
          width: MAX_DIM,
          height: 500
        }
      )
    );
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: MAX_DIM, y: 0 },
        Size {
          width: REST,
          height: 500
        }
      )
    );
    assert_eq!(iter.next(), None);

    Ok(())
  }

  #[test]
  fn vertical() -> Result<(), TestFailure> {
    let mut iter = tiler(self::TilerArguments {
      image_width: 500,
      image_height: 10000,
    })?;
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: 0, y: 0 },
        Size {
          width: 500,
          height: MAX_DIM
        }
      )
    );
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: 0, y: MAX_DIM },
        Size {
          width: 500,
          height: REST
        }
      )
    );
    assert_eq!(iter.next(), None);

    Ok(())
  }

  #[test]
  fn rect() -> Result<(), TestFailure> {
    let mut iter = tiler(self::TilerArguments {
      image_width: 10000,
      image_height: 10000,
    })?;
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: 0, y: 0 },
        Size {
          width: MAX_DIM,
          height: MAX_DIM
        }
      )
    );
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: MAX_DIM, y: 0 },
        Size {
          width: REST,
          height: MAX_DIM
        }
      )
    );
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition { x: 0, y: MAX_DIM },
        Size {
          width: MAX_DIM,
          height: REST
        }
      )
    );
    assert_eq!(
      Ok(iter.next()),
      ScreenIntRect::new(
        crate::geom::PixelPosition {
          x: MAX_DIM,
          y: MAX_DIM
        },
        Size {
          width: REST,
          height: REST
        }
      )
    );
    assert_eq!(iter.next(), None);

    Ok(())
  }
}
