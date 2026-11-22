//! The small questions every other module asks: how near two numbers are, what a value hashes
//! to, and the box around a path.
//!
//! Nearness is one answer rather than four predicates. Two numbers within a 256th of each other
//! are `Equal`, otherwise one is `Below` or `Above` the other, and a NaN is `Unordered`, so a
//! caller that asked "at or below" and one that asked "at or above" both hear no for it, as the
//! predicates they replace did.

use console_core_never::Never;
use console_core_number_conversion::narrow_f32;
use kurbo::{BezPath, PathEl, Point, Rect};
use siphasher::sip128::{Hasher128, SipHasher13};
use std::hash::Hash;

pub(crate) trait OptionLog {
  fn warn_none(self, said: &str) -> Self;
}

impl<T> OptionLog for Option<T> {
  #[inline]
  fn warn_none(self, _said: &str) -> Self {
    self.or_else(|| {
      warn!("{_said}");

      None
    })
  }
}

const SCALAR_NEARLY_ZERO: f32 = 1.0 / 256.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nearly {
  Below,
  Equal,
  Above,
  Unordered,
}

pub trait Nearness: Copy {
  fn nearly(self, other: Self) -> Result<Nearly, Never>;
}

impl Nearness for f32 {
  fn nearly(self, other: Self) -> Result<Nearly, Never> {
    let apart = self - other;

    Ok(
      match (apart.abs() <= SCALAR_NEARLY_ZERO, self.partial_cmp(&other)) {
        (true, _) => Nearly::Equal,
        (false, Some(std::cmp::Ordering::Less)) => Nearly::Below,
        (false, Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)) => Nearly::Above,
        (false, None) => Nearly::Unordered,
      },
    )
  }
}

impl Nearness for f64 {
  fn nearly(self, other: Self) -> Result<Nearly, Never> {
    let apart = self - other;

    Ok(
      match (
        apart.abs() <= f64::from(SCALAR_NEARLY_ZERO),
        self.partial_cmp(&other),
      ) {
        (true, _) => Nearly::Equal,
        (false, Some(std::cmp::Ordering::Less)) => Nearly::Below,
        (false, Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)) => Nearly::Above,
        (false, None) => Nearly::Unordered,
      },
    )
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sameness {
  Same,
  Different,
}

pub(crate) fn nearly_same(one: Point, other: Point) -> Result<Sameness, Never> {
  let Ok(one_x) = narrow_f32(one.x);
  let Ok(one_y) = narrow_f32(one.y);
  let Ok(other_x) = narrow_f32(other.x);
  let Ok(other_y) = narrow_f32(other.y);
  let Ok(across) = one_x.nearly(other_x);
  let Ok(down) = one_y.nearly(other_y);

  Ok(match (across, down) {
    (Nearly::Equal, Nearly::Equal) => Sameness::Same,
    (Nearly::Below | Nearly::Equal | Nearly::Above | Nearly::Unordered, _) => Sameness::Different,
  })
}

pub(crate) fn hash128<T: Hash + ?Sized>(value: &T) -> Result<u128, Never> {
  let mut state = SipHasher13::new();

  value.hash(&mut state);

  Ok(state.finish128().as_u128())
}

pub(crate) fn fast_bounding_box(path: &BezPath) -> Result<Rect, Never> {
  let mut low = Point::new(f64::INFINITY, f64::INFINITY);
  let mut high = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);

  for element in path.elements() {
    let points: &[Point] = match element {
      PathEl::MoveTo(point) | PathEl::LineTo(point) => std::slice::from_ref(point),
      PathEl::QuadTo(control, end) => &[*control, *end],
      PathEl::CurveTo(first, second, end) => &[*first, *second, *end],
      PathEl::ClosePath => &[],
    };

    for point in points {
      low = Point::new(low.x.min(point.x), low.y.min(point.y));
      high = Point::new(high.x.max(point.x), high.y.max(point.y));
    }
  }

  Ok(match ::core::primitive::bool::then_some(low.x > high.x, ()) {
    ::core::option::Option::Some(()) => Rect::ZERO,
    ::core::option::Option::None => Rect::from_points(low, high),
  })
}

pub trait TransformExt {
  fn to_kurbo(&self) -> kurbo::Affine;
}

impl TransformExt for console_core_portable_document_syntax::transform::Transform {
  fn to_kurbo(&self) -> kurbo::Affine {
    let Ok(coefficients) = self.as_coeffs();

    kurbo::Affine::new(coefficients)
  }
}

pub trait RectExt {
  fn to_kurbo(&self) -> Rect;
}

impl RectExt for console_core_portable_document_syntax::object::Rect {
  fn to_kurbo(&self) -> Rect {
    Rect::from_bounds(*self)
  }
}
