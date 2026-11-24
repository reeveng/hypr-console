use core::arch::x86_64::*;
use core::convert::Infallible;

#[derive(Debug, Clone, Copy)]
pub struct FusedF32(());

impl FusedF32 {
  pub fn detect() -> Result<Option<Self>, Infallible> {
    Ok(
      (std::is_x86_feature_detected!("avx2")
        && std::is_x86_feature_detected!("fma")
        && std::is_x86_feature_detected!("bmi1")
        && std::is_x86_feature_detected!("bmi2")
        && std::is_x86_feature_detected!("cmpxchg16b")
        && std::is_x86_feature_detected!("f16c")
        && std::is_x86_feature_detected!("lzcnt")
        && std::is_x86_feature_detected!("movbe")
        && std::is_x86_feature_detected!("popcnt")
        && std::is_x86_feature_detected!("xsave"))
      .then_some(Self(())),
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects AVX2 before the target-feature call"
    )
  )]
  pub fn interleave(
    self,
    first: &[f32],
    second: &[f32],
    output: &mut [[f32; 2]],
  ) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees AVX2.
    unsafe { interleave(first, second, output) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects AVX2 and FMA before the target-feature call"
    )
  )]
  pub fn lift<const MODE: u8>(
    self,
    current: &mut [f32],
    before: &[f32],
    after: &[f32],
    gain_bits: u32,
  ) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees AVX2 and FMA.
    unsafe { lift::<MODE>(current, before, after, gain_bits) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects AVX2 and FMA before the target-feature call"
    )
  )]
  pub fn affine(self, samples: &mut [f32], coefficients: [u32; 2]) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees AVX2 and FMA.
    unsafe { affine(samples, coefficients) }
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "The token constructor detects AVX2 and FMA before the target-feature call"
    )
  )]
  pub fn color<const REVERSIBLE: bool>(
    self,
    planes: [&mut [f32]; 3],
    offset_bits: u32,
    gains: [f32; 4],
    limit: Option<f32>,
  ) -> Result<u64, Infallible> {
    // SAFETY: The token guarantees AVX2 and FMA.
    unsafe { color::<REVERSIBLE>(planes, offset_bits, gains, limit) }
  }
}

#[target_feature(enable = "avx2")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The zipped chunks supply two full input vectors and both full output vectors for the unaligned accesses"
  )
)]
fn interleave(first: &[f32], second: &[f32], output: &mut [[f32; 2]]) -> Result<u64, Infallible> {
  let mut written = 0_u64;

  for ((a, b), out) in first
    .as_chunks::<8>()
    .0
    .iter()
    .zip(second.as_chunks::<8>().0)
    .zip(output.as_chunks_mut::<8>().0)
  {
    // SAFETY: Both inputs and both output halves cover a whole vector.
    unsafe {
      let a_2 = _mm256_loadu_ps(a.as_ptr());
      let b_2 = _mm256_loadu_ps(b.as_ptr());
      let lo = _mm256_unpacklo_ps(a_2, b_2);
      let hi = _mm256_unpackhi_ps(a_2, b_2);
      let out_2 = out.as_mut_ptr().cast::<f32>();
      _mm256_storeu_ps(out_2, _mm256_permute2f128_ps::<0x20>(lo, hi));
      _mm256_storeu_ps(out_2.add(8), _mm256_permute2f128_ps::<0x31>(lo, hi));
    }

    written = written.wrapping_add(8);
  }

  Ok(written)
}

