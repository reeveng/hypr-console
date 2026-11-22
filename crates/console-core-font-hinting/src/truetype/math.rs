//! Fixed-point math for TrueType instructions. Integer narrowing keeps low bits.
//! Vector normalization performs at most 32 refinement steps and uses the last approximation.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use core::num::Wrapping;
use read_fonts::types::{Fixed, Point};

pub struct Padding {
  pub value: i32,
  pub quantum: i32,
}

pub struct Product {
  pub left: i32,
  pub right: i32,
}

pub struct Ratio {
  pub numerator: i32,
  pub denominator: i32,
}

pub struct ProductRatio {
  pub value: i32,
  pub multiplier: i32,
  pub divisor: i32,
}

fn low_i32(value: i64) -> Result<i32, Never> {
  let [a, b, c, d, _, _, _, _] = value.to_le_bytes();
  Ok(i32::from_le_bytes([a, b, c, d]))
}

fn bits_i32(value: u32) -> Result<i32, Never> {
  Ok(i32::from_le_bytes(value.to_le_bytes()))
}

fn bits_u32(value: i32) -> Result<u32, Never> {
  Ok(u32::from_le_bytes(value.to_le_bytes()))
}

pub fn floor(value: i32) -> Result<i32, Never> {
  Ok(value & !63)
}

pub fn round(value: i32) -> Result<i32, Never> {
  floor(value.wrapping_add(32))
}

pub fn ceil(value: i32) -> Result<i32, Never> {
  floor(value.wrapping_add(63))
}

fn floor_pad(input: Padding) -> Result<i32, Never> {
  Ok(input.value & !input.quantum.wrapping_sub(1))
}

pub fn round_pad(input: Padding) -> Result<i32, Never> {
  floor_pad(Padding {
    value: input.value.wrapping_add(input.quantum.wrapping_div(2)),
    quantum: input.quantum,
  })
}

pub fn mul(input: Product) -> Result<i32, Never> {
  let Ok(value) = ({
    let Ok(value) = Fixed::from_bits(input.left);
    value
  } * {
    let Ok(value) = Fixed::from_bits(input.right);
    value
  })
  .to_bits();

  Ok(value)
}

pub fn div(input: Ratio) -> Result<i32, Never> {
  let Ok(value) = ({
    let Ok(value) = Fixed::from_bits(input.numerator);
    value
  } / {
    let Ok(value) = Fixed::from_bits(input.denominator);
    value
  })
  .to_bits();

  Ok(value)
}

pub fn mul_div(input: ProductRatio) -> Result<i32, Never> {
  let Ok(value) = {
    let Ok(value) = {
      let Ok(value) = Fixed::from_bits(input.value);
      value
    }
    .mul_div(
      {
        let Ok(value) = Fixed::from_bits(input.multiplier);
        value
      },
      {
        let Ok(value) = Fixed::from_bits(input.divisor);
        value
      },
    );
    value
  }
  .to_bits();

  Ok(value)
}

pub fn mul_div_no_round(input: ProductRatio) -> Result<i32, Never> {
  let negative = (input.value < 0) ^ (input.multiplier < 0) ^ (input.divisor < 0);
  let product = u64::from(input.value.unsigned_abs())
    .wrapping_mul(u64::from(input.multiplier.unsigned_abs()));
  let magnitude = match product.checked_div(u64::from(input.divisor.unsigned_abs())) {
    Some(quotient) => quotient,
    None => u64::from(i32::MAX.unsigned_abs()),
  };
  let Ok(wide) = fitted::<_, i64>(magnitude);
  let Ok(result) = low_i32(wide);
  Ok(match ::core::primitive::bool::then_some(negative, ()) {
    ::core::option::Option::Some(()) => result.wrapping_neg(),
    ::core::option::Option::None => result,
  })
}

pub fn mul14(input: Product) -> Result<i32, Never> {
  let product = i64::from(input.left).wrapping_mul(i64::from(input.right));
  let bias = 0x2000i64.wrapping_add(product.wrapping_shr(63));
  low_i32(product.wrapping_add(bias).wrapping_shr(14))
}

