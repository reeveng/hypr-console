use super::Neon;
use core::arch::aarch64::*;
use core::convert::Infallible;

impl Neon {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn interleave(
    self,
    first: &[f32],
    second: &[f32],
    output: &mut [[f32; 2]],
  ) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees NEON.
    unsafe { interleave(first, second, output) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn lift<const MODE: u8>(
    self,
    current: &mut [f32],
    before: &[f32],
    after: &[f32],
    gain_bits: u32,
  ) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees NEON.
    unsafe { lift::<MODE>(current, before, after, gain_bits) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn affine(self, samples: &mut [f32], coefficients: [u32; 2]) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees NEON.
    unsafe { affine(samples, coefficients) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "Neon::detect is the only token constructor and checks the NEON target feature before this call"
    )
  )]
  pub fn color<const REVERSIBLE: bool>(
    self,
    planes: [&mut [f32]; 3],
    offset_bits: u32,
    gains: [f32; 4],
    limit: Option<f32>,
  ) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees NEON.
    unsafe { color::<REVERSIBLE>(planes, offset_bits, gains, limit) }
  }
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Each zipped input chunk has four f32 lanes and each output chunk has four interleaved pairs for the eight-lane store"
  )
)]
#[target_feature(enable = "neon")]
fn interleave(first: &[f32], second: &[f32], output: &mut [[f32; 2]]) -> Result<u64, Infallible> {
  let mut written = 0_u64;

  for ((a, b), out) in first
    .as_chunks::<4>()
    .0
    .iter()
    .zip(second.as_chunks::<4>().0)
    .zip(output.as_chunks_mut::<4>().0)
  {
    // SAFETY: Both inputs are full vectors and the output holds four pairs.
    unsafe {
      vst2q_f32(
        out.as_mut_ptr().cast(),
        float32x4x2_t(vld1q_f32(a.as_ptr()), vld1q_f32(b.as_ptr())),
      )
    };

    written = written.wrapping_add(4);
  }

  Ok(written)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Every input and destination is a complete four-lane f32 chunk before a NEON load or store"
  )
)]
#[target_feature(enable = "neon")]
fn lift<const MODE: u8>(
  current: &mut [f32],
  before: &[f32],
  after: &[f32],
  gain_bits: u32,
) -> Result<u64, Infallible> {
  let gain = f32::from_bits(gain_bits);
  let mut written = 0_u64;

  for ((out, a), b) in current
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .zip(before.as_chunks::<4>().0)
    .zip(after.as_chunks::<4>().0)
  {
    // SAFETY: All three slices are whole vector chunks.
    unsafe {
      let sum = vaddq_f32(vld1q_f32(a.as_ptr()), vld1q_f32(b.as_ptr()));
      let dst = vld1q_f32(out.as_ptr());
      let value = match MODE {
        0 => vsubq_f32(
          dst,
          vrndmq_f32(vmulq_n_f32(vaddq_f32(sum, vdupq_n_f32(2.0)), 0.25)),
        ),
        1 => vaddq_f32(dst, vrndmq_f32(vmulq_n_f32(sum, 0.5))),
        _ => vfmaq_n_f32(dst, sum, gain),
      };
      vst1q_f32(out.as_mut_ptr(), value);
    }

    written = written.wrapping_add(4);
  }

  Ok(written)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Each mutable input chunk contains four f32 lanes covering both the NEON load and store"
  )
)]
#[target_feature(enable = "neon")]
fn affine(samples: &mut [f32], [gain_bits, offset_bits]: [u32; 2]) -> Result<u64, Infallible> {
  let gain = f32::from_bits(gain_bits);
  let chunks = samples.as_chunks_mut::<4>().0;
  let offset = vdupq_n_f32(f32::from_bits(offset_bits));
  let mut written = 0_u64;

  for chunk in chunks.iter_mut() {
    // SAFETY: The slice is one whole vector.
    unsafe {
      let value = vld1q_f32(chunk.as_ptr());
      vst1q_f32(chunk.as_mut_ptr(), vfmaq_n_f32(offset, value, gain));
    }

    written = written.wrapping_add(4);
  }

  Ok(written)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Each of the three zipped component planes has a complete four-lane chunk at every NEON load and store"
  )
)]
#[target_feature(enable = "neon")]
fn color<const REVERSIBLE: bool>(
  [first, second, third]: [&mut [f32]; 3],
  offset_bits: u32,
  [cr_r, cb_g, cr_g, cb_b]: [f32; 4],
  limit: Option<f32>,
) -> Result<u64, Infallible> {
  let offset_vector = vdupq_n_f32(f32::from_bits(offset_bits));
  let limit_vector = limit.map(|max| vdupq_n_f32(max));
  let mut written = 0_u64;

  for ((r, g), b) in first
    .as_chunks_mut::<4>()
    .0
    .iter_mut()
    .zip(second.as_chunks_mut::<4>().0)
    .zip(third.as_chunks_mut::<4>().0)
  {
    // SAFETY: All planes have a full vector at this position.
    unsafe {
      let y = vld1q_f32(r.as_ptr());
      let cb = vsubq_f32(vld1q_f32(g.as_ptr()), offset_vector);
      let cr = vsubq_f32(vld1q_f32(b.as_ptr()), offset_vector);
      let [red, green, blue] = match REVERSIBLE.then_some(()) {
        Some(()) => {
          let correction = vrndmq_f32(vmulq_n_f32(vaddq_f32(cr, cb), 0.25));
          let green = vsubq_f32(y, correction);
          [vaddq_f32(cr, green), green, vaddq_f32(cb, green)]
        }
        None => [
          vfmaq_n_f32(y, cr, cr_r),
          vfmaq_n_f32(vfmaq_n_f32(y, cb, cb_g), cr, cr_g),
          vfmaq_n_f32(y, cb, cb_b),
        ],
      };

      for (value, output) in [(red, r), (green, g), (blue, b)] {
        let clipped = match limit_vector {
          Some(max) => vmaxq_f32(vminq_f32(value, max), vdupq_n_f32(0.0)),
          None => value,
        };
        vst1q_f32(output.as_mut_ptr(), clipped);
      }
    }

    written = written.wrapping_add(4);
  }

  Ok(written)
}