#[target_feature(enable = "avx2,fma")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The zipped chunks supply a complete vector in each input and in the writable destination"
  )
)]
fn lift<const MODE: u8>(
  current: &mut [f32],
  before: &[f32],
  after: &[f32],
  gain_bits: u32,
) -> Result<u64, Infallible> {
  let gain_2 = _mm256_set1_ps(f32::from_bits(gain_bits));
  let mut written = 0_u64;

  for ((out, a), b) in current
    .as_chunks_mut::<8>()
    .0
    .iter_mut()
    .zip(before.as_chunks::<8>().0)
    .zip(after.as_chunks::<8>().0)
  {
    // SAFETY: All three slices are whole vector chunks.
    unsafe {
      let sum = _mm256_add_ps(_mm256_loadu_ps(a.as_ptr()), _mm256_loadu_ps(b.as_ptr()));
      let dst = _mm256_loadu_ps(out.as_ptr());
      let value = match MODE {
        0 => _mm256_sub_ps(
          dst,
          _mm256_floor_ps(_mm256_mul_ps(
            _mm256_add_ps(sum, _mm256_set1_ps(2.0)),
            _mm256_set1_ps(0.25),
          )),
        ),
        1 => _mm256_add_ps(
          dst,
          _mm256_floor_ps(_mm256_mul_ps(sum, _mm256_set1_ps(0.5))),
        ),
        _ => _mm256_fmadd_ps(sum, gain_2, dst),
      };
      _mm256_storeu_ps(out.as_mut_ptr(), value);
    }

    written = written.wrapping_add(8);
  }

  Ok(written)
}

#[target_feature(enable = "avx2,fma")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "The mutable chunk supplies eight writable f32 lanes for each unaligned load and store"
  )
)]
fn affine(samples: &mut [f32], [gain_bits, offset_bits]: [u32; 2]) -> Result<u64, Infallible> {
  let gain = f32::from_bits(gain_bits);
  let offset = f32::from_bits(offset_bits);
  let chunks = samples.as_chunks_mut::<8>().0;
  let gain_2 = _mm256_set1_ps(gain);
  let offset_2 = _mm256_set1_ps(offset);

  let mut written = 0_u64;

  for chunk in chunks.iter_mut() {
    // SAFETY: The slice is one whole vector.
    unsafe {
      let value = _mm256_loadu_ps(chunk.as_ptr());
      _mm256_storeu_ps(chunk.as_mut_ptr(), _mm256_fmadd_ps(value, gain_2, offset_2));
    }

    written = written.wrapping_add(8);
  }

  Ok(written)
}

#[target_feature(enable = "avx2,fma")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "Each of the three planes supplies a complete eight-lane chunk for all unaligned accesses"
  )
)]
fn color<const REVERSIBLE: bool>(
  [first, second, third]: [&mut [f32]; 3],
  offset_bits: u32,
  gains: [f32; 4],
  limit: Option<f32>,
) -> Result<u64, Infallible> {
  let offset_vector = _mm256_set1_ps(f32::from_bits(offset_bits));
  let [cr_r, cb_g, cr_g, cb_b] = gains.map(|gain| _mm256_set1_ps(gain));
  let limit_2 = limit.map(|max| _mm256_set1_ps(max));
  let mut written = 0_u64;

  for ((r, g), b) in first
    .as_chunks_mut::<8>()
    .0
    .iter_mut()
    .zip(second.as_chunks_mut::<8>().0)
    .zip(third.as_chunks_mut::<8>().0)
  {
    // SAFETY: All planes have a full vector at this position.
    unsafe {
      let y = _mm256_loadu_ps(r.as_ptr());
      let cb = _mm256_sub_ps(_mm256_loadu_ps(g.as_ptr()), offset_vector);
      let cr = _mm256_sub_ps(_mm256_loadu_ps(b.as_ptr()), offset_vector);
      let [red, green, blue] = match REVERSIBLE.then_some(()) {
        Some(()) => {
          let correction =
            _mm256_floor_ps(_mm256_mul_ps(_mm256_add_ps(cr, cb), _mm256_set1_ps(0.25)));
          let green = _mm256_sub_ps(y, correction);
          [_mm256_add_ps(cr, green), green, _mm256_add_ps(cb, green)]
        }
        None => [
          _mm256_fmadd_ps(cr, cr_r, y),
          _mm256_fmadd_ps(cr, cr_g, _mm256_fmadd_ps(cb, cb_g, y)),
          _mm256_fmadd_ps(cb, cb_b, y),
        ],
      };

      for (value, output) in [(red, r), (green, g), (blue, b)] {
        let value_2 = match limit_2 {
          Some(max) => _mm256_max_ps(_mm256_min_ps(value, max), _mm256_setzero_ps()),
          None => value,
        };
        _mm256_storeu_ps(output.as_mut_ptr(), value_2);
      }
    }

    written = written.wrapping_add(8);
  }

  Ok(written)
}
