use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::index;

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "Rust array lengths and slice chunk const parameters have native width"
  )
)]
pub(crate) const SIMD_WIDTH: usize = 8;

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "JPEG 2000 component and wavelet samples use the scale of their encoded component"
  )
)]
pub(crate) struct Sample(pub(crate) f32);

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "color and wavelet gains are dimensionless scale factors"
  )
)]
pub(crate) struct Gain(pub(crate) f32);

pub(crate) struct MultiplyAdd {
  pub(crate) value: Sample,
  pub(crate) factor: Gain,
  pub(crate) offset: Sample,
}

#[inline(always)]
pub(crate) fn mul_add(
  MultiplyAdd {
    value: Sample(value),
    factor: Gain(factor),
    offset: Sample(offset),
  }: MultiplyAdd,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(all(
      feature = "std",
      any(
        all(
          any(target_arch = "x86", target_arch = "x86_64"),
          target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
      )
    ))]
    {
      f32::mul_add(value, factor, offset)
    }

    #[cfg(not(all(
      feature = "std",
      any(
        all(
          any(target_arch = "x86", target_arch = "x86_64"),
          target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
      )
    )))]
    {
      value * factor + offset
    }
  })
}

#[inline(always)]
pub(crate) fn floor_f32(
  Sample(x): Sample,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.floor()
    }

    #[cfg(not(feature = "std"))]
    {
      let ::core::result::Result::Ok(xi) = ::console_core_number_conversion::toward_zero_i32(
        <::core::primitive::f64 as ::core::convert::From<::core::primitive::f32>>::from(x),
      );
      let ::core::result::Result::Ok(xf) = ::console_core_number_conversion::nearest_f32_from_i64(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::i32>>::from(xi),
      );

      match ::core::primitive::bool::then_some(x < xf, ()) {
        ::core::option::Option::Some(()) => xf - 1.0,
        ::core::option::Option::None => xf,
      }
    }
  })
}

#[inline(always)]
pub(crate) fn round_f32(
  Sample(x): Sample,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      x.round()
    }

    #[cfg(not(feature = "std"))]
    {
      match ::core::primitive::bool::then_some(x >= 0.0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(value) = floor_f32(Sample(x + 0.5));
          value
        }
        ::core::option::Option::None => {
          let Ok(value) = floor_f32(Sample(-x + 0.5));
          -value
        }
      }
    }
  })
}

#[inline(always)]
pub(crate) fn pow2i(exp: i32) -> core::result::Result<f32, console_core_never::Never> {
  Ok(match ::core::primitive::bool::then_some(exp >= 0, ()) {
    ::core::option::Option::Some(()) => {
      match console_core_number_conversion::narrow_f32(f64::from(u32::wrapping_shl(
        1_u32,
        match console_core_number_conversion::wrapped::<_, u32>(exp) {
          Ok(shift_bits) => shift_bits,
        },
      ))) {
        Ok(converted) => converted,
      }
    }
    ::core::option::Option::None => {
      1.0
        / (match console_core_number_conversion::narrow_f32(f64::from(u32::wrapping_shl(
          1_u32,
          match console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_neg(exp)) {
            Ok(shift_bits) => shift_bits,
          },
        ))) {
          Ok(converted) => converted,
        })
    }
  })
}

#[inline(always)]
pub(crate) fn min_f32([a, b]: [f32; 2]) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      a.min(b)
    }

    #[cfg(not(feature = "std"))]
    {
      match ::core::primitive::bool::then_some(a < b, ()) {
        ::core::option::Option::Some(()) => a,
        ::core::option::Option::None => b,
      }
    }
  })
}

#[inline(always)]
pub(crate) fn max_f32([a, b]: [f32; 2]) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    #[cfg(feature = "std")]
    {
      a.max(b)
    }

    #[cfg(not(feature = "std"))]
    {
      match ::core::primitive::bool::then_some(a > b, ()) {
        ::core::option::Option::Some(()) => a,
        ::core::option::Option::None => b,
      }
    }
  })
}

#[derive(Debug, Clone)]
pub(crate) struct SimdBuffer {
  data: Vec<f32>,
  original_len: u64,
}

impl SimdBuffer {
  pub(crate) fn new(mut data: Vec<f32>) -> crate::Result<Self> {
    let Ok(original_len) = console_core_number_conversion::fixed_index(data.len());
    let Ok(padded) = Self::padded_length(original_len);
    crate::resize_decoder_buffer(&mut data, padded, crate::DecoderBuffer::Component, 0.0)?;
    Ok(Self { data, original_len })
  }

  pub(crate) fn zeros(original_len: u64) -> crate::Result<Self> {
    let Ok(padded) = Self::padded_length(original_len);
    let mut data = Vec::new();
    crate::resize_decoder_buffer(&mut data, padded, crate::DecoderBuffer::Component, 0.0)?;
    Ok(Self { data, original_len })
  }

