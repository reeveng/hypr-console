use crate::{NonZeroRect, Point};
use console_core_never::Never;
use console_core_number_conversion::nearest_f32_from_f64;
use console_core_point::Point as Coordinates;
use console_core_transform::AffineTransform;

use crate::scalar::{SCALAR_NEARLY_ZERO, Scalar};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The public affine matrix exposes its original coefficients directly. Its linear entries are ratios and translations are in path units"
  )
)]
pub struct Transform {
  pub sx: f32,
  pub kx: f32,
  pub ky: f32,
  pub sy: f32,
  pub tx: f32,
  pub ty: f32,
}

#[doc = "Independent, dimensionless scale factors along the two path axes."]
#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "axis scale factors are dimensionless ratios"
  )
)]
pub struct ScaleFactors {
  pub x: f32,
  pub y: f32,
}

impl Default for Transform {
  fn default() -> Self {
    Transform {
      sx: 1.0,
      kx: 0.0,
      ky: 0.0,
      sy: 1.0,
      tx: 0.0,
      ty: 0.0,
    }
  }
}

impl Transform {
  pub fn identity() -> Result<Self, Never> {
    Ok(Transform::default())
  }

  pub fn from_translation(offset: Point) -> Result<Self, Never> {
    Ok(Self {
      tx: offset.x,
      ty: offset.y,
      ..Self::default()
    })
  }

