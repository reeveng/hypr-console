//! The tables a granule's decoding reads that are worked out rather than
//! typed: the three-halves power the spectral integers are raised by, the
//! windows of the inverse transform, its cosines, the butterflies of the
//! alias reduction and the intensity stereo ratios.
//!
//! Upstream worked each out the first time it was asked and kept it for the
//! life of the process. They are worked out once here too, when a decoder is
//! made, and held by it, so the process shares nothing it was not handed.

use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use console_core_never::Never;
use console_core_number_conversion::{Float, narrow_f32};

const ANTIALIAS_COEFFICIENTS: [f64; 8] = [
  -0.6, -0.535, -0.33, -0.185, -0.095, -0.041, -0.0142, -0.0037,
];

pub(super) struct Computed {
  pow43: Vec<f32>,
  windows: [[f32; 36]; 4],
  cos12: [[f32; 6]; 6],
  antialias: [(f32, f32); 8],
  intensity_mpeg1: [(f32, f32); 7],
  intensity_mpeg2: [[(f32, f32); 32]; 2],
}

fn windows() -> Result<[[f32; 36]; 4], Never> {
  let pi_36 = PI / 36.0;
  let pi_12 = PI / 12.0;

  let normal = std::array::from_fn(|i| {
    let Ok(at) = i.float();
    let Ok(value) = narrow_f32((pi_36 * (at + 0.5)).sin());

    value
  });

  let start = std::array::from_fn(|i| {
    let Ok(at) = i.float();
    let Ok(long) = narrow_f32((pi_36 * (at + 0.5)).sin());
    let Ok(short) = narrow_f32((pi_12 * ((at - 18.0) + 0.5)).sin());

    match i {
      0..18 => long,
      18..24 => 1.0,
      24..30 => short,
      _ => 0.0,
    }
  });

  let short = std::array::from_fn(|i| {
    let Ok(at) = i.float();
    let Ok(value) = narrow_f32((pi_12 * (at + 0.5)).sin());

    match i {
      0..12 => value,
      _ => 0.0,
    }
  });

  let end = std::array::from_fn(|i| {
    let Ok(at) = i.float();
    let Ok(long) = narrow_f32((pi_36 * (at + 0.5)).sin());
    let Ok(short_2) = narrow_f32((pi_12 * ((at - 6.0) + 0.5)).sin());

    match i {
      6..12 => short_2,
      12..18 => 1.0,
      18..36 => long,
      _ => 0.0,
    }
  });

  Ok([normal, start, short, end])
}

fn cos12() -> Result<[[f32; 6]; 6], Never> {
  let pi_24 = PI / 24.0;

  Ok(std::array::from_fn(|i| {
    std::array::from_fn(|k| {
      let Ok(i_2) = i.float();
      let Ok(k_2) = k.float();
      let n = (2.0 * (i_2 + 3.0) + (12.0 / 2.0) + 1.0) * (2.0 * k_2 + 1.0);
      let Ok(value) = narrow_f32((pi_24 * n).cos());

      value
    })
  }))
}

fn antialias() -> Result<[(f32, f32); 8], Never> {
  Ok(ANTIALIAS_COEFFICIENTS.map(|c| {
    let sqrt = f64::sqrt(1.0 + (c * c));
    let Ok(cs) = narrow_f32(1.0 / sqrt);
    let Ok(ca) = narrow_f32(c / sqrt);

    (cs, ca)
  }))
}

fn intensity_mpeg1() -> Result<[(f32, f32); 7], Never> {
  let pi_12 = PI / 12.0;

  let [a, b, c, d, e, f, _invalid]: [(f32, f32); 7] = std::array::from_fn(|position| {
    let Ok(position_2) = position.float();
    let ratio = (pi_12 * position_2).tan();
    let Ok(left) = narrow_f32(ratio / (1.0 + ratio));
    let Ok(right) = narrow_f32(1.0 / (1.0 + ratio));

    (left, right)
  });

  Ok([a, b, c, d, e, f, (1.0, 0.0)])
}

fn intensity_mpeg2() -> Result<[[(f32, f32); 32]; 2], Never> {
  let scales: [f64; 2] = [1.0 / f64::sqrt(SQRT_2), FRAC_1_SQRT_2];

  Ok(scales.map(|scale| {
    std::array::from_fn(|position| {
      let Ok(float) = position.float();

      match position & 1 {
        0 => {
          let Ok(right) = narrow_f32(scale.powf(float / 2.0));

          (1.0, right)
        }
        _ => {
          let Ok(left) = narrow_f32(scale.powf((float + 1.0) / 2.0));

          (left, 1.0)
        }
      }
    })
  }))
}

impl Computed {
  pub(super) fn new() -> Result<Self, Never> {
    let pow43 = (0..8207u16)
      .map(|i| f32::powf(f32::from(i), 4.0 / 3.0))
      .collect();

    let Ok(windows) = windows();
    let Ok(cos12) = cos12();
    let Ok(antialias) = antialias();
    let Ok(intensity_mpeg1) = intensity_mpeg1();
    let Ok(intensity_mpeg2) = intensity_mpeg2();

    Ok(Computed {
      pow43,
      windows,
      cos12,
      antialias,
      intensity_mpeg1,
      intensity_mpeg2,
    })
  }

  pub(super) fn pow43(&self) -> Result<&[f32], Never> {
    Ok(&self.pow43)
  }

  pub(super) fn windows(&self) -> Result<&[[f32; 36]; 4], Never> {
    Ok(&self.windows)
  }

  pub(super) fn cos12(&self) -> Result<&[[f32; 6]; 6], Never> {
    Ok(&self.cos12)
  }

  pub(super) fn antialias(&self) -> Result<&[(f32, f32); 8], Never> {
    Ok(&self.antialias)
  }

  pub(super) fn intensity_mpeg1(&self) -> Result<&[(f32, f32); 7], Never> {
    Ok(&self.intensity_mpeg1)
  }

  pub(super) fn intensity_mpeg2(&self) -> Result<&[[(f32, f32); 32]; 2], Never> {
    Ok(&self.intensity_mpeg2)
  }
}
