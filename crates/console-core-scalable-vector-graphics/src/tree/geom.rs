use console_core_never::Never;
use console_core_paths::Point;
pub use console_core_paths::{FloatRectangle, NonZeroRect, Size, Transform};
use console_core_scalable_vector_graphics_values::{Align, AspectRatio};
use float_cmp::ApproxEqUlps;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroProximity {
  NearZero,
  Nonzero,
}

impl From<bool> for ZeroProximity {
  fn from(near_zero: bool) -> Self {
    match near_zero.then_some(()) {
      Some(()) => Self::NearZero,
      None => Self::Nonzero,
    }
  }
}

pub trait ApproxZeroUlps: ApproxEqUlps {
  fn zero_proximity(&self, ulps: <Self::Flt as float_cmp::Ulps>::U) -> ZeroProximity;
}

impl ApproxZeroUlps for f32 {
  fn zero_proximity(&self, ulps: i32) -> ZeroProximity {
    ZeroProximity::from(self.approx_eq_ulps(&0.0, ulps))
  }
}

impl ApproxZeroUlps for f64 {
  fn zero_proximity(&self, ulps: i64) -> ZeroProximity {
    ZeroProximity::from(self.approx_eq_ulps(&0.0, ulps))
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LengthValidity {
  Valid,
  Invalid,
}

impl From<bool> for LengthValidity {
  fn from(valid: bool) -> Self {
    match valid.then_some(()) {
      Some(()) => Self::Valid,
      None => Self::Invalid,
    }
  }
}

pub(crate) trait IsValidLength {
  fn length_validity(&self) -> LengthValidity;
}

impl IsValidLength for f32 {
  #[inline]
  fn length_validity(&self) -> LengthValidity {
    LengthValidity::from(*self > 0.0 && self.is_finite())
  }
}

impl IsValidLength for f64 {
  #[inline]
  fn length_validity(&self) -> LengthValidity {
    LengthValidity::from(*self > 0.0 && self.is_finite())
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewBox {
  pub rect: NonZeroRect,

  pub aspect: AspectRatio,
}

impl ViewBox {
  pub fn to_transform(
    self,
    img_size: Size,
  ) -> core::result::Result<Transform, console_core_never::Never> {
    let vr = self.rect;

    let Ok(width) = img_size.width();
    let Ok(vr_width) = vr.width();
    let sx = width / vr_width;
    let Ok(height) = img_size.height();
    let Ok(vr_height) = vr.height();
    let sy = height / vr_height;

    let (sx_2, sy_2) =
      match ::core::primitive::bool::then_some(self.aspect.align == Align::None, ()) {
        ::core::option::Option::Some(()) => (sx, sy),
        ::core::option::Option::None => {
          let s = match ::core::primitive::bool::then_some(self.aspect.slice, ()) {
            ::core::option::Option::Some(()) => {
              match ::core::primitive::bool::then_some(sx < sy, ()) {
                ::core::option::Option::Some(()) => sy,
                ::core::option::Option::None => sx,
              }
            }
            ::core::option::Option::None => match ::core::primitive::bool::then_some(sx > sy, ())
            {
              ::core::option::Option::Some(()) => sy,
              ::core::option::Option::None => sx,
            },
          };

          (s, s)
        }
      };

    let x = -({
      let Ok(explicit_value) = vr.x();
      explicit_value
    }) * sx_2;
    let y = -({
      let Ok(explicit_value) = vr.y();
      explicit_value
    }) * sy_2;
    let Ok(img_size_width) = img_size.width();
    let w = img_size_width
      - ({
        let Ok(explicit_value) = vr.width();
        explicit_value
      }) * sx_2;
    let Ok(img_size_height) = img_size.height();
    let h = img_size_height
      - ({
        let Ok(explicit_value) = vr.height();
        explicit_value
      }) * sy_2;

    let Ok(Point { x: tx, y: ty }) =
      aligned_pos(self.aspect.align, Point { x, y }, Point { x: w, y: h });

    let Ok(transform) = Transform::from_row(sx_2, 0.0, 0.0, sy_2, tx, ty);

    Ok(transform)
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BBox {
  minimum: Point,
  maximum: Point,
}

impl From<FloatRectangle> for BBox {
  fn from(rect: FloatRectangle) -> Self {
    let Ok(left) = rect.left();
    let Ok(top) = rect.top();
    let Ok(right) = rect.right();
    let Ok(bottom) = rect.bottom();

    Self {
      minimum: Point { x: left, y: top },
      maximum: Point {
        x: right,
        y: bottom,
      },
    }
  }
}

impl From<NonZeroRect> for BBox {
  fn from(rect: NonZeroRect) -> Self {
    let Ok(bounding_rect) = rect.to_rect();

    Self::from(bounding_rect)
  }
}

impl Default for BBox {
  fn default() -> Self {
    Self {
      minimum: Point {
        x: f32::MAX,
        y: f32::MAX,
      },
      maximum: Point {
        x: f32::MIN,
        y: f32::MIN,
      },
    }
  }
}

impl BBox {
  pub fn is_default(&self) -> Result<crate::IsDefaultAnswer, Never> {
    let max = f32::MAX.to_bits();
    let min = f32::MIN.to_bits();

    Ok(
      match ::core::primitive::bool::then_some(
        self.minimum.x.to_bits() == max
          && self.minimum.y.to_bits() == max
          && self.maximum.x.to_bits() == min
          && self.maximum.y.to_bits() == min,
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsDefaultAnswer::Yes,
        ::core::option::Option::None => crate::IsDefaultAnswer::No,
      },
    )
  }

  pub fn expand(&self, region: impl Into<Self>) -> Result<Self, Never> {
    let region_2 = <_ as ::core::convert::Into<crate::tree::geom::BBox>>::into(region);

    Ok(Self {
      minimum: Point {
        x: self.minimum.x.min(region_2.minimum.x),
        y: self.minimum.y.min(region_2.minimum.y),
      },
      maximum: Point {
        x: self.maximum.x.max(region_2.maximum.x),
        y: self.maximum.y.max(region_2.maximum.y),
      },
    })
  }

  pub fn to_rect(self) -> Result<Option<FloatRectangle>, Never> {
    match self.is_default() {
      Ok(crate::IsDefaultAnswer::Yes) => Ok(None),
      Ok(crate::IsDefaultAnswer::No) => FloatRectangle::from_ltrb(
        self.minimum.x,
        self.minimum.y,
        self.maximum.x,
        self.maximum.y,
      ),
    }
  }

  pub fn to_non_zero_rect(self) -> Result<Option<NonZeroRect>, Never> {
    match self.is_default() {
      Ok(crate::IsDefaultAnswer::Yes) => Ok(None),
      Ok(crate::IsDefaultAnswer::No) => NonZeroRect::from_ltrb(
        self.minimum.x,
        self.minimum.y,
        self.maximum.x,
        self.maximum.y,
      ),
    }
  }
}

pub(crate) fn aligned_pos(align: Align, origin: Point, spare: Point) -> Result<Point, Never> {
  let Point { x, y } = origin;
  let Point {
    x: width,
    y: height,
  } = spare;
  let (aligned_x, aligned_y) = match align {
    Align::None | Align::XMinYMin => (x, y),
    Align::XMidYMin => (x + width / 2.0, y),
    Align::XMaxYMin => (x + width, y),
    Align::XMinYMid => (x, y + height / 2.0),
    Align::XMidYMid => (x + width / 2.0, y + height / 2.0),
    Align::XMaxYMid => (x + width, y + height / 2.0),
    Align::XMinYMax => (x, y + height),
    Align::XMidYMax => (x + width / 2.0, y + height),
    Align::XMaxYMax => (x + width, y + height),
  };

  Ok(Point {
    x: aligned_x,
    y: aligned_y,
  })
}
