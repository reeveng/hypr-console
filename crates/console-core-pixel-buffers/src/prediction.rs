//! Predictors defined in terms of neighbouring samples.
//! Paeth ties favour left, then above, then upper left.

use console_core_never::Never;
use core::cmp::Ordering;

#[inline]
pub fn paeth<T: Copy + Into<u16>>(samples: [T; 3]) -> Result<T, Never> {
  let [left, above, upper_left] = samples;
  let [a, b, c] = samples.map(|sample| {
    i32::from(<_ as ::core::convert::Into<::core::primitive::u16>>::into(
      sample,
    ))
  });
  let guess = a.wrapping_add(b).wrapping_sub(c);
  let from_left = guess.abs_diff(a);
  let from_above = guess.abs_diff(b);
  let from_corner = guess.abs_diff(c);

  let (edge_distance, edge) = match from_left.cmp(&from_above) {
    Ordering::Less | Ordering::Equal => (from_left, left),
    Ordering::Greater => (from_above, above),
  };

  Ok(match edge_distance.cmp(&from_corner) {
    Ordering::Less | Ordering::Equal => edge,
    Ordering::Greater => upper_left,
  })
}

#[cfg(test)]
mod tests {
  use super::paeth;

  #[test]
  fn ties_choose_left_then_above_at_high_bit_depth() {
    assert_eq!(paeth([2000u16, 3500, 2500]), Ok(3500));
    assert_eq!(paeth([4000u16, 1000, 3000]), Ok(1000));
    assert_eq!(paeth([2000u16, 3500, 3000]), Ok(2000));
    assert_eq!(paeth([1000u16, 3500, 2500]), Ok(2500));
  }
}
