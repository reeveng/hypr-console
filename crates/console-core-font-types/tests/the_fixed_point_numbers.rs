use console_core_font_types::{F2Dot14, F4Dot12, F6Dot10, F26Dot6, F48Dot16, Fixed};
use console_core_never::Never;
use console_core_number_conversion::{narrow_f32, toward_zero_i32};

fn low_half(value: i32) -> Result<i16, Never> {
  let [_, _, high, low] = value.to_be_bytes();

  Ok(i16::from_be_bytes([high, low]))
}

#[test]
fn fixed_floor_fract_abs_and_round_meet_the_negatives() {
  let cases: [(i32, i32, i32, i32, i32); 6] = [
    (
      0x0001_8000,
      0x0001_0000,
      0x0000_8000,
      0x0001_8000,
      0x0002_0000,
    ),
    (
      -0x0001_8000,
      -0x0002_0000,
      0x0000_8000,
      0x0001_8000,
      -0x0001_0000,
    ),
    (-1, -0x0001_0000, 0x0000_ffff, 1, 0),
    (i32::MIN, i32::MIN, 0, i32::MIN, i32::MIN),
    (i32::MAX, 0x7fff_0000, 0x0000_ffff, i32::MAX, i32::MIN),
    (0x0000_7fff, 0, 0x0000_7fff, 0x0000_7fff, 0),
  ];

  for (bits, floor, fract, abs, round) in cases {
    let value = Fixed::from_bits(bits);

    assert_eq!(
      value.and_then(Fixed::floor).and_then(Fixed::to_bits),
      Ok(floor),
      "floor of {bits:#x}"
    );
    assert_eq!(
      value.and_then(Fixed::fract).and_then(Fixed::to_bits),
      Ok(fract),
      "fract of {bits:#x}"
    );
    assert_eq!(
      value.and_then(Fixed::abs).and_then(Fixed::to_bits),
      Ok(abs),
      "abs of {bits:#x}"
    );
    assert_eq!(
      value.and_then(Fixed::round).and_then(Fixed::to_bits),
      Ok(round),
      "round of {bits:#x}"
    );
  }
}

#[test]
fn every_two_dot_fourteen_floors_rounds_and_splits_the_same_way() {
  for bits in i16::MIN..=i16::MAX {
    let value = F2Dot14::from_bits(bits);
    let whole = bits & !0x3fff;

    assert_eq!(
      value.and_then(F2Dot14::floor).and_then(F2Dot14::to_bits),
      Ok(whole)
    );
    assert_eq!(
      value.and_then(F2Dot14::fract).and_then(F2Dot14::to_bits),
      Ok(bits.wrapping_sub(whole))
    );
    assert_eq!(
      value.and_then(F2Dot14::round).and_then(F2Dot14::to_bits),
      Ok(bits.wrapping_add(0x2000) & !0x3fff)
    );
    assert_eq!(
      value.and_then(F2Dot14::abs).and_then(F2Dot14::to_bits),
      Ok(bits.wrapping_abs())
    );
    assert_eq!(
      value.map(|held| -held).and_then(F2Dot14::to_bits),
      Ok(bits.wrapping_neg())
    );
    assert_eq!(
      value.and_then(F2Dot14::to_f32).and_then(F2Dot14::from_f32),
      value
    );
  }
}

#[test]
fn fixed_adds_and_subtracts_by_the_policy_its_name_says() {
  let Ok(two) = Fixed::from_i32(2);
  let Ok(three) = Fixed::from_i32(3);
  let Ok(four) = Fixed::from_i32(4);
  let Ok(minus_two) = Fixed::from_i32(-2);
  let Ok(minus_four) = Fixed::from_i32(-4);

  assert_eq!(Fixed::MAX.wrapping_add(Fixed::EPSILON), Ok(Fixed::MIN));
  assert_eq!(Fixed::MAX.saturating_add(Fixed::EPSILON), Ok(Fixed::MAX));
  assert_eq!(Fixed::MAX.checked_add(Fixed::EPSILON), Ok(None));
  assert_eq!(Fixed::ONE.checked_add(Fixed::ONE), Ok(Some(two)));
  assert_eq!(Fixed::MIN.wrapping_sub(Fixed::EPSILON), Ok(Fixed::MAX));
  assert_eq!(Fixed::MIN.saturating_sub(Fixed::EPSILON), Ok(Fixed::MIN));
  assert_eq!(Fixed::MAX + Fixed::EPSILON, Fixed::MIN);
  assert_eq!(Fixed::MIN - Fixed::EPSILON, Fixed::MAX);
  assert_eq!(-Fixed::MIN, Fixed::MIN);

  let mut held = Fixed::ONE;

  held += Fixed::ONE;
  assert_eq!(held, two);
  held -= three;
  assert_eq!(held, Fixed::NEG_ONE);
  held *= four;
  assert_eq!(held, minus_four);
  held /= minus_two;
  assert_eq!(held, two);
}

