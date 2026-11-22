//! PDF patterns.

use crate::cache::Cache;
use crate::color::{Color, ColorSpace};
use crate::context::{Context, InterpreterCache};
use crate::device::Device;
use crate::font::Glyph;
use crate::interpret::state::{ActiveTransferFunction, Opacity, State};
use crate::shading::Shading;
use crate::soft_mask::SoftMask;
use crate::util::{Nearness, RectExt, hash128};
use crate::{BlendMode, CacheKey, ClipPath, GlyphDrawMode, Image, PaintOperation, PathDrawMode};
use crate::{FillRule, InterpreterSettings, Paint, interpret};
use console_core_never::Never;
use console_core_portable_document_syntax::ContainsKeyAnswer;
use console_core_portable_document_syntax::content::TypedIter;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::{
  BBOX, EXT_G_STATE, MATRIX, PAINT_TYPE, RESOURCES, SHADING, X_STEP, Y_STEP,
};
use console_core_portable_document_syntax::object::{Object, dict_or_stream};
use console_core_portable_document_syntax::page::Resources;
use console_core_portable_document_syntax::xref::XRef;
use kurbo::{Affine, BezPath, Rect, Shape};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Pattern<'a> {
  Shading(ShadingPattern),

  Tiling(Box<TilingPattern<'a>>),
}

impl<'a> Pattern<'a> {
  pub(crate) fn new(
    object: Object<'a>,
    ctx: &Context<'a>,
    resources: &Resources<'a>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match object {
      Object::Dict(dict) => {
        let Ok(shading_pattern) = ShadingPattern::new(
          &dict,
          &ctx.interpreter_cache.object_cache,
          ({
            let Ok(explicit_value) = ctx.get();
            explicit_value
          })
          .graphics_state
          .non_stroke_alpha,
        );

        Some(Self::Shading(match shading_pattern {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        }))
      }
      Object::Stream(stream) => {
        let Ok(tiling_pattern) = TilingPattern::new(stream, ctx, resources);

        Some(Self::Tiling(Box::new(match tiling_pattern {
          Some(explicit_value) => explicit_value,
          None => return Ok(None),
        })))
      }
      Object::Null(_)
      | Object::Boolean(_)
      | Object::Number(_)
      | Object::Name(_)
      | Object::String(_)
      | Object::Array(_) => None,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Each pattern transforms an owned clip via copy on write while sharing the unchanged cached geometry"
    )
  )]
  pub(crate) fn pre_concat_transform(
    &mut self,
    transform: Affine,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self {
      Self::Shading(p) => {
        p.matrix = transform * p.matrix;
        let transformed_clip_path = p.shading.clip_path.clone().map(|r| p.matrix * r);
        Arc::make_mut(&mut p.shading).clip_path = transformed_clip_path;
      }
      Self::Tiling(p) => p.matrix = transform * p.matrix,
    };

    Ok(())
  }

  pub(crate) fn set_transfer_function(
    &mut self,
    tf: ActiveTransferFunction,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self {
      Self::Shading(pattern) => pattern.transfer_function = Some(tf),
      Self::Tiling(_) => {}
    };

    Ok(())
  }
}

impl CacheKey for Pattern<'_> {
  fn cache_key(&self) -> u128 {
    match self {
      Self::Shading(p) => p.cache_key(),
      Self::Tiling(p) => p.cache_key(),
    }
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The existing public pattern opacity is a normalized float consumed directly by rendering devices"
  )
)]
pub struct ShadingPattern {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public shading field lets independently transformed device patterns share immutable geometry"
    )
  )]
  pub shading: Arc<Shading>,

  pub matrix: Affine,

  pub opacity: f32,

  pub transfer_function: Option<ActiveTransferFunction>,
}

impl ShadingPattern {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "This constructor initializes the public shading handle for cloned device patterns"
    )
  )]
  pub(crate) fn from_shading(
    shading: Shading,
    matrix: Affine,
    Opacity(opacity): Opacity,
    transfer_function: Option<ActiveTransferFunction>,
  ) -> Result<Self, Never> {
    Ok(Self {
      shading: Arc::new(shading),
      opacity,
      matrix,
      transfer_function,
    })
  }

  pub(crate) fn new(
    dict: &Dict<'_>,
    cache: &Cache,
    opacity: Opacity,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let shading = match ({
      let Ok(explicit_value) = dict.get::<Object<'_>>(SHADING);
      explicit_value
    })
    .and_then(|o| {
      let (dict_2, stream) = ({
        let Ok(explicit_value) = dict_or_stream(&o);
        explicit_value
      })?;

      {
        let Ok(explicit_value) = Shading::new(dict_2, stream, cache);
        explicit_value
      }
    }) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let matrix = match ({
      let Ok(explicit_value) = dict.get::<[f64; 6]>(MATRIX);
      explicit_value
    })
    .map(Affine::new)
    {
      Some(present_value) => present_value,
      None => Default::default(),
    };

    let Ok(contains_key) = dict.contains_key(EXT_G_STATE);

    match contains_key {
      ContainsKeyAnswer::Yes => warn!("shading patterns with ext_g_state are not supported yet"),
      ContainsKeyAnswer::No => {}
    }

    Self::from_shading(shading, matrix, opacity, None).map(Some)
  }
}

