//! The irreversible multi-component transformation, as specified in
//! Annex G.2 and G.3.

use super::ComponentData;
use super::codestream::{Header, WaveletTransform};
use crate::error::{ColorError, Result};
use crate::math::{Float32x8, Level, Simd, dispatch};

pub(crate) fn apply_inverse(
  components: &mut [ComponentData],
  component_infos: &[super::codestream::ComponentInfo],
  header: &Header<'_>,
) -> Result<()> {
  let (s0, s1, s2) = match (components, header.validation) {
    ([s0, s1, s2, ..], _) => (s0, s1, s2),
    (_, crate::ValidationMode::Lenient) => return Ok(()),
    (_, crate::ValidationMode::Strict) => return Err(crate::DecodeError::Color(ColorError::Mct)),
  };

  let (info0, info1, info2) = match component_infos {
    [i0, i1, i2, ..] => (i0, i1, i2),
    _ => return Err(crate::DecodeError::Color(ColorError::Mct)),
  };
  let Ok(transform) = info0.wavelet_transform();
  let Ok(transform1) = info1.wavelet_transform();
  let Ok(transform2) = info2.wavelet_transform();

  match (transform == transform1
    && transform1 == transform2
    && s0.container.len() == s1.container.len()
    && s1.container.len() == s2.container.len())
  .then_some(())
  {
    Some(()) => {}
    None => return Err(crate::DecodeError::Color(ColorError::Mct)),
  }

  let Ok(first) = s0.container.samples_mut();
  let Ok(second) = s1.container.samples_mut();
  let Ok(third) = s2.container.samples_mut();
  let Ok(()) = apply_inner(transform, first, second, third);

  Ok(())
}

fn apply_inner(
  transform: WaveletTransform,
  s0: &mut [f32],
  s1: &mut [f32],
  s2: &mut [f32],
) -> core::result::Result<(), console_core_never::Never> {
  {
    dispatch!(Level::new(), simd => {
      let Ok(()) = apply_inner_impl(simd, transform, s0, s1, s2);
    });
  };

  Ok(())
}

#[inline(always)]
fn apply_inner_impl<S: Simd>(
  simd: S,
  transform: WaveletTransform,
  s0: &mut [f32],
  s1: &mut [f32],
  s2: &mut [f32],
) -> core::result::Result<(), console_core_never::Never> {
  let _: () = {
    match transform {
      WaveletTransform::Irreversible97 => {
        for ((y0, y1), y2) in s0
          .as_chunks_mut::<8>()
          .0
          .iter_mut()
          .zip(s1.as_chunks_mut::<8>().0.iter_mut())
          .zip(s2.as_chunks_mut::<8>().0.iter_mut())
        {
          let Ok(y_0) = Float32x8::from_slice(simd, y0);
          let Ok(y_1) = Float32x8::from_slice(simd, y1);
          let Ok(y_2) = Float32x8::from_slice(simd, y2);

          let Ok(splat) = Float32x8::splat(simd, crate::math::Sample(1.402));
          let Ok(i0) = y_2.mul_add(splat, y_0);
          let Ok(float32x8_splat) = Float32x8::splat(simd, crate::math::Sample(-0.71414));
          let Ok(mul_add) = y_1.mul_add(
            match Float32x8::splat(simd, crate::math::Sample(-0.34413)) {
              Ok(value) => value,
            },
            y_0,
          );
          let Ok(i1) = y_2.mul_add(float32x8_splat, mul_add);
          let Ok(i2) = y_1.mul_add(
            match Float32x8::splat(simd, crate::math::Sample(1.772)) {
              Ok(value) => value,
            },
            y_0,
          );

          let Ok(()) = i0.store(y0);
          let Ok(()) = i1.store(y1);
          let Ok(()) = i2.store(y2);
        }
      }

      WaveletTransform::Reversible53 => {
        for ((y0, y1), y2) in s0
          .as_chunks_mut::<8>()
          .0
          .iter_mut()
          .zip(s1.as_chunks_mut::<8>().0.iter_mut())
          .zip(s2.as_chunks_mut::<8>().0.iter_mut())
        {
          let Ok(y_0) = Float32x8::from_slice(simd, y0);
          let Ok(y_1) = Float32x8::from_slice(simd, y1);
          let Ok(y_2) = Float32x8::from_slice(simd, y2);

          let Ok(floor) = ((y_2 + y_1) * 0.25).floor();
          let i1 = y_0 - floor;
          let i0 = y_2 + i1;
          let i2 = y_1 + i1;

          let Ok(()) = i0.store(y0);
          let Ok(()) = i1.store(y1);
          let Ok(()) = i2.store(y2);
        }
      }
    }
  };
  Ok(())
}