#[test]
fn the_constants_are_the_values_they_name() {
  assert_eq!(F2Dot14::ONE.to_bits(), Ok(0x4000));
  assert_eq!(F2Dot14::NEG_ONE.to_bits(), Ok(-0x4000));
  assert_eq!(F4Dot12::ONE.to_bits(), Ok(0x1000));
  assert_eq!(F4Dot12::NEG_ONE.to_bits(), Ok(-0x1000));
  assert_eq!(F6Dot10::ONE.to_bits(), Ok(0x0400));
  assert_eq!(F6Dot10::NEG_ONE.to_bits(), Ok(-0x0400));
  assert_eq!(Fixed::ONE.to_bits(), Ok(0x0001_0000));
  assert_eq!(Fixed::NEG_ONE.to_bits(), Ok(-0x0001_0000));
  assert_eq!(F26Dot6::ONE.to_bits(), Ok(64));
  assert_eq!(F26Dot6::NEG_ONE.to_bits(), Ok(-64));
  assert_eq!(F48Dot16::ONE.to_bits(), Ok(0x0001_0000));
  assert_eq!(F48Dot16::NEG_ONE.to_bits(), Ok(-0x0001_0000));
  assert_eq!(F48Dot16::MIN.to_bits(), Ok(i64::MIN));
  assert_eq!(F48Dot16::EPSILON.to_bits(), Ok(1));
  assert_eq!(F2Dot14::EPSILON.to_bits(), Ok(1));
  assert_eq!(F2Dot14::ZERO.to_bits(), Ok(0));
  assert_eq!(Fixed::default(), Fixed::ZERO);
  assert_eq!(Fixed::ONE.to_be_bytes(), Ok([0, 1, 0, 0]));
  assert_eq!(F2Dot14::NEG_ONE.to_be_bytes(), Ok([0xc0, 0]));
  assert_eq!(F48Dot16::ONE.to_be_bytes(), Ok([0, 0, 0, 0, 0, 1, 0, 0]));
  assert_eq!(
    F26Dot6::from_bits(-1).and_then(F26Dot6::to_be_bytes),
    Ok([0xff; 4])
  );
}

#[test]
fn fixed_converts_to_its_neighbours() {
  let cases: [i32; 10] = [
    0,
    1,
    -1,
    2,
    -2,
    0x0001_8000,
    -0x0001_8000,
    i32::MAX,
    i32::MIN,
    0x0123_4567,
  ];

  for bits in cases {
    let Ok(value) = Fixed::from_bits(bits);
    let Ok(as_float) = narrow_f32(f64::from(bits));

    assert_eq!(
      value.to_f26dot6().and_then(F26Dot6::to_bits),
      Ok(bits.wrapping_add(0x200).wrapping_shr(10))
    );
    assert_eq!(
      value.to_f2dot14().and_then(F2Dot14::to_bits),
      low_half(bits.wrapping_add(2).wrapping_shr(2))
    );
    assert_eq!(
      value.to_i32(),
      Ok(bits.wrapping_add(0x8000).wrapping_shr(16))
    );
    assert_eq!(
      value.to_f48dot16().and_then(F48Dot16::to_bits),
      Ok(i64::from(bits))
    );
    assert_eq!(value.to_f32(), Ok(as_float / 65536.0));
    assert_eq!(value.to_f48dot16().and_then(F48Dot16::to_fixed), Ok(value));
  }

  assert_eq!(Ok(Fixed::from(7)), Fixed::from_i32(7));
  assert_eq!(
    Fixed::from_i32(-3).and_then(Fixed::to_bits),
    Ok(-0x0003_0000)
  );
  assert_eq!(
    Fixed::from_i32(1000).and_then(Fixed::to_bits),
    Ok(0x03e8_0000)
  );
  assert_eq!(
    Fixed::from_i32(0x8000).and_then(Fixed::to_bits),
    Ok(i32::MIN)
  );
  assert_eq!(F26Dot6::from_i32(-3).and_then(F26Dot6::to_bits), Ok(-192));
  assert_eq!(F26Dot6::from_bits(-33).and_then(F26Dot6::to_i32), Ok(-1));
  assert_eq!(F26Dot6::from_bits(-32).and_then(F26Dot6::to_i32), Ok(0));
  assert_eq!(F26Dot6::from_bits(-96).and_then(F26Dot6::to_f32), Ok(-1.5));
  assert_eq!(
    F2Dot14::from_bits(-0x6000).and_then(F2Dot14::to_fixed),
    Fixed::from_bits(-0x0001_8000)
  );
  assert_eq!(
    Fixed::from_f64(42.5).and_then(Fixed::to_f26dot6),
    F26Dot6::from_f64(42.5)
  );
}

