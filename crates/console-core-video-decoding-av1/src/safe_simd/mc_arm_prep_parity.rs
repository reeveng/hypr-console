//! Differential parity for the aarch64 16bpc `prep` (compound-prediction
//! motion compensation) kernels, against the scalar reference in `src/mc.rs`.
//!
//! ## Why 16bpc specifically
//!
//! `mct_prep_direct` already dual-computes NEON against
//! `prep_8tap_rust` / `prep_bilin_rust` under `__simd_test`, and a full-corpus
//! decode logs 132,144 `MC_PREP_MISMATCH` calls across 91 vectors, every one
//! of them 10- or 12-bit (record:
//! `benchmarks/aarch64_md5_attribution_2026-08-07.meta`). Zero at 8bpc. This
//! module reproduces that in a second, per (filter, mx, my, size, bitdepth)
//! cell, so a fix can be attributed rather than hoped at.
//!
//! ## The convention this module pins
//!
//! `prep` output is the compound-prediction intermediate that `avg` / `w_avg`
//! / `mask` / `w_mask` then consume. At 16bpc the reference subtracts
//! `BitDepth16::PREP_BIAS` (8192) from every value so it fits `i16`, and the
//! four consumers add `PREP_BIAS * k` back inside their rounding constant.
//!
//! The aarch64 side used to run an UNBIASED variant of that convention in
//! three of the five kernels — `prep_8tap_16bpc_inner` omitted the
//! subtraction, `avg`/`w_avg`/`mask` omitted the addition — while
//! `prep_bilin_16bpc_inner` and `w_mask_16bpc_inner` used the biased one. Two
//! conventions in one seam is not survivable in a decoder that falls back to
//! the scalar reference for any shape its SIMD does not cover: the `tmp`
//! buffer is shared, so a scalar-produced `tmp` read by a NEON `avg` (or the
//! reverse) is off by exactly one `PREP_BIAS`. This module asserts the SCALAR
//! convention, because that is the one the fallback path always speaks.

#![cfg(all(test, target_arch = "aarch64"))]

use crate::include::common::bitdepth::{BitDepth, BitDepth8, BitDepth16};
use crate::include::dav1d::picture::Rav1dPictureDataComponent;
use crate::src::enum_map::EnumKey;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::levels::Filter2d;
use crate::src::mc::{UnscaledSampling, prep_unscaled_reference};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use core::cmp::Ordering;
use core::num::NonZeroU64;

#[doc = "The xorshift64* seed reproduces every fixture plane."]
struct Rng(u64);

impl Rng {
  fn sample(&mut self, outcomes: NonZeroU64) -> Result<u64, Never> {
    let Ok(seeded) = super::next_fixture_sample(&mut self.0);
    Ok(seeded.wrapping_rem(outcomes.get()))
  }
}

enum KernelPath {
  Neon,
  ScalarFallback,
}

struct PredictorDifference {
  position: Point<u64>,
  neon: i16,
  scalar: i16,
}

struct CellResult {
  path: KernelPath,
  difference: Option<PredictorDifference>,
}

#[derive(Default)]
struct Report {
  cells: u64,
  live: u64,
  bad: Vec<String>,
  first: Option<String>,
}

impl Report {
  fn record(&mut self, cell: &FixtureCell, outcome: CellResult) -> Result<(), Never> {
    self.cells = self.cells.wrapping_add(1);

    match outcome.path {
      KernelPath::Neon => self.live = self.live.wrapping_add(1),
      KernelPath::ScalarFallback => {}
    }

    match outcome.difference {
      Some(mismatch) => {
        let FixtureCell {
          filter,
          bitdepth,
          sampling: UnscaledSampling { size, phase },
        } = cell;
        let code = filter.ordinal();
        let label = format!(
          "prep bd={bitdepth} filter={code} {}x{} mx={} my={}",
          size.width, size.height, phase.x, phase.y
        );

        match &self.first {
          Some(_) => {}
          None => {
            let PredictorDifference {
              position: Point { x, y },
              neon,
              scalar,
            } = mismatch;
            let delta = i32::from(neon).wrapping_sub(i32::from(scalar));
            self.first = Some(format!(
              "{label}: at ({x},{y}) neon={neon} scalar={scalar} (diff {delta})"
            ));
          }
        }

        self.bad.push(label);
      }
      None => {}
    }

    Ok(())
  }