  pub fn from_scale_factors(factors: ScaleFactors) -> Result<Self, Never> {
    Ok(Self {
      sx: factors.x,
      sy: factors.y,
      ..Self::default()
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public skew parameters are dimensionless coefficients"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public skew constructor accepts the x and y coefficients in that order"
    )
  )]
  pub fn from_skew(kx: f32, ky: f32) -> Result<Self, Never> {
    let Ok(transform) = Transform::from_row(1.0, ky, kx, 1.0, 0.0, 0.0);

    Ok(transform)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public rotation angle is expressed in degrees"
    )
  )]
  pub fn from_rotate(angle: f32) -> Result<Self, Never> {
    let v = angle.to_radians();
    let a = v.cos();
    let b = v.sin();
    let c = -b;
    let d = a;
    let Ok(transform) = Transform::from_row(a, b, c, d, 0.0, 0.0);

    Ok(transform)
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The angle is in degrees and the pivot is in path coordinates"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rotation accepts degrees, then the pivot x and y"
    )
  )]
  pub fn from_rotate_at(angle: f32, tx: f32, ty: f32) -> Result<Self, Never> {
    let mut ts = Self::default();
    let Ok(pre_translate) = ts.pre_translate_by(Point { x: tx, y: ty });
    ts = pre_translate;
    let Ok(transform) = Transform::from_rotate(angle);
    let Ok(pre_concat) = ts.pre_concat(transform);
    ts = pre_concat;
    let Ok(ts_pre_translate) = ts.pre_translate_by(Point { x: -tx, y: -ty });
    ts = ts_pre_translate;

    Ok(ts)
  }

  #[inline]
  pub fn from_bbox(bbox: NonZeroRect) -> Result<Self, Never> {
    let Ok(width) = bbox.width();
    let Ok(height) = bbox.height();
    let Ok(transform) = Transform::from_row(
      width,
      0.0,
      0.0,
      height,
      {
        let Ok(explicit_value) = bbox.x();
        explicit_value
      },
      {
        let Ok(explicit_value) = bbox.y();
        explicit_value
      },
    );

    Ok(transform)
  }

  pub fn is_finite(&self) -> Result<crate::IsFiniteAnswer, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.sx.is_finite()
          && self.ky.is_finite()
          && self.kx.is_finite()
          && self.sy.is_finite()
          && self.tx.is_finite()
          && self.ty.is_finite(),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsFiniteAnswer::Yes,
        ::core::option::Option::None => crate::IsFiniteAnswer::No,
      },
    )
  }

  pub fn is_valid(&self) -> Result<crate::IsValidAnswer, Never> {
    let Ok(is_finite) = self.is_finite();

    Ok(match is_finite {
      crate::IsFiniteAnswer::Yes => {
        let Ok((sx, sy)) = self.get_scale();

        match ::core::primitive::bool::then_some(
          sx.is_nearly_zero_within_tolerance(f32::EPSILON)
            == crate::ScalarProximity::WithinTolerance
            || sy.is_nearly_zero_within_tolerance(f32::EPSILON)
              == crate::ScalarProximity::WithinTolerance,
          (),
        ) {
          ::core::option::Option::Some(()) => crate::IsValidAnswer::No,
          ::core::option::Option::None => crate::IsValidAnswer::Yes,
        }
      }
      crate::IsFiniteAnswer::No => crate::IsValidAnswer::No,
    })
  }

  pub fn is_identity(&self) -> Result<crate::IsIdentityAnswer, Never> {
    Ok(
      match ::core::primitive::bool::then_some(*self == Transform::default(), ()) {
        ::core::option::Option::Some(()) => crate::IsIdentityAnswer::Yes,
        ::core::option::Option::None => crate::IsIdentityAnswer::No,
      },
    )
  }

  pub fn is_scale(&self) -> Result<crate::IsScaleAnswer, Never> {
    let Ok(has_scale) = self.has_scale();

    Ok(
      match ::core::primitive::bool::then_some(
        matches!(has_scale, crate::HasScaleAnswer::Yes)
          && !matches!(
            {
              let Ok(explicit_value) = self.has_skew();
              explicit_value
            },
            crate::HasSkewAnswer::Yes
          )
          && !matches!(
            {
              let Ok(explicit_value) = self.has_translate();
              explicit_value
            },
            crate::HasTranslateAnswer::Yes
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsScaleAnswer::Yes,
        ::core::option::Option::None => crate::IsScaleAnswer::No,
      },
    )
  }

  pub fn is_skew(&self) -> Result<crate::IsSkewAnswer, Never> {
    let Ok(has_scale) = self.has_scale();

    Ok(
      match ::core::primitive::bool::then_some(
        !matches!(has_scale, crate::HasScaleAnswer::Yes)
          && matches!(
            {
              let Ok(explicit_value) = self.has_skew();
              explicit_value
            },
            crate::HasSkewAnswer::Yes
          )
          && !matches!(
            {
              let Ok(explicit_value) = self.has_translate();
              explicit_value
            },
            crate::HasTranslateAnswer::Yes
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsSkewAnswer::Yes,
        ::core::option::Option::None => crate::IsSkewAnswer::No,
      },
    )
  }

  pub fn is_translate(&self) -> Result<crate::IsTranslateAnswer, Never> {
    let Ok(has_scale) = self.has_scale();

    Ok(
      match ::core::primitive::bool::then_some(
        !matches!(has_scale, crate::HasScaleAnswer::Yes)
          && !matches!(
            {
              let Ok(explicit_value) = self.has_skew();
              explicit_value
            },
            crate::HasSkewAnswer::Yes
          )
          && matches!(
            {
              let Ok(explicit_value) = self.has_translate();
              explicit_value
            },
            crate::HasTranslateAnswer::Yes
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsTranslateAnswer::Yes,
        ::core::option::Option::None => crate::IsTranslateAnswer::No,
      },
    )
  }

  pub fn is_scale_translate(&self) -> Result<crate::IsScaleTranslateAnswer, Never> {
    let Ok(has_scale) = self.has_scale();

    Ok(
      match ::core::primitive::bool::then_some(
        (matches!(has_scale, crate::HasScaleAnswer::Yes)
          || matches!(
            {
              let Ok(explicit_value) = self.has_translate();
              explicit_value
            },
            crate::HasTranslateAnswer::Yes
          ))
          && !matches!(
            {
              let Ok(explicit_value) = self.has_skew();
              explicit_value
            },
            crate::HasSkewAnswer::Yes
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsScaleTranslateAnswer::Yes,
        ::core::option::Option::None => crate::IsScaleTranslateAnswer::No,
      },
    )
  }

  pub fn has_scale(&self) -> Result<crate::HasScaleAnswer, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        ::core::primitive::f32::to_bits(self.sx) != ::core::primitive::f32::to_bits(1.0)
          || ::core::primitive::f32::to_bits(self.sy) != ::core::primitive::f32::to_bits(1.0),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::HasScaleAnswer::Yes,
        ::core::option::Option::None => crate::HasScaleAnswer::No,
      },
    )
  }

  pub fn has_skew(&self) -> Result<crate::HasSkewAnswer, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        !::core::matches!(
          ::core::primitive::f32::classify(self.kx),
          ::core::num::FpCategory::Zero
        ) || !::core::matches!(
          ::core::primitive::f32::classify(self.ky),
          ::core::num::FpCategory::Zero
        ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::HasSkewAnswer::Yes,
        ::core::option::Option::None => crate::HasSkewAnswer::No,
      },
    )
  }

  pub fn has_translate(&self) -> Result<crate::HasTranslateAnswer, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        !::core::matches!(
          ::core::primitive::f32::classify(self.tx),
          ::core::num::FpCategory::Zero
        ) || !::core::matches!(
          ::core::primitive::f32::classify(self.ty),
          ::core::num::FpCategory::Zero
        ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::HasTranslateAnswer::Yes,
        ::core::option::Option::None => crate::HasTranslateAnswer::No,
      },
    )
  }

  pub fn get_scale(&self) -> Result<(f32, f32), Never> {
    let x_scale = (self.sx * self.sx + self.kx * self.kx).sqrt();
    let y_scale = (self.ky * self.ky + self.sy * self.sy).sqrt();

    Ok((x_scale, y_scale))
  }

  #[must_use]
  pub fn pre_scale_by(&self, factors: ScaleFactors) -> Result<Self, Never> {
    let Ok(transform) = Self::from_scale_factors(factors);
    self.pre_concat(transform)
  }

  #[must_use]
  pub fn post_scale_by(&self, factors: ScaleFactors) -> Result<Self, Never> {
    let Ok(transform) = Self::from_scale_factors(factors);
    self.post_concat(transform)
  }

  #[must_use]
  pub fn pre_translate_by(&self, offset: Point) -> Result<Self, Never> {
    let Ok(transform) = Self::from_translation(offset);
    self.pre_concat(transform)
  }

  #[must_use]
  pub fn post_translate_by(&self, offset: Point) -> Result<Self, Never> {
    let Ok(transform) = Self::from_translation(offset);
    self.post_concat(transform)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public rotation angle is expressed in degrees"
    )
  )]
  pub fn pre_rotate(&self, angle: f32) -> Result<Self, Never> {
    let Ok(transform) = Transform::from_rotate(angle);
    let Ok(pre_concat) = self.pre_concat(transform);

    Ok(pre_concat)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public rotation angle is expressed in degrees"
    )
  )]
  pub fn post_rotate(&self, angle: f32) -> Result<Self, Never> {
    let Ok(transform) = Transform::from_rotate(angle);
    let Ok(post_concat) = self.post_concat(transform);

    Ok(post_concat)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The angle is in degrees and the pivot is in path coordinates"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rotation accepts degrees, then the pivot x and y"
    )
  )]
  pub fn pre_rotate_at(&self, angle: f32, tx: f32, ty: f32) -> Result<Self, Never> {
    let Ok(transform) = Transform::from_rotate_at(angle, tx, ty);
    let Ok(pre_concat) = self.pre_concat(transform);

    Ok(pre_concat)
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The angle is in degrees and the pivot is in path coordinates"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public rotation accepts degrees, then the pivot x and y"
    )
  )]
  pub fn post_rotate_at(&self, angle: f32, tx: f32, ty: f32) -> Result<Self, Never> {
    let Ok(transform) = Transform::from_rotate_at(angle, tx, ty);
    let Ok(post_concat) = self.post_concat(transform);

    Ok(post_concat)
  }

  #[must_use]
  pub fn pre_concat(&self, other: Self) -> Result<Self, Never> {
    let Ok(explicit_value) = concat(*self, other);

    Ok(explicit_value)
  }

  #[must_use]
  pub fn post_concat(&self, other: Self) -> Result<Self, Never> {
    let Ok(explicit_value) = concat(other, *self);

    Ok(explicit_value)
  }

  pub(crate) fn from_unit_direction(direction: Point) -> Result<Self, Never> {
    Self::from_row(
      direction.x,
      direction.y,
      -direction.y,
      direction.x,
      0.0,
      0.0,
    )
  }

  fn point_mapping(&self) -> Result<PointMapping, Never> {
    let Ok(identity) = self.is_identity();

    match (matches!(identity, crate::IsIdentityAnswer::Yes)).then_some(()) {
      Some(()) => {
        return Ok(PointMapping::Unchanged);
      }
      None => {}
    }

    let Ok(translation) = self.is_translate();

    match (matches!(translation, crate::IsTranslateAnswer::Yes)).then_some(()) {
      Some(()) => {
        return Ok(PointMapping::Translation);
      }
      None => {}
    }

    let Ok(scale_translation) = self.is_scale_translate();
    Ok(match scale_translation {
      crate::IsScaleTranslateAnswer::Yes => PointMapping::ScaleAndTranslation,
      crate::IsScaleTranslateAnswer::No => PointMapping::Affine,
    })
  }

  pub fn map_point(&self, point: &mut Point) -> Result<(), Never> {
    self.map_points(core::slice::from_mut(point))
  }

  pub fn map_points(&self, points: &mut [Point]) -> Result<(), Never> {
    let Ok(mapping) = self.point_mapping();

    match mapping {
      PointMapping::Unchanged => {}
      PointMapping::Translation => {
        for point in points {
          point.x += self.tx;
          point.y += self.ty;
        }
      }
      PointMapping::ScaleAndTranslation => {
        for point in points {
          point.x = point.x * self.sx + self.tx;
          point.y = point.y * self.sy + self.ty;
        }
      }
      PointMapping::Affine => {
        for point in points {
          let x = point.x * self.sx + point.y * self.kx + self.tx;
          let y = point.x * self.ky + point.y * self.sy + self.ty;
          point.x = x;
          point.y = y;
        }
      }
    }

    Ok(())
  }

  pub fn invert(&self) -> Result<Option<Self>, Never> {
    let Ok(identity) = self.is_identity();

    match (matches!(identity, crate::IsIdentityAnswer::Yes)).then_some(()) {
      Some(()) => {
        return Ok(Some(*self));
      }
      None => {}
    }

    let Ok(scale_translation) = self.is_scale_translate();

    match (matches!(scale_translation, crate::IsScaleTranslateAnswer::Yes)).then_some(()) {
      Some(()) => {
        let Ok(scale) = self.has_scale();
        let inverse = match scale {
          crate::HasScaleAnswer::Yes => {
            let inv_x = self.sx.invert();
            let inv_y = self.sy.invert();
            let Ok(transform) =
              Self::from_row(inv_x, 0.0, 0.0, inv_y, -self.tx * inv_x, -self.ty * inv_y);
            transform
          }
          crate::HasScaleAnswer::No => {
            let Ok(transform) = Self::from_translation(Point {
              x: -self.tx,
              y: -self.ty,
            });
            transform
          }
        };

        return Ok(Some(inverse));
      }
      None => {}
    }

    let Ok(determinant) = inv_determinant(self);
    let reciprocal = match determinant {
      Some(reciprocal) => reciprocal,
      None => return Ok(None),
    };
    let Ok(inverse) = compute_inverse(self, reciprocal);
    let Ok(finite) = inverse.is_finite();

    Ok(match finite {
      crate::IsFiniteAnswer::Yes => Some(inverse),
      crate::IsFiniteAnswer::No => None,
    })
  }
}

