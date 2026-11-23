use crate::StrictNum;
use console_core_finite_numbers::NonZeroPositiveF32;
use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_u64, toward_zero_u32};
use console_core_size::Size as Dimensions;
use core::cmp::Ordering;

use crate::{IntRect, LengthU32, NonZeroRect, FloatRectangle};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use crate::NoStdFloat;

#[derive(Copy, Clone, PartialEq, Debug, Hash, Eq)]
pub struct IntSize(Dimensions<LengthU32>);

impl IntSize {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public size constructor takes width then height in pixels"
    )
  )]
  pub fn from_wh(width: u32, height: u32) -> Result<Option<Self>, Never> {
    Ok(Some(Self(Dimensions {
      width: match LengthU32::new(width) {
        Some(value) => value,
        None => return Ok(None),
      },
      height: match LengthU32::new(height) {
        Some(value) => value,
        None => return Ok(None),
      },
    })))
  }

  pub(crate) fn from_wh_safe(width: LengthU32, height: LengthU32) -> Result<Self, Never> {
    Ok(Self(Dimensions { width, height }))
  }

  pub fn width(&self) -> Result<u32, Never> {
    Ok(self.0.width.get())
  }

  pub fn height(&self) -> Result<u32, Never> {
    Ok(self.0.height.get())
  }

  pub fn dimensions(&self) -> Result<(u32, u32), Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();

    Ok((width, height))
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public scaling factor is a dimensionless ratio"
    )
  )]
  pub fn scale_by(&self, factor: f32) -> Result<Option<Self>, Never> {
    let Ok(width) = extent_as_float(self.0.width);
    let Ok(height) = extent_as_float(self.0.height);
    let Ok(scaled_width) = toward_zero_u32(f64::from((width * factor).round()));
    let Ok(scaled_height) = toward_zero_u32(f64::from((height * factor).round()));

    Self::from_wh(scaled_width, scaled_height)
  }

  #[inline]
  pub fn scale_to(&self, to: Self) -> Result<Self, Never> {
    let Ok(width) = scaled_integer_extent(IntScale {
      target: to.0.height,
      source: self.0.width,
      reference: self.0.height,
    });
    let Ok(axis) = ScaleMode::FitWithin.axis(Some(width.cmp(&to.0.width)));

    match axis {
      ScaleAxis::Height => Self::from_wh_safe(width, to.0.height),
      ScaleAxis::Width => {
        let Ok(height) = scaled_integer_extent(IntScale {
          target: to.0.width,
          source: self.0.height,
          reference: self.0.width,
        });
        Self::from_wh_safe(to.0.width, height)
      }
    }
  }

  #[inline]
  pub fn scale_to_width(&self, new_width: u32) -> Result<Option<Self>, Never> {
    let width = match LengthU32::new(new_width) {
      Some(width) => width,
      None => return Ok(None),
    };
    let Ok(height) = scaled_integer_extent(IntScale {
      target: width,
      source: self.0.height,
      reference: self.0.width,
    });

    Ok(Some(Self(Dimensions { width, height })))
  }

  #[inline]
  pub fn scale_to_height(&self, new_height: u32) -> Result<Option<Self>, Never> {
    let height = match LengthU32::new(new_height) {
      Some(height) => height,
      None => return Ok(None),
    };
    let Ok(width) = scaled_integer_extent(IntScale {
      target: height,
      source: self.0.width,
      reference: self.0.height,
    });

    Ok(Some(Self(Dimensions { width, height })))
  }

  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "A nonzero u32 always converts to a positive finite f32"
    )
  )]
  pub fn to_size(&self) -> Result<Size, Never> {
    let Ok(width) = extent_as_float(self.0.width);
    let Ok(height) = extent_as_float(self.0.height);
    let Ok(size) = Size::from_wh(width, height);

    Ok(size.unwrap())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public position takes x then y in pixels"
    )
  )]
  pub fn to_int_rect(&self, x: i32, y: i32) -> Result<Option<IntRect>, Never> {
    IntRect::from_xywh(x, y, self.0.width.get(), self.0.height.get())
  }
}

enum ScaleMode {
  FitWithin,
  Fill,
}

enum ScaleAxis {
  Width,
  Height,
}

impl ScaleMode {
  fn axis(self, width_at_target_height: Option<Ordering>) -> Result<ScaleAxis, Never> {
    use Ordering::{Equal, Greater, Less};

    Ok(match (self, width_at_target_height) {
      (Self::FitWithin, Some(Equal | Greater)) | (Self::Fill, Some(Equal | Less)) => {
        ScaleAxis::Width
      }
      _ => ScaleAxis::Height,
    })
  }
}

struct IntScale {
  target: LengthU32,
  source: LengthU32,
  reference: LengthU32,
}

