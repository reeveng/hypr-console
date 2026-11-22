//! Wrapping updates for positions and offsets that use library widths.

use console_core_never::Never;

#[inline]
pub fn wrapping_add_index(amount: usize, target: &mut usize) -> Result<(), Never> {
  *target = target.wrapping_add(amount);
  Ok(())
}

#[inline]
pub fn wrapping_sub_index(amount: usize, target: &mut usize) -> Result<(), Never> {
  *target = target.wrapping_sub(amount);
  Ok(())
}

#[inline]
pub fn wrapping_mul_index(amount: usize, target: &mut usize) -> Result<(), Never> {
  *target = target.wrapping_mul(amount);
  Ok(())
}

#[inline]
pub fn wrapping_add_offset(amount: isize, target: &mut isize) -> Result<(), Never> {
  *target = target.wrapping_add(amount);
  Ok(())
}

#[inline]
pub fn wrapping_sub_offset(amount: isize, target: &mut isize) -> Result<(), Never> {
  *target = target.wrapping_sub(amount);
  Ok(())
}

#[inline]
pub fn wrapping_mul_offset(amount: isize, target: &mut isize) -> Result<(), Never> {
  *target = target.wrapping_mul(amount);
  Ok(())
}
