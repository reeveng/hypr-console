//! The inverse transform of a granule: from five hundred and seventy-six
//! spectral lines, through the alias reduction and the windowed inverse MDCT
//! of each of the thirty-two subbands, to the samples the polyphase filterbank
//! is fed.
//!
//! A subband's eighteen lines become thirty-six samples, of which the first
//! eighteen are added to what the previous granule left over and the last
//! eighteen are kept for the next. A short block does that as three transforms
//! of twelve, one for each window, and a long one as one of thirty-six.

use console_core_never::Never;
use console_core_number_conversion::index;

const DCT_IV_SCALE: [f32; 18] = [
  1.998_096_5,
  1.982_889_8,
  1.952_592,
  1.907_433_9,
  1.847_759,
  1.774_021_6,
  1.686_782_8,
  1.586_706_6,
  1.474_554_7,
  1.351_180_4,
  1.217_522_9,
  1.074_599_3,
  0.923_497_2,
  0.765_366_85,
  0.601_411_6,
  0.432_879_24,
  0.261_052_37,
  0.087_238_77,
];
const DCT_II_18_SCALE: [f32; 9] = [
  1.992_389_4,
  1.931_851_6,
  1.812_615_6,
  1.638_304_1,
  std::f32::consts::SQRT_2,
  1.147_152_9,
  0.845_236_54,
  0.517_638_1,
  0.174_311_49,
];
const DCT_II_9: [f32; 7] = [
  -1.732_050_8,
  1.879_385_2,
  -0.347_296_36,
  -1.532_088_9,
  -0.684_040_3,
  -1.969_615_5,
  -1.285_575_3,
];

fn collect<const N: usize>(values: impl Iterator<Item = f32>) -> Result<[f32; N], Never> {
  let mut collected = [0f32; N];

  for (slot, value) in collected.iter_mut().zip(values) {
    *slot = value;
  }

  Ok(collected)
}

fn dct_ii_9(x: &[f32; 9]) -> Result<[f32; 9], Never> {
  let [x0, x1, x2, x3, x4, x5, x6, x7, x8] = *x;
  let [d0, d1, d2, d3, d4, d5, d6] = DCT_II_9;

  let a01 = x3 + x5;
  let a02 = x3 - x5;
  let a03 = x6 + x2;
  let a04 = x6 - x2;
  let a05 = x1 + x7;
  let a06 = x1 - x7;
  let a07 = x8 + x0;
  let a08 = x8 - x0;

  let a09 = x4 + a05;
  let a10 = a01 + a03;
  let a11 = a10 + a07;
  let a12 = a03 - a07;
  let a13 = a01 - a07;
  let a14 = a01 - a03;
  let a15 = a02 - a04;
  let a16 = a15 + a08;
  let a17 = a04 + a08;
  let a18 = a02 - a08;
  let a19 = a02 + a04;
  let a20 = 2.0 * x4 - a05;

  let m1 = d0 * a06;
  let m2 = d1 * a12;
  let m3 = d2 * a13;
  let m4 = d3 * a14;
  let m5 = d0 * a16;
  let m6 = d4 * a17;
  let m7 = d5 * a18;
  let m8 = d6 * a19;

  let a21 = a20 + m2;
  let a22 = a20 - m2;
  let a23 = a20 + m3;
  let a24 = m1 + m6;
  let a25 = m1 - m6;
  let a26 = m1 + m7;

  Ok([
    a09 + a11,
    m8 - a26,
    m4 - a21,
    m5,
    a22 - m3,
    a25 - m7,
    a11 - 2.0 * a09,
    a24 + m8,
    a23 + m4,
  ])
}

fn running_difference(values: [f32; 9]) -> Result<[f32; 9], Never> {
  collect(values.iter().scan(0.0f32, |carry, value| {
    let next = value - *carry;

    *carry = next;

    Some(next)
  }))
}

fn dct_ii_18(x: &[f32; 18]) -> Result<[f32; 18], Never> {
  let pairs = x.iter().zip(x.iter().rev()).take(9);

  let Ok(even_inputs) = collect::<9>(pairs.clone().map(|(front, back)| front + back));
  let Ok(odd_inputs) = collect::<9>(
    pairs
      .zip(DCT_II_18_SCALE.iter())
      .map(|((front, back), scale)| scale * (front - back)),
  );

  let Ok(evens) = dct_ii_9(&even_inputs);
  let Ok(odds) = dct_ii_9(&odd_inputs);
  let Ok(odds_2) = running_difference(odds);

  collect(
    evens
      .iter()
      .zip(odds_2.iter())
      .flat_map(|(even, odd)| [*even, *odd]),
  )
}