enum PointMapping {
  Unchanged,
  Translation,
  ScaleAndTranslation,
  Affine,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The inverse determinant is the reciprocal of the matrix's dimensionless linear determinant"
  )
)]
struct InverseDeterminant(f64);

fn inv_determinant(transform: &Transform) -> Result<Option<InverseDeterminant>, Never> {
  let determinant = f64::from(transform.sx) * f64::from(transform.sy)
    - f64::from(transform.kx) * f64::from(transform.ky);
  let tolerance = SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO;
  let Ok(rounded) = nearest_f32_from_f64(determinant);

  Ok(match rounded.is_nearly_zero_within_tolerance(tolerance) {
    crate::ScalarProximity::WithinTolerance => None,
    crate::ScalarProximity::OutsideTolerance => Some(InverseDeterminant(1.0 / determinant)),
  })
}

fn compute_inverse(
  transform: &Transform,
  InverseDeterminant(reciprocal): InverseDeterminant,
) -> Result<Transform, Never> {
  let Ok(sx) = nearest_f32_from_f64(f64::from(transform.sy) * reciprocal);
  let Ok(ky) = nearest_f32_from_f64(f64::from(-transform.ky) * reciprocal);
  let Ok(kx) = nearest_f32_from_f64(f64::from(-transform.kx) * reciprocal);
  let Ok(sy) = nearest_f32_from_f64(f64::from(transform.sx) * reciprocal);
  let x = f64::from(transform.kx) * f64::from(transform.ty)
    - f64::from(transform.sy) * f64::from(transform.tx);
  let y = f64::from(transform.ky) * f64::from(transform.tx)
    - f64::from(transform.sx) * f64::from(transform.ty);
  let Ok(tx) = nearest_f32_from_f64(x * reciprocal);
  let Ok(ty) = nearest_f32_from_f64(y * reciprocal);

  Transform::from_row(sx, ky, kx, sy, tx, ty)
}

