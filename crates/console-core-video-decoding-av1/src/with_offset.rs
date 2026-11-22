#![forbid(unsafe_code)]
use crate::src::strided::Strided;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Sub;
use std::ops::SubAssign;

#[derive(Clone, Copy)]
pub struct WithOffset<T> {
  pub data: T,
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(explicit051_no_machine_width, reason = "the public PicOffset view stores its established Rust slice position at native width")
  )]
  pub offset: usize,
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native unsigned index operand")
)]
impl<T> AddAssign<usize> for WithOffset<T> {
  #[cfg_attr(debug_assertions, track_caller)]
  fn add_assign(&mut self, rhs: ::console_core_api_compatibility::NativeSize) {
    let ::core::result::Result::Ok(()) =
      ::console_core_number_conversion::wrapping_add_index(rhs, &mut self.offset);
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native unsigned index operand")
)]
impl<T> SubAssign<usize> for WithOffset<T> {
  #[cfg_attr(debug_assertions, track_caller)]
  fn sub_assign(&mut self, rhs: ::console_core_api_compatibility::NativeSize) {
    let ::core::result::Result::Ok(()) =
      ::console_core_number_conversion::wrapping_sub_index(rhs, &mut self.offset);
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native signed displacement operand")
)]
impl<T> AddAssign<isize> for WithOffset<T> {
  #[cfg_attr(debug_assertions, track_caller)]
  fn add_assign(&mut self, rhs: ::console_core_api_compatibility::NativeOffset) {
    self.offset = self.offset.wrapping_add_signed(rhs);
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native signed displacement operand")
)]
impl<T> SubAssign<isize> for WithOffset<T> {
  #[cfg_attr(debug_assertions, track_caller)]
  fn sub_assign(&mut self, rhs: ::console_core_api_compatibility::NativeOffset) {
    self.offset = self.offset.wrapping_add_signed((rhs).wrapping_neg());
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native unsigned index operand")
)]
impl<T> Add<usize> for WithOffset<T> {
  type Output = Self;

  #[cfg_attr(debug_assertions, track_caller)]
  fn add(mut self, rhs: ::console_core_api_compatibility::NativeSize) -> Self::Output {
    self += rhs;
    self
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native unsigned index operand")
)]
impl<T> Sub<usize> for WithOffset<T> {
  type Output = Self;

  #[cfg_attr(debug_assertions, track_caller)]
  fn sub(mut self, rhs: ::console_core_api_compatibility::NativeSize) -> Self::Output {
    self -= rhs;
    self
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native signed displacement operand")
)]
impl<T> Add<isize> for WithOffset<T> {
  type Output = Self;

  #[cfg_attr(debug_assertions, track_caller)]
  fn add(mut self, rhs: ::console_core_api_compatibility::NativeOffset) -> Self::Output {
    self += rhs;
    self
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(explicit051_no_machine_width, reason = "the public picture offset operator retains its native signed displacement operand")
)]
impl<T> Sub<isize> for WithOffset<T> {
  type Output = Self;

  #[cfg_attr(debug_assertions, track_caller)]
  fn sub(mut self, rhs: ::console_core_api_compatibility::NativeOffset) -> Self::Output {
    self -= rhs;
    self
  }
}

impl<S: Strided> Strided for WithOffset<S> {
  fn fixed_stride(&self) -> Result<i64, console_core_never::Never> {
    self.data.fixed_stride()
  }
}