  fn padded_length(original_len: u64) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(lanes) = console_core_number_conversion::fixed_index(SIMD_WIDTH);
    let padding = lanes.wrapping_sub(original_len.wrapping_rem(lanes.max(1)));
    Ok(original_len.saturating_add(padding))
  }

  pub(crate) fn samples_mut(
    &mut self,
  ) -> core::result::Result<&mut [f32], console_core_never::Never> {
    Ok(&mut self.data)
  }

  pub(crate) fn truncated(&self) -> crate::Result<&[f32]> {
    let len = console_core_number_conversion::checked_index(self.original_len)
      .map_err(|_overflow| crate::ValidationError::InvalidDimensions)?;
    let data = self
      .data
      .get(..len)
      .ok_or(crate::ValidationError::InvalidDimensions)?;
    Ok(data)
  }
}

impl core::ops::Deref for SimdBuffer {
  type Target = [f32];

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.data
  }
}

#[inline]
pub(crate) fn simd() -> Result<Option<console_core_simd::FusedF32>, Never> {
  Ok({
    #[cfg(feature = "simd")]
    {
      let Ok(simd) = console_core_simd::Simd::detected();
      let Ok(capability) = simd.fused_f32();
      capability
    }

    #[cfg(not(feature = "simd"))]
    {
      None
    }
  })
}

#[doc = "Matches the separate multiply and add of the portable SIMD fallback."]
#[inline(always)]
pub(crate) fn vector_mul_add(calculation: MultiplyAdd) -> Result<f32, Never> {
  #[cfg(feature = "simd")]
  {
    let MultiplyAdd {
      value: Sample(value),
      factor: Gain(gain),
      offset: Sample(offset),
    } = calculation;
    Ok(value * gain + offset)
  }

  #[cfg(not(feature = "simd"))]
  {
    mul_add(calculation)
  }
}

pub(crate) fn affine(
  backend: Option<console_core_simd::FusedF32>,
  samples: &mut [f32],
  Gain(gain): Gain,
  Sample(offset): Sample,
) -> Result<(), Never> {
  let written = match backend {
    Some(cpu) => {
      let Ok(prefix) = cpu.affine(
        samples,
        console_core_simd::AffineGain::from(gain),
        console_core_simd::AffineOffset::from(offset),
      );
      prefix
    }
    None => 0,
  };
  let Ok(skip) = index(written);

  for sample in samples.iter_mut().skip(skip) {
    let Ok(value) = vector_mul_add(MultiplyAdd {
      value: Sample(*sample),
      factor: Gain(gain),
      offset: Sample(offset),
    });
    *sample = value;
  }

  Ok(())
}

pub(crate) fn color(
  transform: console_core_simd::ComponentTransform,
  components: [&mut [f32]; 3],
) -> Result<(), Never> {
  use console_core_simd::ComponentTransform;

  let [red, green, blue] = components.map(|channel| {
    let (prefix, _) = channel.as_chunks_mut::<SIMD_WIDTH>();
    prefix.as_flattened_mut()
  });
  let Ok(backend) = simd();
  let written = match backend {
    Some(cpu) => {
      let Ok(prefix) = cpu.color(transform, [red, green, blue]);
      prefix
    }
    None => 0,
  };
  let Ok(skip) = index(written);
  let pixels = red.iter_mut().zip(green).zip(blue).skip(skip);

  match transform {
    ComponentTransform::Reversible => {
      for ((r, g), b) in pixels {
        let Ok(correction) = floor_f32(Sample((*b + *g) * 0.25));
        let corrected = *r - correction;
        (*r, *g, *b) = (*b + corrected, corrected, *g + corrected);
      }
    }
    ComponentTransform::Ycbcr {
      offset,
      gains: [cr_r, cb_g, cr_g, cb_b],
      maximum,
    } => {
      for ((r, g), b) in pixels {
        let y = *r;
        let cb = *g - offset;
        let cr = *b - offset;

        let add_gain = |value, gain, base| {
          vector_mul_add(MultiplyAdd {
            value: Sample(value),
            factor: Gain(gain),
            offset: Sample(base),
          })
        };
        let Ok(r_value) = add_gain(cr, cr_r, y);
        let Ok(g_from_cb) = add_gain(cb, cb_g, y);
        let Ok(g_value) = add_gain(cr, cr_g, g_from_cb);
        let Ok(b_value) = add_gain(cb, cb_b, y);
        let Ok(values) = clip_color([r_value, g_value, b_value], maximum);
        [*r, *g, *b] = values;
      }
    }
  }

  Ok(())
}

fn clip_color(mut channels: [f32; 3], maximum: Option<f32>) -> Result<[f32; 3], Never> {
  match maximum {
    Some(max) => {
      for value in &mut channels {
        let Ok(upper) = min_f32([*value, max]);
        let Ok(clipped) = max_f32([upper, 0.0]);
        *value = clipped;
      }
    }
    None => {}
  }

  Ok(channels)
}