fn extent_as_float(extent: LengthU32) -> Result<f32, Never> {
  nearest_f32_from_u64(u64::from(extent.get()))
}

fn scaled_integer_extent(scale: IntScale) -> Result<LengthU32, Never> {
  let Ok(target) = extent_as_float(scale.target);
  let Ok(source) = extent_as_float(scale.source);
  let Ok(reference) = extent_as_float(scale.reference);
  let Ok(rounded) = toward_zero_u32(f64::from((target * source / reference).ceil()));

  Ok(match LengthU32::new(rounded) {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => LengthU32::MIN,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_support::{Required, TestFailure};

  #[test]
  fn int_size_tests() -> Result<(), TestFailure> {
    assert_eq!(IntSize::from_wh(0, 0), Ok(None));
    assert_eq!(IntSize::from_wh(1, 0), Ok(None));
    assert_eq!(IntSize::from_wh(0, 1), Ok(None));

    let Ok(made) = IntSize::from_wh(3, 4);
    let size = made.required()?;
    assert_eq!(size.to_int_rect(1, 2), IntRect::from_xywh(1, 2, 3, 4));
    assert_eq!(size.to_int_rect(i32::MAX, 2), Ok(None));

    Ok(())
  }
}

#[derive(Copy, Clone, PartialEq, Debug, Hash, Eq)]
pub struct Size(Dimensions<NonZeroPositiveF32>);

impl Size {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public size constructor takes width then height in path units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public dimensions are in path coordinate units"
    )
  )]
  pub fn from_wh(width: f32, height: f32) -> Result<Option<Self>, Never> {
    Ok(Some(Self(Dimensions {
      width: match NonZeroPositiveF32::new(width) {
        Some(value) => value,
        None => return Ok(None),
      },
      height: match NonZeroPositiveF32::new(height) {
        Some(value) => value,
        None => return Ok(None),
      },
    })))
  }

  pub fn width(&self) -> Result<f32, Never> {
    Ok(self.0.width.get())
  }

  pub fn height(&self) -> Result<f32, Never> {
    Ok(self.0.height.get())
  }

  pub fn scale_to(&self, to: Self) -> Result<Option<Self>, Never> {
    size_scale(*self, to, ScaleMode::FitWithin)
  }

  pub fn expand_to(&self, to: Self) -> Result<Option<Self>, Never> {
    size_scale(*self, to, ScaleMode::Fill)
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public scaling factor is a dimensionless ratio"
    )
  )]
  pub fn scale_by(&self, factor: f32) -> Result<Option<Self>, Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(size) = Self::from_wh(width * factor, height * factor);

    Ok(size)
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The requested width is in path coordinate units"
    )
  )]
  pub fn scale_to_width(&self, new_width: f32) -> Result<Option<Self>, Never> {
    let Ok(height) = self.height();
    let Ok(width) = self.width();
    let new_height = new_width * height / width;
    let Ok(size) = Self::from_wh(new_width, new_height);

    Ok(size)
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The requested height is in path coordinate units"
    )
  )]
  pub fn scale_to_height(&self, new_height: f32) -> Result<Option<Self>, Never> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let new_width = new_height * width / height;
    let Ok(size) = Self::from_wh(new_width, new_height);

    Ok(size)
  }

  pub fn to_int_size(&self) -> Result<IntSize, Never> {
    let Ok(width) = toward_zero_u32(f64::from(self.0.width.get().round()));
    let Ok(height) = toward_zero_u32(f64::from(self.0.height.get().round()));

    IntSize::from_wh_safe(
      match LengthU32::new(width) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => LengthU32::MIN,
      },
      match LengthU32::new(height) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => LengthU32::MIN,
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public position takes x then y in path units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The position is in path coordinate units"
    )
  )]
  pub fn to_rect(&self, x: f32, y: f32) -> Result<Option<FloatRectangle>, Never> {
    FloatRectangle::from_xywh(x, y, self.0.width.get(), self.0.height.get())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public position takes x then y in path units"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The position is in path coordinate units"
    )
  )]
  pub fn to_non_zero_rect(&self, x: f32, y: f32) -> Result<Option<NonZeroRect>, Never> {
    NonZeroRect::from_xywh(x, y, self.0.width.get(), self.0.height.get())
  }
}

fn size_scale(source: Size, target: Size, mode: ScaleMode) -> Result<Option<Size>, Never> {
  let width = target.0.height.get() * source.0.width.get() / source.0.height.get();
  let Ok(axis) = mode.axis(width.partial_cmp(&target.0.width.get()));

  match axis {
    ScaleAxis::Height => Size::from_wh(width, target.0.height.get()),
    ScaleAxis::Width => {
      let height = target.0.width.get() * source.0.height.get() / source.0.width.get();
      Size::from_wh(target.0.width.get(), height)
    }
  }
}
