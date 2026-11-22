//! Bit-pattern conversions hold against Rust integer casts across every width.

use console_core_number_conversion::wrapped;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one oracle assertion for each primitive destination is shared by every tested source width without duplicating the twelve-cast matrix"
  )
)]
macro_rules! destinations {
  ($source:expr) => {
    assert_eq!(wrapped::<_, u8>($source), Ok(($source) as u8));
    assert_eq!(wrapped::<_, i8>($source), Ok(($source) as i8));
    assert_eq!(wrapped::<_, u16>($source), Ok(($source) as u16));
    assert_eq!(wrapped::<_, i16>($source), Ok(($source) as i16));
    assert_eq!(wrapped::<_, u32>($source), Ok(($source) as u32));
    assert_eq!(wrapped::<_, i32>($source), Ok(($source) as i32));
    assert_eq!(wrapped::<_, u64>($source), Ok(($source) as u64));
    assert_eq!(wrapped::<_, i64>($source), Ok(($source) as i64));
    assert_eq!(wrapped::<_, u128>($source), Ok(($source) as u128));
    assert_eq!(wrapped::<_, i128>($source), Ok(($source) as i128));
    assert_eq!(wrapped::<_, usize>($source), Ok(($source) as usize));
    assert_eq!(wrapped::<_, isize>($source), Ok(($source) as isize));
  };
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "Rust integer casts are the independent oracle for the bit-preserving conversion under test"
  )
)]
#[test]
fn every_sixteen_bit_pattern_keeps_its_low_bits_at_every_width() {
  for value in u16::MIN..=u16::MAX {
    destinations!(value);
    destinations!(value as i16);
  }
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "Rust integer casts are the independent oracle for the bit-preserving conversion under test"
  )
)]
#[test]
fn signed_extremes_extend_their_sign_before_narrowing() {
  for value in [
    i128::MIN,
    i128::MIN.wrapping_add(1),
    i64::MIN as i128,
    i32::MIN as i128,
    i16::MIN as i128,
    i8::MIN as i128,
    -1,
    0,
    1,
    i8::MAX as i128,
    i16::MAX as i128,
    i32::MAX as i128,
    i64::MAX as i128,
    i128::MAX,
  ] {
    destinations!(value);
  }
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "Rust integer casts are the independent oracle for the bit-preserving conversion under test"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the oracle deliberately compares usize and isize destinations against the pointer width on this machine"
  )
)]
#[test]
fn random_patterns_keep_the_same_bits_from_every_source_width() {
  let mut state = 0xd2a9_17b3_c654_e8f0_28e1_6a3b_f597_c40du128;

  for _sample in 0..4096 {
    state ^= state.wrapping_shl(23);
    state ^= state.wrapping_shr(17);
    state ^= state.wrapping_shl(37);
    destinations!(state);
    destinations!(state as i128);
    destinations!(state as u64);
    destinations!(state as i64);
    destinations!(state as u32);
    destinations!(state as i32);
    destinations!(state as u16);
    destinations!(state as i16);
    destinations!(state as u8);
    destinations!(state as i8);
    destinations!(state as usize);
    destinations!(state as isize);
  }
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "Rust's direct integer-to-f32 cast is the independent oracle; going through f64 would introduce double rounding"
  )
)]
#[test]
fn float_rounding_preserves_midpoints_and_their_neighbors() {
  use console_core_number_conversion::nearest_f32_from_u64;

  for shift in 1..=40 {
    for significand in [0x80_0000u64, 0x80_0001, 0xff_ffff] {
      let midpoint = significand
        .wrapping_shl(shift)
        .wrapping_add(1u64.wrapping_shl(shift.wrapping_sub(1)));

      for delta in [-1, 0, 1] {
        let value = midpoint.wrapping_add_signed(delta);
        let Ok(converted) = nearest_f32_from_u64(value);

        assert_eq!(converted.to_bits(), (value as f32).to_bits());
      }
    }
  }

  let mut state = 0x7bd4_18c9_59e2_a6f3u64;

  for _sample in 0..8192 {
    state ^= state.wrapping_shl(23);
    state ^= state.wrapping_shr(17);
    state ^= state.wrapping_shl(37);

    let Ok(converted) = nearest_f32_from_u64(state);

    assert_eq!(converted.to_bits(), (state as f32).to_bits());
  }

  for value in [0, 1, u64::MAX] {
    let Ok(converted) = nearest_f32_from_u64(value);

    assert_eq!(converted.to_bits(), (value as f32).to_bits());
  }
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "primitive casts are the independent oracle for signed and pointer-width rounding"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the pointer-width conversions are compared against casts at the machine's native width"
  )
)]
#[test]
fn signed_and_pointer_width_rounding_agree_with_primitive_casts() {
  use console_core_number_conversion::{
    nearest_f32_from_i64, nearest_f32_from_isize, nearest_f32_from_usize,
  };

  for value in [
    i64::MIN,
    i64::MIN.wrapping_add(1),
    -16_777_217,
    -1,
    0,
    1,
    16_777_217,
    i64::MAX,
  ] {
    assert_eq!(nearest_f32_from_i64(value), Ok(value as f32));
  }

  for value in [isize::MIN, -1, 0, 1, isize::MAX] {
    assert_eq!(nearest_f32_from_isize(value), Ok(value as f32));
  }

  for value in [0, 1, usize::MAX] {
    assert_eq!(nearest_f32_from_usize(value), Ok(value as f32));
  }
}

#[test]
fn checked_positions_return_the_conversion_error() {
  use console_core_number_conversion::{checked_index, checked_offset};

  assert_eq!(checked_index(0x1234_u16), Ok(0x1234));
  assert!(matches!(checked_index(-1_i8), Err(_outside_index_range)));
  assert_eq!(checked_offset(-23_i8), Ok(-23));
  assert!(matches!(
    checked_offset(u128::MAX),
    Err(_outside_offset_range)
  ));
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the test checks the unsigned and signed endpoints of the target's native integer range"
  )
)]
#[test]
fn fixed_positions_round_trip_across_the_native_range() {
  use console_core_number_conversion::{
    checked_index, checked_offset, fixed_index, fixed_offset,
  };

  for position in [0, 1, usize::MAX] {
    let Ok(stored) = fixed_index(position);
    assert_eq!(checked_index(stored), Ok(position));
  }

  for offset in [isize::MIN, -1, 0, 1, isize::MAX] {
    let Ok(stored) = fixed_offset(offset);
    assert_eq!(checked_offset(stored), Ok(offset));
  }
}