  fn finish(self, expectation: SweepExpectation) -> Result<(), Never> {
    let SweepExpectation { name, minimum } = expectation;
    assert!(
      self.cells >= minimum,
      "{name}: only {} parameter cells ran (expected at least {minimum})",
      self.cells,
    );
    assert_eq!(
      self.live, self.cells,
      "{name}: every cell must exercise the NEON path"
    );
    assert!(
      self.bad.is_empty(),
      "{name}: {} of {} parameter cells diverged.\nFirst mismatch: {:?}\nCells: {:?}",
      self.bad.len(),
      self.cells,
      self.first,
      self.bad,
    );
    Ok(())
  }
}

struct SweepExpectation {
  name: &'static str,
  minimum: u64,
}

#[doc = "Eight pixels of padding cover the 8-tap reads of three samples before and four after each edge."]
const PAD: u64 = 8;

struct FixturePlane {
  stride: u64,
  origin: u64,
  count: u64,
  padded_samples: u64,
}

impl FixturePlane {
  fn new<Pixel>(size: Size<i32>) -> Rav1dResult<Self> {
    let width = u64::try_from(size.width).map_err(|_| Rav1dError::EINVAL)?;
    let height = u64::try_from(size.height).map_err(|_| Rav1dError::EINVAL)?;
    let padding = PAD.wrapping_mul(2);
    let stride = width.wrapping_add(padding);
    let rows = height.wrapping_add(padding);
    let origin = PAD.wrapping_mul(stride).wrapping_add(PAD);
    let count = width.checked_mul(height).ok_or(Rav1dError::EINVAL)?;
    let Ok(pixel_bytes) = fixed_index(core::mem::size_of::<Pixel>());
    let pixels_per_alignment = 64_u64.checked_div(pixel_bytes).ok_or(Rav1dError::EINVAL)?;
    let padded_samples = stride
      .wrapping_mul(rows)
      .checked_next_multiple_of(pixels_per_alignment)
      .ok_or(Rav1dError::EINVAL)?;
    Ok(Self {
      stride,
      origin,
      count,
      padded_samples,
    })
  }
}

struct FixtureCell {
  filter: Filter2d,
  bitdepth: u8,
  sampling: UnscaledSampling,
}

fn prep_cell<BD: BitDepth>(
  cell: &FixtureCell,
  plane: &FixturePlane,
  source: &[BD::Pixel],
  depth: BD,
) -> Rav1dResult<CellResult>
where
  BD::Pixel: Default,
{
  let Ok(stride) = index(plane.stride);
  let Ok(base) = index(plane.origin);
  let Ok(length) = index(plane.count);
  let mut native_predictors = [0_i16; 128 * 128];
  let mut reference_predictors = [0_i16; 128 * 128];
  let neon = native_predictors
    .get_mut(..length)
    .ok_or(Rav1dError::EINVAL)?;
  let scalar = reference_predictors
    .get_mut(..length)
    .ok_or(Rav1dError::EINVAL)?;
  let FixtureCell {
    filter, sampling, ..
  } = *cell;
  let UnscaledSampling { size, phase } = sampling;
  let native_outcome = {
    let Ok(mut pixels) = crate::src::safe_simd::aligned_plane(source);
    let component = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut pixels, stride);
    let at = component.with_offset::<BD>() + base;
    crate::src::safe_simd::mc_arm::mct_prep_dispatch::<BD>(
      filter,
      neon,
      at,
      size.width,
      size.height,
      phase.x,
      phase.y,
      depth,
    )
  };

  {
    let Ok(mut pixels) = crate::src::safe_simd::aligned_plane(source);
    let component = Rav1dPictureDataComponent::wrap_buf::<BD>(&mut pixels, stride);
    let at = component.with_offset::<BD>() + base;
    prep_unscaled_reference::<BD>(filter, scalar, at, sampling, depth)?;
  }

  let path = match native_outcome.then_some(()) {
    Some(()) => KernelPath::Neon,
    None => KernelPath::ScalarFallback,
  };
  let width = u64::try_from(size.width).map_err(|_| Rav1dError::EINVAL)?;
  let row_width = NonZeroU64::new(width).ok_or(Rav1dError::EINVAL)?;
  let difference = (0..plane.count)
    .zip(neon.iter().copied().zip(scalar.iter().copied()))
    .find_map(
      |(ordinal, (actual, expected))| match actual.cmp(&expected) {
        Ordering::Equal => None,
        Ordering::Less | Ordering::Greater => Some(PredictorDifference {
          position: Point {
            x: ordinal.wrapping_rem(row_width.get()),
            y: ordinal.wrapping_div(row_width.get()),
          },
          neon: actual,
          scalar: expected,
        }),
      },
    );

  Ok(CellResult { path, difference })
}