pub fn normalize14(point: Point<i32>) -> Result<Point<i32>, Never> {
  let (mut sx, mut sy) = (Wrapping(1i32), Wrapping(1i32));
  let Ok(x_bits) = bits_u32(point.x);
  let Ok(y_bits) = bits_u32(point.y);
  let mut ux = Wrapping(x_bits);
  let mut uy = Wrapping(y_bits);
  const ZERO: Wrapping<u32> = Wrapping(0);
  let mut result = Point::default();

  match ::core::primitive::bool::then_some(point.x < 0, ()) {
    ::core::option::Option::Some(()) => {
      ux = ZERO - ux;
      sx = -sx;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(point.y < 0, ()) {
    ::core::option::Option::Some(()) => {
      uy = ZERO - uy;
      sy = -sy;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(ux == ZERO, ()) {
    ::core::option::Option::Some(()) => {
      result.x = point.x.wrapping_div(4);

      match ::core::primitive::bool::then_some(uy.0 > 0, ()) {
        ::core::option::Option::Some(()) => result.y = (sy * Wrapping(0x10000) / Wrapping(4)).0,
        ::core::option::Option::None => {}
      }

      return Ok(result);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(uy == ZERO, ()) {
    ::core::option::Option::Some(()) => {
      result.y = point.y.wrapping_div(4);

      match ::core::primitive::bool::then_some(ux.0 > 0, ()) {
        ::core::option::Option::Some(()) => result.x = (sx * Wrapping(0x10000) / Wrapping(4)).0,
        ::core::option::Option::None => {}
      }

      return Ok(result);
    }
    ::core::option::Option::None => {}
  }

  let mut length = match ::core::primitive::bool::then_some(ux > uy, ()) {
    ::core::option::Option::Some(()) => ux + (uy >> 1),
    ::core::option::Option::None => uy + (ux >> 1),
  };
  let Ok(leading) = fitted::<_, i32>(length.0.leading_zeros());
  let mut shift = Wrapping(leading);
  let Ok(initial_shift) = index(leading);
  shift -= Wrapping(15)
    + match ::core::primitive::bool::then_some(length >= (Wrapping(0xAAAAAAAAu32) >> initial_shift), ()) {
      ::core::option::Option::Some(()) => Wrapping(1),
      ::core::option::Option::None => Wrapping(0),
    };

  match ::core::primitive::bool::then_some(shift.0 > 0, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(amount) = index(shift.0);
      ux <<= amount;
      uy <<= amount;
      length = match ::core::primitive::bool::then_some(ux > uy, ()) {
        ::core::option::Option::Some(()) => ux + (uy >> 1),
        ::core::option::Option::None => uy + (ux >> 1),
      };
    }
    ::core::option::Option::None => {
      let Ok(amount) = index(shift.0.unsigned_abs());
      ux >>= amount;
      uy >>= amount;
      length >>= amount;
    }
  }

  let Ok(length_bits) = bits_i32(length.0);
  let mut correction = Wrapping(0x10000) - Wrapping(length_bits);
  let Ok(horizontal_bits) = bits_i32(ux.0);
  let Ok(vertical_bits) = bits_i32(uy.0);
  let fixed_x = Wrapping(horizontal_bits);
  let fixed_y = Wrapping(vertical_bits);
  let (mut final_x, mut final_y) = (ux, uy);

  for _ in 0..u32::BITS {
    let Ok(horizontal) = bits_u32((fixed_x + ((fixed_x * correction) >> 16)).0);
    let Ok(vertical) = bits_u32((fixed_y + ((fixed_y * correction) >> 16)).0);
    final_x = Wrapping(horizontal);
    final_y = Wrapping(vertical);
    let Ok(squared) = bits_i32((final_x * final_x + final_y * final_y).0);
    let mut delta = Wrapping(squared.wrapping_neg()) / Wrapping(0x200);
    delta = delta * ((Wrapping(0x10000) + correction) >> 8) / Wrapping(0x10000);
    correction += delta;

    match ::core::primitive::bool::then_some(delta <= Wrapping(0), ()) {
      ::core::option::Option::Some(()) => break,
      ::core::option::Option::None => {}
    }
  }

  let Ok(horizontal) = bits_i32(final_x.0);
  let Ok(vertical) = bits_i32(final_y.0);
  Ok({
    let Ok(value) = Point::new(
      (Wrapping(horizontal) * sx / Wrapping(4)).0,
      (Wrapping(vertical) * sy / Wrapping(4)).0,
    );
    value
  })
}

#[cfg(test)]
mod tests {
  use raw::types::{F2Dot14, Fixed};

  const FLOAT_TOLERANCE: f32 = 1e-4;

  #[test]
  fn mul_div_no_round() {
    let cases = [
      ((-326, -11474, 9942), 376),
      ((-6781, 13948, 11973), -7899),
      ((3517, 15622, 8075), 6804),
      ((-6127, 15026, 2276), -40450),
      ((11257, 14828, 2542), 65664),
      ((-12797, -16280, -9086), -22929),
      ((-7994, -3340, 9583), 2786),
      ((-16101, -13780, -1427), -155481),
      ((10304, -16331, 15480), -10870),
      ((-15879, 11912, -4650), 40677),
      ((-5015, 6382, -15977), 2003),
      ((2080, -11930, -15457), 1605),
      ((-11071, 13350, 16138), -9158),
      ((16084, -13564, -770), 283329),
      ((14304, -10377, -21), 7068219),
      ((-14056, -8853, -5488), -22674),
      ((-10319, 14797, 8554), -17850),
      ((-7820, 6826, 10555), -5057),
      ((7257, 15928, 8159), 14167),
      ((14929, 11579, -13204), -13091),
      ((2808, 12070, -14697), -2306),
      ((-13818, 8544, -1649), 71595),
      ((3265, 7325, -1373), -17418),
      ((14832, 10586, -6440), -24380),
      ((4123, 8274, -2022), -16871),
      ((4645, -4149, -7242), 2661),
      ((-3891, 8366, 5771), -5640),
      ((-15447, -3428, -9335), -5672),
      ((13670, -14311, -11122), 17589),
      ((12590, -6592, 13159), -6306),
      ((-8369, -10193, 5051), 16888),
      ((-9539, 5167, 2595), -18993),
    ];
    for ((a, b, c), expected_result) in cases {
      let Ok(result) = super::mul_div_no_round(super::ProductRatio {
        value: a,
        multiplier: b,
        divisor: c,
      });
      assert_eq!(result, expected_result);
      let Ok(fa) = ({
        let Ok(value) = Fixed::from_bits(a);
        value
      })
      .to_f32();
      let Ok(fb) = ({
        let Ok(value) = Fixed::from_bits(b);
        value
      })
      .to_f32();
      let Ok(fc) = ({
        let Ok(value) = Fixed::from_bits(c);
        value
      })
      .to_f32();
      let fresult = fa * fb / fc;
      let Ok(fexpected_result) = ({
        let Ok(value) = Fixed::from_bits(expected_result);
        value
      })
      .to_f32();
      assert!((fresult - fexpected_result).abs() < FLOAT_TOLERANCE);
    }
  }

  #[test]
  fn mul14() {
    let cases = [
      ((6236, -10078), -3836),
      ((-6803, -5405), 2244),
      ((-10006, -12852), 7849),
      ((-15434, -4102), 3864),
      ((-8681, 9269), -4911),
      ((9449, -9130), -5265),
      ((12643, 2161), 1668),
      ((-6115, 9284), -3465),
      ((316, 3390), 65),
      ((15077, -12901), -11872),
      ((-12182, 11613), -8635),
      ((-7213, 8246), -3630),
      ((13482, 8096), 6662),
      ((5690, 15016), 5215),
      ((-5991, 12613), -4612),
      ((13112, -8404), -6726),
      ((13524, 6786), 5601),
      ((7156, 3291), 1437),
      ((-2978, 353), -64),
      ((-1755, 14626), -1567),
      ((14402, 7886), 6932),
      ((7124, 15730), 6840),
      ((-12679, 14830), -11476),
      ((-9374, -12999), 7437),
      ((12301, -4685), -3517),
      ((5324, 2066), 671),
      ((6783, -4946), -2048),
      ((12078, -968), -714),
      ((-10137, 14116), -8734),
      ((-13946, 11585), -9861),
      ((-678, -2205), 91),
      ((-2629, -3319), 533),
    ];
    for ((a, b), expected_result) in cases {
      let Ok(result) = super::mul14(super::Product { left: a, right: b });
      assert_eq!(result, expected_result);
      let Ok(a_bits) = console_core_number_conversion::fitted::<_, i16>(a);
      let Ok(fa) = ({
        let Ok(value) = F2Dot14::from_bits(a_bits);
        value
      })
      .to_f32();
      let Ok(b_bits) = console_core_number_conversion::fitted::<_, i16>(b);
      let Ok(fb) = ({
        let Ok(value) = F2Dot14::from_bits(b_bits);
        value
      })
      .to_f32();
      let fresult = fa * fb;
      let Ok(expected_result_bits) =
        console_core_number_conversion::fitted::<_, i16>(expected_result);
      let Ok(fexpected_result) = ({
        let Ok(value) = F2Dot14::from_bits(expected_result_bits);
        value
      })
      .to_f32();
      assert!((fresult - fexpected_result).abs() < FLOAT_TOLERANCE);
    }
  }

  #[test]
  fn normalize14() {
    let cases = [
      ((-13660, 11807), (-12395, 10713)),
      ((-10763, 9293), (-12401, 10707)),
      ((-3673, 673), (-16115, 2952)),
      ((15886, -2964), (16106, -3005)),
      ((15442, -2871), (16108, -2994)),
      ((-6308, 5744), (-12114, 11031)),
      ((9410, -10415), (10983, -12156)),
      ((-10620, -14856), (-9528, -13328)),
      ((-9372, 12029), (-10069, 12924)),
      ((-1272, -1261), (-11635, -11534)),
      ((-7076, -5517), (-12920, -10074)),
      ((-10297, 179), (-16381, 284)),
      ((9256, -13235), (9389, -13426)),
      ((5315, -12449), (6433, -15068)),
      ((8064, 15213), (7673, 14476)),
      ((-8665, 41), (-16383, 77)),
      ((-3455, -4720), (-9677, -13220)),
      ((13449, -5152), (15299, -5861)),
      ((-15605, 8230), (-14492, 7643)),
      ((4716, -13690), (5336, -15490)),
      ((12904, -11422), (12268, -10859)),
      ((2825, -6396), (6619, -14987)),
      ((4654, 15245), (4783, 15670)),
      ((-14769, 15133), (-11443, 11725)),
      ((-8090, -9057), (-10914, -12219)),
      ((-472, 1953), (-3848, 15925)),
      ((-12563, 1040), (-16328, 1351)),
      ((-7938, 15587), (-7435, 14599)),
      ((-9701, 5356), (-14343, 7919)),
      ((-642, -14484), (-725, -16367)),
      ((12963, -9690), (13123, -9809)),
      ((7067, 5361), (13053, 9902)),
      ((0x4000, 0), (0x4000, 0)),
      ((0, 0x4000), (0, 0x4000)),
      ((-0x4000, 0), (-0x4000, 0)),
      ((0, -0x4000), (0, -0x4000)),
    ];
    for ((x, y), expected) in cases {
      let Ok(n) = super::normalize14(read_fonts::types::Point { x, y });
      assert_eq!((n.x, n.y), expected);

      let Ok(nx) = console_core_number_conversion::fitted::<_, i16>(n.x);
      let Ok(fx) = ({
        let Ok(value) = F2Dot14::from_bits(nx);
        value
      })
      .to_f32();
      let Ok(ny) = console_core_number_conversion::fitted::<_, i16>(n.y);
      let Ok(fy) = ({
        let Ok(value) = F2Dot14::from_bits(ny);
        value
      })
      .to_f32();
      let flen = (fx * fx + fy * fy).sqrt();
      assert!((flen - 1.0).abs() <= FLOAT_TOLERANCE);
    }
  }
}
