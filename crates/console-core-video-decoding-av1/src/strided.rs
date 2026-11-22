#![forbid(unsafe_code)]
use crate::include::common::bitdepth::BitDepth;
use console_core_api_compatibility::NativeOffset;
use console_core_never::Never;
use console_core_number_conversion::wrapped_offset;
use std::ops::Deref;
use std::ops::DerefMut;

pub trait Strided {
  fn fixed_stride(&self) -> Result<i64, Never>;

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "the existing native stride accessor returns the signed slice displacement directly")
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the existing picture view uses native signed byte displacements for slice access")
  )]
  fn stride(&self) -> NativeOffset {
    let Ok(fixed) = self.fixed_stride();
    let Ok(native) = wrapped_offset(fixed);
    native
  }

  fn fixed_pixel_stride<BD: BitDepth>(&self) -> Result<i64, Never> {
    let Ok(stride) = self.fixed_stride();
    Ok(BD::pxstride(stride))
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the existing picture view uses native signed pixel displacements for slice access")
  )]
  fn pixel_stride<BD: BitDepth>(&self) -> Result<NativeOffset, Never> {
    let Ok(fixed) = self.fixed_pixel_stride::<BD>();
    wrapped_offset(fixed)
  }
}

impl<S: Strided> Strided for &S {
  fn fixed_stride(&self) -> Result<i64, Never> {
    (*self).fixed_stride()
  }
}

#[derive(Clone, Copy)]
pub struct WithStride<T> {
  pub buf: T,
  pub stride: i64,
}

impl<T> Strided for WithStride<T> {
  fn fixed_stride(&self) -> Result<i64, Never> {
    Ok(self.stride)
  }
}

impl<T> Deref for WithStride<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.buf
  }
}

impl<T> DerefMut for WithStride<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.buf
  }
}