#[test]
fn forty_eight_dot_sixteen_holds_what_sixteen_dot_sixteen_cannot() {
  let wide = 1_099_511_627_776.5;

  assert_eq!(
    F48Dot16::from_bits(0x0001_0000_0000_8000).and_then(F48Dot16::to_i32),
    Ok(1)
  );
  assert_eq!(
    F48Dot16::from_bits(-0x0001_8001).and_then(F48Dot16::to_i32),
    Ok(-2)
  );
  assert_eq!(
    F48Dot16::from_i32(-5).and_then(F48Dot16::to_bits),
    Ok(-0x0005_0000)
  );
  assert_eq!(
    F48Dot16::from_bits(0x0001_8000).and_then(F48Dot16::to_f32),
    Ok(1.5)
  );
  assert_eq!(
    F48Dot16::from_bits(i64::MIN).and_then(F48Dot16::to_f64),
    Ok(-140_737_488_355_328.0)
  );
  assert_eq!(
    F48Dot16::from_bits(-1).and_then(F48Dot16::to_f64),
    Ok(-1.0 / 65536.0)
  );
  assert_eq!(F48Dot16::from_f64(1.5), F48Dot16::from_bits(0x0001_8000));
  assert_eq!(F48Dot16::from_f64(-1.5), F48Dot16::from_bits(-0x0001_8000));
  assert_eq!(
    F48Dot16::from_f64(1_099_511_627_776.0),
    F48Dot16::from_bits(0x0100_0000_0000_0000)
  );
  assert_eq!(
    F48Dot16::from_f64(wide).and_then(F48Dot16::to_f64),
    Ok(wide)
  );
  assert_eq!(
    F48Dot16::from_bits(0x0001_7fff).and_then(F48Dot16::round),
    F48Dot16::from_bits(0x0001_0000)
  );
  assert_eq!(
    F48Dot16::from_bits(0x0001_8000).and_then(F48Dot16::round),
    F48Dot16::from_bits(0x0002_0000)
  );
  assert_eq!(
    F48Dot16::from_f64(-1.5).and_then(F48Dot16::round),
    F48Dot16::from_f64(-1.0)
  );
  assert_eq!(F48Dot16::from_f64(1.5).and_then(F48Dot16::to_i64), Ok(2));
  assert_eq!(F48Dot16::from_f64(-1.5).and_then(F48Dot16::to_i64), Ok(-1));
  assert_eq!(
    F48Dot16::from_f64(1_099_511_627_776.25).and_then(F48Dot16::to_i64),
    Ok(1_099_511_627_776)
  );
  assert_eq!(
    F48Dot16::from_i64(0x0100_0000_0000).and_then(F48Dot16::to_bits),
    Ok(0x0100_0000_0000_0000)
  );
  assert_eq!(
    F48Dot16::from_bits(0x8000_0000).and_then(F48Dot16::to_fixed),
    Fixed::from_bits(i32::MIN)
  );
  assert_eq!(
    F48Dot16::from_bits(-0x8000_0001).and_then(F48Dot16::to_fixed),
    Fixed::from_bits(i32::MAX)
  );
}