impl CacheKey for ShadingPattern {
  fn cache_key(&self) -> u128 {
    {
      let Ok(explicit_value) = hash128(&(self.shading.cache_key(), self.matrix.cache_key()));
      explicit_value
    }
  }
}

struct TilingGeometry {
  bbox: Rect,
  steps: [f32; 2],
  matrix: Affine,
}

fn tiling_geometry(dict: &Dict<'_>) -> Result<Option<TilingGeometry>, Never> {
  let Ok(bounds) = dict.get::<console_core_portable_document_syntax::object::Rect>(BBOX);
  let Ok(step_across) = dict.get::<f32>(X_STEP);
  let Ok(step_down) = dict.get::<f32>(Y_STEP);
  let (bbox, x_step, y_step) = match (bounds, step_across, step_down) {
    (Some(bounds_2), Some(x_step), Some(y_step)) => (bounds_2.to_kurbo(), x_step, y_step),
    (None, _, _) | (_, None, _) | (_, _, None) => return Ok(None),
  };
  let degenerate = matches!(x_step.nearly(0.0), Ok(crate::util::Nearly::Equal))
    || matches!(y_step.nearly(0.0), Ok(crate::util::Nearly::Equal))
    || bbox.is_zero_area();

  match degenerate.then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let Ok(transform) = dict.get::<[f64; 6]>(MATRIX);
  let matrix = match transform {
    Some(values) => Affine::new(values),
    None => Affine::default(),
  };

  Ok(Some(TilingGeometry {
    bbox,
    steps: [x_step, y_step],
    matrix,
  }))
}

fn base_pattern_color(space: &ColorSpace) -> Result<ColorSpace, Never> {
  let Ok(base) = space.pattern_cs();

  match base {
    Some(color) => Ok(color),
    None => ColorSpace::device_gray(),
  }
}

#[derive(Clone, Copy)]
enum PatternColor {
  Colored,
  Uncolored,
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The existing public x_step and y_step fields represent signed distances in PDF pattern space"
  )
)]
pub struct TilingPattern<'a> {
  cache_key: u128,
  ctx_bbox: Rect,

  pub bbox: Rect,

  pub x_step: f32,

  pub y_step: f32,

  pub matrix: Affine,
  stream: Stream<'a>,
  color: PatternColor,
  pub(crate) stroke_paint: Color,
  pub(crate) non_stroking_paint: Color,
  pub(crate) parent_resources: Resources<'a>,
  pub(crate) cache: InterpreterCache<'a>,
  pub(crate) settings: InterpreterSettings,
  pub(crate) xref: &'a XRef,
  nesting_depth: u32,
}

impl Debug for TilingPattern<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str("TilingPattern")
  }
}

impl<'a> TilingPattern<'a> {
  pub(crate) fn new(
    stream: Stream<'a>,
    ctx: &Context<'a>,
    resources: &Resources<'a>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let cache_key = stream.cache_key();
    let Ok(dict) = stream.dict();

    let Ok(geometry) = tiling_geometry(dict);
    let TilingGeometry {
      bbox,
      steps: [x_step, y_step],
      matrix,
    } = match geometry {
      Some(values) => values,
      None => return Ok(None),
    };
    let Ok(paint_type) = dict.get::<u8>(PAINT_TYPE);
    let color = match paint_type {
      Some(1) => PatternColor::Colored,
      Some(_uncolored) => PatternColor::Uncolored,
      None => return Ok(None),
    };
    let Ok(state) = ctx.get();
    let Ok(ctx_bbox) = ctx.bbox();
    let Ok(fill_cs) = base_pattern_color(&state.graphics_state.none_stroke_cs);
    let Ok(stroke_cs) = base_pattern_color(&state.graphics_state.stroke_cs);
    let Ok(non_stroking_paint) = Color::new(
      fill_cs,
      state.graphics_state.non_stroke_color.clone(),
      state.graphics_state.non_stroke_alpha,
    );
    let Ok(stroke_paint) = Color::new(
      stroke_cs,
      state.graphics_state.stroke_color.clone(),
      state.graphics_state.stroke_alpha,
    );
    let Ok(ctx_nesting_depth) = ctx.nesting_depth();
    let nesting_depth = u32::wrapping_add(ctx_nesting_depth, 1);

    Ok(Some(Self {
      cache_key,
      bbox,
      x_step,
      y_step,
      matrix,
      ctx_bbox,
      color,
      stream,
      stroke_paint,
      non_stroking_paint,
      settings: ctx.settings.clone(),
      parent_resources: resources.clone(),
      cache: ctx.interpreter_cache.clone(),
      xref: ctx.xref,
      nesting_depth,
    }))
  }

