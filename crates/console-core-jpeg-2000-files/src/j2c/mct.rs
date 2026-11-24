//! The irreversible multi-component transformation, as specified in
//! Annex G.2 and G.3.

use super::ComponentData;
use super::codestream::{Header, WaveletTransform};
use crate::error::{ColorError, Result};
use crate::math;
use console_core_simd::ComponentTransform;

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
  let Ok(wavelet) = info0.wavelet_transform();
  let Ok(transform1) = info1.wavelet_transform();
  let Ok(transform2) = info2.wavelet_transform();

  match (wavelet == transform1
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
  let transform = match wavelet {
    WaveletTransform::Reversible53 => ComponentTransform::Reversible,
    WaveletTransform::Irreversible97 => ComponentTransform::Ycbcr {
      offset: 0.0,
      gains: [1.402, -0.34413, -0.71414, 1.772],
      maximum: None,
    },
  };
  let Ok(()) = math::color(transform, [first, second, third]);

  Ok(())
}