#[test]
fn floats_are_rounded_and_clamped_in_fixed_16_16() {
  assert_eq!(Fixed::from_f64(f64::NAN), Ok(Fixed::ZERO));
  assert_eq!(Fixed::from_f64(f64::INFINITY), Ok(Fixed::MAX));
  assert_eq!(Fixed::from_f64(f64::NEG_INFINITY), Ok(Fixed::MIN));
  assert_eq!(Fixed::from_f64(-40000.0), Ok(Fixed::MIN));
  assert_eq!(Fixed::from_f64(32768.0), Ok(Fixed::MAX));
  assert_eq!(
    Fixed::from_f64(0.5 / 65536.0).and_then(Fixed::to_bits),
    Ok(1)
  );
  assert_eq!(
    Fixed::from_f64(-0.5 / 65536.0).and_then(Fixed::to_bits),
    Ok(-1)
  );
  assert_eq!(
    Fixed::from_f64(0.49 / 65536.0).and_then(Fixed::to_bits),
    Ok(0)
  );
  assert_eq!(
    Fixed::from_f64(32767.0).and_then(Fixed::to_bits),
    Ok(0x7fff_0000)
  );
  assert_eq!(
    Fixed::from_f64(28_672.000_015_258_79).and_then(Fixed::to_bits),
    Ok(0x7000_0001)
  );
  assert_eq!(
    Fixed::from_f64(-0.000_015_259).and_then(Fixed::to_bits),
    Ok(-1)
  );

  for input in [0.05, 0.6, 0.2, 0.4, 0.677_55] {
    assert_eq!(
      Fixed::from_f64(input).and_then(Fixed::to_bits),
      toward_zero_i32((input * 65536.0).round())
    );
    assert_eq!(
      Fixed::from_f64(-input).and_then(Fixed::to_bits),
      toward_zero_i32((-input * 65536.0).round())
    );
  }
}

#[test]
fn floats_are_rounded_and_clamped_in_the_other_fixed_point_formats() {
  assert_eq!(F2Dot14::from_f32(f32::NAN), Ok(F2Dot14::ZERO));
  assert_eq!(F2Dot14::from_f32(5.0), Ok(F2Dot14::MAX));
  assert_eq!(F2Dot14::from_f32(-5.0), Ok(F2Dot14::MIN));
  assert_eq!(
    F2Dot14::from_f32(1.999_939).and_then(F2Dot14::to_bits),
    Ok(0x7fff)
  );
  assert_eq!(
    F2Dot14::from_f32(1.75).and_then(F2Dot14::to_bits),
    Ok(0x7000)
  );
  assert_eq!(
    F2Dot14::from_f32(0.000_061_035_6).and_then(F2Dot14::to_bits),
    Ok(1)
  );
  assert_eq!(
    F2Dot14::from_f32(-0.000_061).and_then(F2Dot14::to_bits),
    Ok(-1)
  );
  assert_eq!(
    F2Dot14::from_f32(-2.0).and_then(F2Dot14::to_bits),
    Ok(i16::MIN)
  );
  assert_eq!(F2Dot14::from_f64(1.5).and_then(F2Dot14::to_f64), Ok(1.5));
  assert_eq!(F4Dot12::from_f32(-8.5), Ok(F4Dot12::MIN));
  assert_eq!(F6Dot10::from_f64(100.0), Ok(F6Dot10::MAX));
  assert_eq!(
    F26Dot6::from_f64(-0.5 / 64.0).and_then(F26Dot6::to_bits),
    Ok(-1)
  );
  assert_eq!(F48Dot16::from_f64(1e300), Ok(F48Dot16::MAX));
  assert_eq!(F48Dot16::from_f64(-1e300), Ok(F48Dot16::MIN));
  assert_eq!(F48Dot16::from_f64(f64::NAN), Ok(F48Dot16::ZERO));
}