fn concat(a: Transform, b: Transform) -> Result<Transform, Never> {
  let Ok(first_identity) = a.is_identity();
  let Ok(second_identity) = b.is_identity();

  match (first_identity, second_identity) {
    (crate::IsIdentityAnswer::Yes, _) => return Ok(b),
    (_, crate::IsIdentityAnswer::Yes) => return Ok(a),
    (crate::IsIdentityAnswer::No, crate::IsIdentityAnswer::No) => {}
  }

  let Ok(first_skew) = a.has_skew();
  let Ok(second_skew) = b.has_skew();

  match (matches!(
    (first_skew, second_skew),
    (crate::HasSkewAnswer::No, crate::HasSkewAnswer::No)
  ))
  .then_some(())
  {
    Some(()) => {
      return Transform::from_row(
        a.sx * b.sx,
        0.0,
        0.0,
        a.sy * b.sy,
        a.sx * b.tx + a.tx,
        a.sy * b.ty + a.ty,
      );
    }
    None => {}
  }

  let Ok(widened) = AffineTransform::from_coefficients([
    f64::from(a.sx),
    f64::from(a.ky),
    f64::from(a.kx),
    f64::from(a.sy),
    0.0,
    0.0,
  ]);
  let rounded_column = |column: Point| {
    let Ok(wide) = widened.apply_linear(Coordinates {
      x: f64::from(column.x),
      y: f64::from(column.y),
    });
    let Ok(x) = nearest_f32_from_f64(wide.x);
    let Ok(y) = nearest_f32_from_f64(wide.y);

    Point { x, y }
  };
  let first = rounded_column(Point { x: b.sx, y: b.ky });
  let second = rounded_column(Point { x: b.kx, y: b.sy });
  let offset = rounded_column(Point { x: b.tx, y: b.ty });

  Transform::from_row(
    first.x,
    first.y,
    second.x,
    second.y,
    offset.x + a.tx,
    offset.y + a.ty,
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    HasScaleAnswer, HasSkewAnswer, HasTranslateAnswer, IsIdentityAnswer, IsScaleAnswer,
    IsScaleTranslateAnswer, IsSkewAnswer, IsTranslateAnswer,
  };

  #[test]
  fn transform() {
    assert_eq!(
      Transform::identity(),
      Transform::from_row(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    );
    assert_eq!(
      Transform::from_scale(1.0, 2.0),
      Transform::from_row(1.0, 0.0, 0.0, 2.0, 0.0, 0.0)
    );
    assert_eq!(
      Transform::from_skew(2.0, 3.0),
      Transform::from_row(1.0, 3.0, 2.0, 1.0, 0.0, 0.0)
    );
    assert_eq!(
      Transform::from_translate(5.0, 6.0),
      Transform::from_row(1.0, 0.0, 0.0, 1.0, 5.0, 6.0)
    );

    let Ok(ts) = Transform::identity();
    assert_eq!(ts.is_identity(), Ok(IsIdentityAnswer::Yes));
    assert_eq!(ts.is_scale(), Ok(IsScaleAnswer::No));
    assert_eq!(ts.is_skew(), Ok(IsSkewAnswer::No));
    assert_eq!(ts.is_translate(), Ok(IsTranslateAnswer::No));
    assert_eq!(ts.is_scale_translate(), Ok(IsScaleTranslateAnswer::No));
    assert_eq!(ts.has_scale(), Ok(HasScaleAnswer::No));
    assert_eq!(ts.has_skew(), Ok(HasSkewAnswer::No));
    assert_eq!(ts.has_translate(), Ok(HasTranslateAnswer::No));

    let Ok(ts_2) = Transform::from_scale_factors(crate::ScaleFactors { x: 2.0, y: 3.0 });
    assert_eq!(ts_2.is_identity(), Ok(IsIdentityAnswer::No));
    assert_eq!(ts_2.is_scale(), Ok(IsScaleAnswer::Yes));
    assert_eq!(ts_2.is_skew(), Ok(IsSkewAnswer::No));
    assert_eq!(ts_2.is_translate(), Ok(IsTranslateAnswer::No));
    assert_eq!(ts_2.is_scale_translate(), Ok(IsScaleTranslateAnswer::Yes));
    assert_eq!(ts_2.has_scale(), Ok(HasScaleAnswer::Yes));
    assert_eq!(ts_2.has_skew(), Ok(HasSkewAnswer::No));
    assert_eq!(ts_2.has_translate(), Ok(HasTranslateAnswer::No));

    let Ok(ts_3) = Transform::from_skew(2.0, 3.0);
    assert_eq!(ts_3.is_identity(), Ok(IsIdentityAnswer::No));
    assert_eq!(ts_3.is_scale(), Ok(IsScaleAnswer::No));
    assert_eq!(ts_3.is_skew(), Ok(IsSkewAnswer::Yes));
    assert_eq!(ts_3.is_translate(), Ok(IsTranslateAnswer::No));
    assert_eq!(ts_3.is_scale_translate(), Ok(IsScaleTranslateAnswer::No));
    assert_eq!(ts_3.has_scale(), Ok(HasScaleAnswer::No));
    assert_eq!(ts_3.has_skew(), Ok(HasSkewAnswer::Yes));
    assert_eq!(ts_3.has_translate(), Ok(HasTranslateAnswer::No));

    let Ok(ts_4) = Transform::from_translation(crate::Point { x: 2.0, y: 3.0 });
    assert_eq!(ts_4.is_identity(), Ok(IsIdentityAnswer::No));
    assert_eq!(ts_4.is_scale(), Ok(IsScaleAnswer::No));
    assert_eq!(ts_4.is_skew(), Ok(IsSkewAnswer::No));
    assert_eq!(ts_4.is_translate(), Ok(IsTranslateAnswer::Yes));
    assert_eq!(ts_4.is_scale_translate(), Ok(IsScaleTranslateAnswer::Yes));
    assert_eq!(ts_4.has_scale(), Ok(HasScaleAnswer::No));
    assert_eq!(ts_4.has_skew(), Ok(HasSkewAnswer::No));
    assert_eq!(ts_4.has_translate(), Ok(HasTranslateAnswer::Yes));

    let Ok(ts_5) = Transform::from_row(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
    assert_eq!(ts_5.is_identity(), Ok(IsIdentityAnswer::No));
    assert_eq!(ts_5.is_scale(), Ok(IsScaleAnswer::No));
    assert_eq!(ts_5.is_skew(), Ok(IsSkewAnswer::No));
    assert_eq!(ts_5.is_translate(), Ok(IsTranslateAnswer::No));
    assert_eq!(ts_5.is_scale_translate(), Ok(IsScaleTranslateAnswer::No));
    assert_eq!(ts_5.has_scale(), Ok(HasScaleAnswer::Yes));
    assert_eq!(ts_5.has_skew(), Ok(HasSkewAnswer::Yes));
    assert_eq!(ts_5.has_translate(), Ok(HasTranslateAnswer::Yes));

    let Ok(ts_6) = Transform::from_scale_factors(crate::ScaleFactors { x: 1.0, y: 1.0 });
    assert_eq!(ts_6.has_scale(), Ok(HasScaleAnswer::No));

    let Ok(ts_7) = Transform::from_skew(0.0, 0.0);
    assert_eq!(ts_7.has_skew(), Ok(HasSkewAnswer::No));

    let Ok(ts_8) = Transform::from_translation(crate::Point { x: 0.0, y: 0.0 });
    assert_eq!(ts_8.has_translate(), Ok(HasTranslateAnswer::No));
  }

  #[test]
  fn concat() {
    let Ok(ts) = Transform::from_row(1.2, 3.4, -5.6, -7.8, 1.2, 3.4);
    assert_eq!(
      ts.pre_scale(2.0, -4.0),
      Transform::from_row(2.4, 6.8, 22.4, 31.2, 1.2, 3.4)
    );

    let Ok(ts_2) = Transform::from_row(1.2, 3.4, -5.6, -7.8, 1.2, 3.4);
    assert_eq!(
      ts_2.post_scale(2.0, -4.0),
      Transform::from_row(2.4, -13.6, -11.2, 31.2, 2.4, -13.6)
    );
  }

  #[test]
  fn concatenation_rounds_the_linear_offset_before_adding_translation() {
    let skew = f32::EPSILON / 4.0;
    let Ok(outer) = Transform::from_row(1.0, 0.0, skew, 1.0, -1.0, 0.0);
    let Ok(offset) = Transform::from_translation(Point { x: 1.0, y: 1.0 });
    let Ok(combined) = outer.pre_concat(offset);

    assert_eq!(combined.tx, 0.0);
    assert_eq!(combined.ty, 1.0);
  }
}