#[doc = "Shapes used by the original NEON differential, including rectangular and maximum-size blocks."]
const SIZES: [[u8; 2]; 14] = [
  [4, 4],
  [4, 8],
  [8, 4],
  [8, 8],
  [8, 16],
  [16, 8],
  [16, 16],
  [16, 32],
  [32, 16],
  [32, 32],
  [32, 64],
  [64, 32],
  [64, 64],
  [128, 128],
];

#[doc = "These phases cover identity, horizontal, vertical and combined kernels."]
const SUBPEL: [[i32; 2]; 6] = [[0, 0], [5, 0], [0, 11], [5, 11], [1, 1], [15, 15]];

struct FixtureSweep<BD> {
  depth: BD,
  bitdepth: u8,
  seed: u64,
  filters: &'static [Filter2d],
}

fn sweep<BD: BitDepth>(report: &mut Report, cases: FixtureSweep<BD>) -> Rav1dResult
where
  BD::Pixel: Default,
{
  let FixtureSweep {
    depth,
    bitdepth,
    seed,
    filters,
  } = cases;
  let maximum = u64::try_from(depth.into_c()).map_err(|_| Rav1dError::EINVAL)?;
  let outcomes = NonZeroU64::new(maximum.wrapping_add(1)).ok_or(Rav1dError::EINVAL)?;

  for &filter in filters {
    let code = u64::from(filter.ordinal()).wrapping_shl(40);

    for [width, height] in SIZES {
      let size = Size {
        width: i32::from(width),
        height: i32::from(height),
      };
      let plane = FixturePlane::new::<BD::Pixel>(size)?;
      let mut rng = Rng(seed ^ code ^ plane.count);
      let mut samples = Vec::new();

      for _ in 0..plane.padded_samples {
        let Ok(value) = rng.sample(outcomes);
        let Ok(pixel) = console_core_number_conversion::wrapped::<_, i32>(value);
        let converted = BD::Pixel::try_from(pixel).map_err(|_| Rav1dError::EINVAL)?;
        samples.push(converted);
      }

      for [x, y] in SUBPEL {
        let sampling = UnscaledSampling {
          size,
          phase: Point { x, y },
        };
        let cell = FixtureCell {
          filter,
          bitdepth,
          sampling,
        };
        let result = prep_cell(&cell, &plane, &samples, depth)?;
        let Ok(()) = report.record(&cell, result);
      }
    }
  }

  Ok(())
}

#[test]
fn prep_16bpc_matches_scalar() -> Rav1dResult {
  let Ok(_lock) = crate::src::safe_simd::token_test_lock();
  let mut report = Report::default();
  const FILTERS: &[Filter2d] = &[
    Filter2d::Regular8Tap,
    Filter2d::Smooth8Tap,
    Filter2d::Sharp8Tap,
    Filter2d::RegularSmooth8Tap,
    Filter2d::Bilinear,
  ];

  for (bitdepth, maximum) in [(10, 1023), (12, 4095)] {
    let seed = 0x5EED_0000_0000_0001 ^ u64::from(bitdepth).wrapping_shl(48);
    let depth = BitDepth16::new(maximum);
    sweep(
      &mut report,
      FixtureSweep {
        depth,
        bitdepth,
        seed,
        filters: FILTERS,
      },
    )?;
  }

  let Ok(()) = report.finish(SweepExpectation {
    name: "aarch64 16bpc prep",
    minimum: 600,
  });
  Ok(())
}

#[doc = "The 8bpc control exercises the same differential with previously verified byte kernels."]
#[test]
fn prep_8bpc_matches_scalar_control() -> Rav1dResult {
  let Ok(_lock) = crate::src::safe_simd::token_test_lock();
  let mut report = Report::default();
  let depth = BitDepth8::new(());
  let bitdepth = 8;
  let seed = 0xC0FF_EE00_0000_0002;
  const FILTERS: &[Filter2d] = &[
    Filter2d::Regular8Tap,
    Filter2d::Sharp8Tap,
    Filter2d::Bilinear,
  ];
  sweep(
    &mut report,
    FixtureSweep {
      depth,
      bitdepth,
      seed,
      filters: FILTERS,
    },
  )?;
  let Ok(()) = report.finish(SweepExpectation {
    name: "aarch64 8bpc prep (control)",
    minimum: 200,
  });
  Ok(())
}