#[test]
fn fixed_multiplies_and_divides_with_rounding() {
  let cases: [(i32, i32); 12] = [
    (0x0001_8000, 0x0001_8000),
    (-0x0001_8000, 0x0001_8000),
    (-0x0001_8000, -0x0001_8000),
    (3, 0x8000),
    (-3, 0x8000),
    (1, 1),
    (-1, 1),
    (i32::MAX, i32::MAX),
    (i32::MIN, i32::MIN),
    (i32::MIN, -1),
    (0x0123_4567, -0x0765_4321),
    (0, 5),
  ];

  for (a, b) in cases {
    let Ok(fixed) =
      Fixed::from_bits(a).and_then(|left| Fixed::from_bits(b).map(|right| left * right));
    let Ok(dot_six) =
      F26Dot6::from_bits(a).and_then(|left| F26Dot6::from_bits(b).map(|right| left * right));

    assert_eq!(
      fixed.to_bits(),
      rounded_product([a, b], 16),
      "{a:#x} * {b:#x}"
    );
    assert_eq!(
      dot_six.to_bits(),
      rounded_product([a, b], 6),
      "{a:#x} * {b:#x}"
    );
  }
}

fn rounded_product([a, b]: [i32; 2], fract_bits: u32) -> Result<i32, Never> {
  let product = i64::from(a).wrapping_mul(i64::from(b));
  let half = 1i64.wrapping_shl(fract_bits.wrapping_sub(1));
  let toward_zero = i64::from(product < 0);
  let [_, _, _, _, first, second, third, fourth] = product
    .wrapping_add(half)
    .wrapping_sub(toward_zero)
    .wrapping_shr(fract_bits)
    .to_be_bytes();

  Ok(i32::from_be_bytes([first, second, third, fourth]))
}

#[test]
fn fixed_divides_and_scales_without_overflowing() {
  let quotients: [(i32, i32, i32); 8] = [
    (0x0001_0000, 0x0003_0000, 0x5555),
    (-0x0001_0000, 0x0003_0000, -0x5555),
    (0x0002_0000, 0x0003_0000, 0xaaab),
    (5, 0, 0x7fff_ffff),
    (-5, 0, -0x7fff_ffff),
    (0, 0, 0x7fff_ffff),
    (i32::MIN, 0x0001_0000, i32::MIN),
    (0x7fff_0000, 0x0000_0001, 0),
  ];
  let scaled: [(i32, i32, i32, i32); 8] = [
    (0x0001_0000, 0x0002_0000, 0x0003_0000, 0xaaab),
    (-0x0001_0000, 0x0002_0000, 0x0003_0000, -0xaaab),
    (0x0001_0000, -0x0002_0000, -0x0003_0000, 0xaaab),
    (7, 3, 0, 0x7fff_ffff),
    (-7, 3, 0, -0x7fff_ffff),
    (i32::MAX, i32::MAX, 1, 1),
    (100, 200, 300, 67),
    (-0x005c_8000, 0x0001_0000, 0x0000_00b9, i32::MIN),
  ];

  for (a, b, quotient) in quotients {
    let Ok(divided) =
      Fixed::from_bits(a).and_then(|left| Fixed::from_bits(b).map(|right| left / right));

    assert_eq!(divided.to_bits(), Ok(quotient), "{a:#x} / {b:#x}");
  }

  for (value, a, b, answer) in scaled {
    let Ok(held) = Fixed::from_bits(value);
    let Ok(by) = Fixed::from_bits(a);
    let Ok(over) = Fixed::from_bits(b);

    assert_eq!(
      held.mul_div(by, over).and_then(Fixed::to_bits),
      Ok(answer),
      "{value:#x} * {a:#x} / {b:#x}"
    );
  }

  let Ok(minus) = Fixed::from_f64(-92.5);
  let Ok(small) = Fixed::from_f64(0.002_822_875_976_562_5);

  assert_eq!((minus / small).to_bits(), Ok(i32::MIN));
  assert_eq!((Fixed::MIN / Fixed::NEG_ONE).to_bits(), Ok(i32::MIN));
}

