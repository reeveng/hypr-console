use crate::CacheKey;
use crate::color::Color;
use crate::pattern::Pattern;
use crate::util::hash128;
use crate::x_object::ImageXObject;
use console_core_portable_document_syntax::object::Stream;
use kurbo::{BezPath, Cap, Join};

#[derive(Debug, Clone)]
pub struct ClipPath {
  pub path: BezPath,

  pub fill: FillRule,
}

impl CacheKey for ClipPath {
  fn cache_key(&self) -> u128 {
    {
      let Ok(explicit_value) = hash128(&(&self.path.to_svg(), &self.fill));
      explicit_value
    }
  }
}

pub struct StencilImage<'a, 'b> {
  pub(crate) paint: Paint<'a>,
  pub(crate) image_xobject: ImageXObject<'b>,
}

impl<'a, 'b> StencilImage<'a, 'b> {
  pub fn with_stencil(
    &self,
    func: impl FnOnce(LumaData, &Paint<'a>),
    target_dimension: Option<(u32, u32)>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(decoded) = self.decoded_stencil(target_dimension);

    match decoded {
      Some((samples, paint)) => func(samples, paint),
      None => {}
    }

    Ok(())
  }

  #[doc = "Decode the stencil and borrow the paint applied through it."]
  pub fn decoded_stencil(
    &self,
    target_dimension: Option<(u32, u32)>,
  ) -> Result<Option<(LumaData, &Paint<'a>)>, console_core_never::Never> {
    let Ok(decoded) = self.image_xobject.decoded_mask(target_dimension);

    Ok(decoded.map(|mask| (mask.luma, &self.paint)))
  }

  #[doc(hidden)]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(width) = self.image_xobject.width();

    Ok(width)
  }

  #[doc(hidden)]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(height) = self.image_xobject.height();

    Ok(height)
  }
}

impl CacheKey for StencilImage<'_, '_> {
  fn cache_key(&self) -> u128 {
    self.image_xobject.cache_key()
  }
}

pub struct RasterImage<'a>(pub(crate) ImageXObject<'a>);

impl RasterImage<'_> {
  pub fn with_rgba(
    &self,
    func: impl FnOnce(ImageData, Option<LumaData>),
    target_dimension: Option<(u32, u32)>,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(decoded) = self.decoded_rgba(target_dimension);

    match decoded {
      Some((image, alpha)) => func(image, alpha),
      None => {}
    }

    Ok(())
  }

  #[doc = "Decode the raster samples and any separate alpha mask."]
  pub fn decoded_rgba(
    &self,
    target_dimension: Option<(u32, u32)>,
  ) -> Result<Option<(ImageData, Option<LumaData>)>, console_core_never::Never> {
    let Ok(decoded) = self.0.decoded_raster(target_dimension);

    Ok(decoded.map(|samples| (samples.image, samples.alpha)))
  }

  pub fn stream(&self) -> core::result::Result<&Stream<'_>, console_core_never::Never> {
    let Ok(stream) = self.0.stream();

    Ok(stream)
  }

  #[doc(hidden)]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(width) = self.0.width();

    Ok(width)
  }

  #[doc(hidden)]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(height) = self.0.height();

    Ok(height)
  }
}

impl CacheKey for RasterImage<'_> {
  fn cache_key(&self) -> u128 {
    self.0.cache_key()
  }
}

pub enum Image<'a, 'b> {
  Stencil(StencilImage<'a, 'b>),

  Raster(RasterImage<'b>),
}

impl Image<'_, '_> {
  #[doc(hidden)]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(match self {
      Image::Stencil(s) => {
        let Ok(width) = s.width();

        width
      }
      Image::Raster(r) => {
        let Ok(width) = r.width();

        width
      }
    })
  }

  #[doc(hidden)]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(match self {
      Image::Stencil(s) => {
        let Ok(height) = s.height();

        height
      }
      Image::Raster(r) => {
        let Ok(height) = r.height();

        height
      }
    })
  }
}

impl CacheKey for Image<'_, '_> {
  fn cache_key(&self) -> u128 {
    match self {
      Image::Stencil(i) => i.cache_key(),
      Image::Raster(i) => i.cache_key(),
    }
  }
}

#[derive(Clone)]
pub struct RgbData {
  pub data: Vec<u8>,

  pub width: u32,

  pub height: u32,

  pub interpolation: crate::Interpolation,

  pub scale_factors: (f32, f32),
}

#[derive(Clone)]
pub struct LumaData {
  pub data: Vec<u8>,

  pub width: u32,

  pub height: u32,

  pub interpolation: crate::Interpolation,

  pub scale_factors: (f32, f32),
}

#[derive(Clone)]
pub enum ImageData {
  Rgb(RgbData),

  Luma(LumaData),
}

impl ImageData {
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(match self {
      Self::Rgb(d) => d.width,
      Self::Luma(d) => d.width,
    })
  }

  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(match self {
      Self::Rgb(d) => d.height,
      Self::Luma(d) => d.height,
    })
  }

  pub fn interpolate(
    &self,
  ) -> core::result::Result<crate::Interpolation, console_core_never::Never> {
    Ok(match self {
      Self::Rgb(data) => data.interpolation,
      Self::Luma(data) => data.interpolation,
    })
  }

  pub fn scale_factors(&self) -> core::result::Result<(f32, f32), console_core_never::Never> {
    Ok(match self {
      Self::Rgb(d) => d.scale_factors,
      Self::Luma(d) => d.scale_factors,
    })
  }
}

#[derive(Clone, Debug)]
pub enum Paint<'a> {
  Color(Color),

  Pattern(Box<Pattern<'a>>),
}

impl CacheKey for Paint<'_> {
  fn cache_key(&self) -> u128 {
    match self {
      Paint::Color(c) => {
        let Ok(rgba) = c.to_rgba();
        let Ok(rgba8) = rgba.to_rgba8();

        {
          let Ok(explicit_value) = hash128(&rgba8);
          explicit_value
        }
      }
      Paint::Pattern(p) => p.cache_key(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum PathDrawMode {
  Fill(FillRule),

  Stroke(StrokeProps),
}

#[derive(Clone, Debug)]
pub enum GlyphDrawMode {
  Fill,

  Stroke(StrokeProps),

  Invisible,
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "drawing devices receive PDF line widths and dash distances in user units and a dimensionless miter ratio"
  )
)]
pub struct StrokeProps {
  pub line_width: f32,

  pub line_cap: Cap,

  pub line_join: Join,

  pub miter_limit: f32,

  pub dash_array: Vec<f32>,

  pub dash_offset: f32,
}

impl Default for StrokeProps {
  fn default() -> Self {
    Self {
      line_width: 1.0,
      line_cap: Cap::Butt,
      line_join: Join::Miter,
      miter_limit: 10.0,
      dash_array: vec![],
      dash_offset: 0.0,
    }
  }
}

#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq)]
pub enum FillRule {
  NonZero,

  EvenOdd,
}

#[derive(Clone, Debug, Copy, Hash, PartialEq, Eq, Default)]
pub enum BlendMode {
  #[default]
  Normal,

  Multiply,

  Screen,

  Overlay,

  Darken,

  Lighten,

  ColorDodge,

  ColorBurn,

  HardLight,

  SoftLight,

  Difference,

  Exclusion,

  Hue,

  Saturation,

  Color,

  Luminosity,
}