  pub fn interpret(
    &self,
    device: &mut impl Device<'a>,
    initial_transform: Affine,
    operation: PaintOperation,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      self.nesting_depth > crate::context::MAX_PAINT_NESTING
        || !matches!(
          {
            let Ok(explicit_value) = crate::context::take_nested_paint(self.nesting_depth);
            explicit_value
          },
          crate::PaintBudget::Granted
        ),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        warn!("tiling pattern nesting depth or paint budget exceeded");
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(state) = State::new(initial_transform);

    let Ok(mut context) = Context::new_with(
      state.ctm,
      (initial_transform * self.ctx_bbox.to_path(0.1)).bounding_box(),
      &self.cache,
      self.xref,
      self.settings.clone(),
      state,
      self.nesting_depth,
    );

    let decoded = match match self.stream.decoded() {
      Ok(parsed_value) => Some(parsed_value),
      Err(_invalid_stream_encoding) => None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(dict) = self.stream.dict();
    let Ok(own_dict) = dict.get(RESOURCES);
    let Ok(resources) = Resources::from_parent(
      match own_dict {
        Some(present_value) => present_value,
        None => Default::default(),
      },
      self.parent_resources.clone(),
    );
    let Ok(iter) = TypedIter::new(decoded.as_ref());

    let clip_path = ClipPath {
      path: initial_transform * self.bbox.to_path(0.1),
      fill: FillRule::NonZero,
    };
    device.push_clip_path(&clip_path);

    match self.color {
      PatternColor::Colored => {
        let Ok(()) = interpret(iter, &resources, &mut context, device);
      }
      PatternColor::Uncolored => {
        let paint = match operation {
          PaintOperation::Fill => Paint::Color(self.non_stroking_paint.clone()),
          PaintOperation::Stroke => Paint::Color(self.stroke_paint.clone()),
        };

        let Ok(mut device_2) = StencilPatternDevice::new(device, paint.clone());
        let Ok(()) = interpret(iter, &resources, &mut context, &mut device_2);
      }
    }

    device.pop_clip_path();

    Ok(Some(()))
  }
}

impl CacheKey for TilingPattern<'_> {
  fn cache_key(&self) -> u128 {
    self.cache_key
  }
}

struct StencilPatternDevice<'a, 'b, T: Device<'a>> {
  inner: &'b mut T,
  paint: Paint<'a>,
}

impl<'a, 'b, T: Device<'a>> StencilPatternDevice<'a, 'b, T> {
  pub(crate) fn new(
    device: &'b mut T,
    paint: Paint<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      inner: device,
      paint,
    })
  }
}

impl<'a, T: Device<'a>> Device<'a> for StencilPatternDevice<'a, '_, T> {
  fn draw_path(
    &mut self,
    path: &BezPath,
    transform: Affine,
    _: &Paint<'_>,
    draw_mode: &PathDrawMode,
  ) {
    self
      .inner
      .draw_path(path, transform, &self.paint, draw_mode);
  }

  fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}

  fn push_clip_path(&mut self, clip_path: &ClipPath) {
    self.inner.push_clip_path(clip_path);
  }

  fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}

  fn draw_glyph(
    &mut self,
    g: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    p: &Paint<'a>,
    draw_mode: &GlyphDrawMode,
  ) {
    self
      .inner
      .draw_glyph(g, transform, glyph_transform, p, draw_mode);
  }

  fn draw_image(&mut self, image: Image<'a, '_>, transform: Affine) {
    match image {
      Image::Stencil(mut stencil) => {
        stencil.paint = self.paint.clone();
        self.inner.draw_image(Image::Stencil(stencil), transform);
      }
      Image::Raster(_) => {}
    }
  }

  fn pop_clip_path(&mut self) {
    self.inner.pop_clip_path();
  }

  fn pop_transparency_group(&mut self) {}

  fn set_blend_mode(&mut self, _: BlendMode) {}
}