#[test]
fn twenty_six_dot_six_multiplies_and_divides_like_sixteen_dot_sixteen() {
  let Ok(half) = F26Dot6::from_f64(0.5);
  let Ok(two) = F26Dot6::from_f64(2.0);
  let Ok(three) = F26Dot6::from_f64(3.0);
  let Ok(minus) = F26Dot6::from_f64(-2.4);

  assert_eq!(Ok(half * two), F26Dot6::from_f64(1.0));
  assert_eq!(Ok(half * minus), F26Dot6::from_f64(-1.2));
  assert_eq!(Ok(half / two), F26Dot6::from_f64(0.25));
  assert_eq!(
    Ok(half / minus),
    F26Dot6::from_f64(-0.208_333_333_333_333_34)
  );
  assert_eq!(Ok(two / three), F26Dot6::from_f64(0.666_666_666_666_666_6));
  assert_eq!(F26Dot6::ONE * F26Dot6::ONE, F26Dot6::ONE);
  assert_eq!(F26Dot6::ONE / F26Dot6::ONE, F26Dot6::ONE);
  assert_eq!(
    Ok(F26Dot6::ONE / F26Dot6::ZERO),
    F26Dot6::from_bits(0x7fff_ffff)
  );
  assert_eq!(
    Ok(-F26Dot6::ONE / F26Dot6::ZERO),
    F26Dot6::from_bits(-0x7fff_ffff)
  );

  let Ok(quarter) =
    Fixed::from_f64(0.5).and_then(|half_2| Fixed::from_f64(2.0).map(|two_2| half_2 / two_2));

  assert_eq!(Ok(quarter), Fixed::from_f64(0.25));
}

#[test]
fn a_delta_moves_a_value_by_its_own_quantum() {
  let raws: [i64; 8] = [
    0,
    1,
    -1,
    100,
    -32768,
    65536,
    0x0100_0000_0000,
    -0x0100_0000_0000,
  ];
  let Ok(one_and_a_half) = Fixed::from_f64(1.5);
  let Ok(quarter) = F2Dot14::from_f32(0.25);

  for raw_bits in raws {
    let Ok(delta) = F48Dot16::from_bits(raw_bits);
    let Ok(raw) = delta.to_f64();
    let Ok(fixed_base) = one_and_a_half.to_f32();
    let Ok(two_dot_base) = quarter.to_f32();
    let Ok(fixed_step) = narrow_f32(raw * (1.0 / 65536.0));
    let Ok(two_dot_step) = narrow_f32(raw * (1.0 / 16384.0));

    assert_eq!(
      one_and_a_half.apply_delta(delta),
      Ok(fixed_base + fixed_step)
    );
    assert_eq!(quarter.apply_delta(delta), Ok(two_dot_base + two_dot_step));
  }

  let Ok(one) = F48Dot16::from_i64(1);
  let Ok(minus_half) = F48Dot16::from_f64(-0.5);
  let Ok(half) = F2Dot14::from_f32(0.5);

  assert_eq!(Fixed::ZERO.apply_delta(one), Ok(1.0 / 65536.0));
  assert_eq!(F2Dot14::ZERO.apply_delta(one), Ok(1.0 / 16384.0));
  assert_eq!(half.apply_delta(minus_half), Ok(0.5 - 0.5 / 16384.0));
  assert_eq!(
    F48Dot16::from_i64(65_536).and_then(|step| Fixed::ONE.apply_delta(step)),
    Ok(2.0)
  );
  assert_eq!(
    F48Dot16::from_i64(16_384).and_then(|step| F2Dot14::ONE.apply_delta(step)),
    Ok(2.0)
  );
}

#[test]
fn a_scaled_delta_is_summed_exactly_and_rounded_once() {
  let Ok(half) = Fixed::from_f64(0.5);
  let Ok(quarter) = Fixed::from_f64(0.25);
  let deltas = [(100, Fixed::ONE), (-250, half), (37, quarter)];
  let mut total = F48Dot16::ZERO;

  for (delta, scalar) in deltas {
    let Ok(scaled) = scalar.mul_i32(delta);

    total += scaled;
  }

  assert_eq!(Ok(total), F48Dot16::from_f64(-15.75));
  assert_eq!(total.to_i64(), Ok(-16));
  assert_eq!(total.round(), F48Dot16::from_f64(-16.0));
  assert_eq!(half.mul_i32(3), F48Dot16::from_f64(1.5));
  assert_eq!(Fixed::ONE.mul_i32(-7), F48Dot16::from_i64(-7));
  assert_eq!(Fixed::ZERO.mul_i32(i32::MAX), Ok(F48Dot16::ZERO));
  assert_eq!(
    Fixed::MIN.mul_i32(i32::MIN),
    F48Dot16::from_bits(0x4000_0000_0000_0000)
  );
  assert_eq!(
    Fixed::from_bits(3).and_then(|small| small.mul_i32(-5)),
    F48Dot16::from_bits(-15)
  );
}
