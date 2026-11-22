use crate::LengthU32;

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

pub const LENGTH_U32_ONE: LengthU32 = LengthU32::MIN;

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float is the dimensionless exponent of a transfer curve"
  )
)]
pub struct GammaExponent(pub f32);

pub fn approx_powf(
  sample: console_core_paths::NormalizedF32,
  GammaExponent(y): GammaExponent,
) -> core::result::Result<f32, console_core_never::Never> {
  use console_core_paths::StrictNum;

  let x = sample.get();

  match ::core::primitive::bool::then_some(
    ::core::matches!(
      ::core::primitive::f32::classify(x),
      ::core::num::FpCategory::Zero
    ) || ::core::primitive::f32::to_bits(x) == ::core::primitive::f32::to_bits(1.0),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(x);
    }
    ::core::option::Option::None => {}
  }

  let e = (match ::console_core_number_conversion::nearest_f32_from_u64(
    <::core::primitive::u64 as ::core::convert::From<::core::primitive::u32>>::from(x.to_bits()),
  ) {
    ::core::result::Result::Ok(_converted_number) => _converted_number,
  }) * (1.0f32
    / match ::console_core_number_conversion::nearest_f32_from_i64(
      <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
        i32::wrapping_shl(1, 23_u32),
      ),
    ) {
      ::core::result::Result::Ok(_converted_number) => _converted_number,
    });
  let m = f32::from_bits((x.to_bits() & 0x007fffff) | 0x3f000000);

  let log2_x = e - 124.225_52_f32 - 1.498_030_3_f32 * m - 1.725_88_f32 / (0.352_088_72_f32 + m);

  let x_2 = log2_x * y;

  let f = x_2 - x_2.floor();

  let mut a = x_2 + 121.274_055_f32;
  a -= f * 1.490_129_1_f32;
  a += 27.728_024_f32 / (4.842_525_5_f32 - f);
  a *= match ::console_core_number_conversion::nearest_f32_from_i64(
    <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(
      i32::wrapping_shl(1, 23_u32),
    ),
  ) {
    ::core::result::Result::Ok(_converted_number) => _converted_number,
  };

  Ok(
    match ::core::primitive::bool::then_some(
      a < match ::console_core_number_conversion::nearest_f32_from_u64(
        <::core::primitive::u64 as ::core::convert::From<::core::primitive::u32>>::from(
          f32::INFINITY.to_bits(),
        ),
      ) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      (),
    ) {
      ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(a > 0.0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(round) = console_core_number_conversion::toward_zero_u32(f64::from(a.round()));

          f32::from_bits(round)
        }
        ::core::option::Option::None => 0.0,
      },
      ::core::option::Option::None => f32::INFINITY,
    },
  )
}