fn dct_iv(x: &[f32; 18]) -> Result<[f32; 18], Never> {
  let Ok(scaled) = collect::<18>(
    DCT_IV_SCALE
      .iter()
      .zip(x.iter())
      .map(|(scale, x_2)| scale * x_2),
  );
  let Ok(transformed) = dct_ii_18(&scaled);

  collect(transformed.iter().scan(0.0f32, |carry, value| {
    let next = (value / 2.0) - *carry;

    *carry = next;

    Some(next)
  }))
}

pub fn imdct36(
  x: &mut [f32; 18],
  window: &[f32; 36],
  overlap: &mut [f32; 18],
) -> Result<(), Never> {
  let Ok(dct) = dct_iv(x);

  let first = overlap
    .iter()
    .zip(dct.iter().skip(9))
    .zip(window.iter())
    .map(|((o, d), w)| o + d * w);

  let second = overlap
    .iter()
    .skip(9)
    .zip(dct.iter().skip(9).rev())
    .zip(window.iter().skip(9))
    .map(|((o, d), w)| o - d * w);

  let Ok(output) = collect::<18>(first.take(9).chain(second));

  let kept = dct
    .iter()
    .take(9)
    .rev()
    .zip(window.iter().skip(18))
    .map(|(d, w)| -d * w)
    .chain(
      dct
        .iter()
        .take(9)
        .zip(window.iter().skip(27))
        .map(|(d, w)| -d * w),
    );

  let Ok(kept_2) = collect::<18>(kept);

  *x = output;
  *overlap = kept_2;

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the windowed transform sample is a dimensionless amplitude added to the other overlapping samples"
  )
)]
fn accumulate(tmp: &mut [f32; 36], at: u32, value: f32) -> Result<(), Never> {
  let Ok(at_2) = index(at);

  for slot in tmp.get_mut(at_2).into_iter() {
    *slot += value;
  }

  Ok(())
}

fn dot(lane: [f32; 6], row: [f32; 6]) -> Result<f32, Never> {
  let [a0, a1, a2, a3, a4, a5] = lane;
  let [c0, c1, c2, c3, c4, c5] = row;

  Ok((a0 * c0) + (a1 * c1) + (a2 * c2) + (a3 * c3) + (a4 * c4) + (a5 * c5))
}

pub fn imdct12_win(
  x: &mut [f32; 18],
  window: &[f32; 36],
  overlap: &mut [f32; 18],
  cos12: &[[f32; 6]; 6],
) -> Result<(), Never> {
  let [
    x0,
    x1,
    x2,
    x3,
    x4,
    x5,
    x6,
    x7,
    x8,
    x9,
    x10,
    x11,
    x12,
    x13,
    x14,
    x15,
    x16,
    x17,
  ] = *x;
  let [w0, w1, w2, w3, w4, w5, w6, w7, w8, w9, w10, w11, ..] = window;
  let [r0, r1, r2, r3, r4, r5] = *cos12;

  let lanes = [
    [x0, x3, x6, x9, x12, x15],
    [x1, x4, x7, x10, x13, x16],
    [x2, x5, x8, x11, x14, x17],
  ];
  let rows = [(r0, r3), (r1, r4), (r2, r5)];
  let picks = [(w2, w3, w6, w11), (w1, w4, w7, w10), (w0, w5, w8, w9)];

  let mut tmp = [0f32; 36];

  for (w, lane) in (0u32..3).zip(lanes) {
    let base = 6u32.saturating_add(w.saturating_mul(6));

    for ((i, (left, right)), (a, b, c, d)) in (0u32..3).zip(rows).zip(picks) {
      let Ok(yl) = dot(lane, left);
      let Ok(yr) = dot(lane, right);

      accumulate(&mut tmp, base.saturating_add(2).saturating_sub(i), -yl * a)?;
      accumulate(&mut tmp, base.saturating_add(3).saturating_add(i), yl * b)?;
      accumulate(&mut tmp, base.saturating_add(6).saturating_add(i), yr * c)?;
      accumulate(&mut tmp, base.saturating_add(11).saturating_sub(i), yr * d)?;
    }
  }

  let halves = tmp.iter().take(18).zip(tmp.iter().skip(18));

  for ((slot, kept), (now, later)) in x.iter_mut().zip(overlap.iter_mut()).zip(halves) {
    *slot = now + *kept;
    *kept = *later;
  }

  Ok(())
}
