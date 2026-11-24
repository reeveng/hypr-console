//! Operators share a portable SIMD implementation and a lane-wise fallback.

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "keeps scalar and portable SIMD implementations together for each lane operator"
  )
)]
macro_rules! lane_operator {
  ($vector:ident, $trait:ident, $op:ident, $symbol:tt, $scalar:expr) => {
    impl core::ops::$trait for $vector {
      type Output = Self;

      #[inline]
      fn $op(self, rhs: Self) -> Self {
        #[cfg(feature = "simd")]
        let result = (core::simd::Simd::from_array(self.0) $symbol
          core::simd::Simd::from_array(rhs.0)).to_array();
        #[cfg(not(feature = "simd"))]
        let result = super::lanes::combine(self.0, rhs.0, $scalar);

        Self(result)
      }
    }
  };
}

pub(super) use lane_operator;

#[cfg(not(feature = "simd"))]
#[inline]
pub(super) fn combine<T: Copy, U: Copy, const LANES: usize>(
  left: [T; LANES],
  right: [T; LANES],
  operation: impl Fn(T, T) -> U,
) -> [U; LANES] {
  core::array::from_fn(|lane| operation(left[lane], right[lane]))
}
