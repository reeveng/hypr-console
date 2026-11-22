//! Full-raster Catmull–Rom interpolation of grey, RGB and straight-alpha RGBA.
//! Each axis widens the kernel when shrinking and normalizes the surviving edge
//! taps. Both passes round to byte samples. Signed Q15 coefficients preserve the
//! clipped edge behavior of the PDF renderer. A single row holds the vertical pass.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, toward_zero_u8, toward_zero_u32};
use core::{cmp::Ordering, fmt};
use core::ops::Range;

const MAX_PIXELS: u64 = 268_435_456;
const MAX_WORK_BYTES: u64 = 1 << 30;
const Q15_UNIT: f64 = 32768.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channels {
  Grey,
  Rgb,
  Rgba,
}

impl Channels {
  fn count(self) -> Result<u32, Never> {
    Ok(match self {
      Self::Grey => 1,
      Self::Rgb => 3,
      Self::Rgba => 4,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeError {
  Dimensions,
  Buffer,
  Allocation,
}

impl fmt::Display for ResizeError {
  fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
    out.write_str(match self {
      Self::Dimensions => "invalid image dimensions for resampling",
      Self::Buffer => "sample buffer length does not match the dimensions and channels",
      Self::Allocation => "image resampling would exceed available memory",
    })
  }
}

impl core::error::Error for ResizeError {}

struct Span {
  first: u32,
  coefficients: Range<u64>,
}

struct Axis {
  source: u32,
  target: u32,
}

fn reserve<T>(buffer: &mut Vec<T>, additional: u64) -> Result<(), ResizeError> {
  let Ok(used) = fixed_index(buffer.len());
  let Ok(element_bytes) = fixed_index(size_of::<T>());
  let bytes = used
    .saturating_add(additional)
    .saturating_mul(element_bytes);

  match bytes {
    0..=MAX_WORK_BYTES => {}
    _over_budget => return Err(ResizeError::Allocation),
  }

  let Ok(count) = index(additional);
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "the resulting element count in bytes was checked against MAX_WORK_BYTES above"
    )
  )]
  buffer
    .try_reserve(count)
    .map_err(|_| ResizeError::Allocation)
}

impl Axis {
  fn append(&self, pixel: u32, weights: &mut Vec<f64>) -> Result<Span, ResizeError> {
    let ratio = f64::from(self.source) / f64::from(self.target);
    let spread = ratio.max(1.0);
    let width = (4.0 * spread).round();
    let center = (f64::from(pixel) + 0.5) * ratio;
    let radius = width * 0.5;
    let Ok(first) = toward_zero_u32((center - radius).floor().max(0.0));
    let Ok(end) = toward_zero_u32(
      (center + radius)
        .ceil()
        .min(f64::from(first) + width)
        .min(f64::from(self.source)),
    );

    let Ok(offset) = fixed_index(weights.len());
    let length = end.saturating_sub(first);
    let after = offset.saturating_add(u64::from(length));

    reserve(weights, u64::from(length))?;

    for tap in first..end {
      let x = ((f64::from(tap) + 0.5 - center) / spread).abs();
      let weight = match x {
        ..1.0 => (1.5 * x - 2.5) * x * x + 1.0,
        1.0..2.0 => ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0,
        _outside_kernel => 0.0,
      };
      weights.push(weight);
    }

    let Ok(start) = index(offset);
    let taps = weights.get_mut(start..).ok_or(ResizeError::Buffer)?;
    let Ok(()) = normalize(taps);

    Ok(Span {
      first,
      coefficients: offset..after,
    })
  }
}

enum CoefficientChange {
  Raise,
  Lower,
}

impl From<bool> for CoefficientChange {
  fn from(increase: bool) -> Self {
    match bool::then_some(increase, ()) {
      Some(()) => Self::Raise,
      None => Self::Lower,
    }
  }
}

fn normalize(taps: &mut [f64]) -> Result<(), Never> {
  let total: f64 = taps.iter().sum();
  let mut residual = Q15_UNIT;

  for tap in &mut *taps {
    *tap = (*tap / total * Q15_UNIT)
      .round()
      .clamp(-Q15_UNIT, Q15_UNIT - 1.0);
    residual -= *tap;
  }

  let Ok(tap_count) = fixed_index(taps.len());

  for _correction in 0..tap_count {
    match residual.total_cmp(&0.0) {
      Ordering::Equal => break,
      Ordering::Less | Ordering::Greater => {}
    }

    let direction = CoefficientChange::from(residual.is_sign_positive());
    let eligible = taps
      .iter_mut()
      .filter(|weight| match direction {
        CoefficientChange::Raise => **weight < Q15_UNIT - 1.0,
        CoefficientChange::Lower => **weight > -Q15_UNIT,
      });
    let strongest = eligible.max_by(|a, b| a.abs().total_cmp(&b.abs()));
    let tap = match strongest {
      Some(tap) => tap,
      None => break,
    };

    let change = residual.clamp(-Q15_UNIT - *tap, Q15_UNIT - 1.0 - *tap);
    *tap += change;
    residual -= change;
  }

  for tap in taps {
    *tap /= Q15_UNIT;
  }

  Ok(())
}

fn sample_length(size: Size<u32>, channels: Channels) -> Result<u64, ResizeError> {
  let Ok(area) = size.area();
  let Ok(count) = channels.count();

  match area {
    1..=MAX_PIXELS => Ok(area.saturating_mul(u64::from(count))),
    _invalid => Err(ResizeError::Dimensions),
  }
}

#[derive(Clone, Copy)]
enum Alpha {
  Straight,
  Associated,
}

impl From<bool> for Alpha {
  fn from(varying: bool) -> Self {
    match bool::then_some(varying, ()) {
      Some(()) => Self::Associated,
      None => Self::Straight,
    }
  }
}

fn associate(bytes: &mut [u8], channels: Channels) -> Result<Alpha, Never> {
  let alpha_mode = match (channels, bytes.first_chunk::<4>()) {
    (Channels::Rgba, Some([_, _, _, first])) => {
      Alpha::from(bytes.as_chunks::<4>().0.iter().any(|[_, _, _, alpha]| alpha != first))
    }
    (Channels::Grey | Channels::Rgb, _) | (Channels::Rgba, None) => Alpha::Straight,
  };

  match alpha_mode {
    Alpha::Associated => {
      for [red, green, blue, alpha] in bytes.as_chunks_mut::<4>().0 {
        let factor = f64::from(*alpha) / 255.0;

        for channel in [red, green, blue] {
          let Ok(sample) = toward_zero_u8((f64::from(*channel) * factor).round());
          *channel = sample;
        }
      }
    }
    Alpha::Straight => {}
  }

  Ok(alpha_mode)
}

enum ResizePlan {
  Unchanged,
  Resample,
}

impl From<bool> for ResizePlan {
  fn from(unchanged: bool) -> Self {
    match bool::then_some(unchanged, ()) {
      Some(()) => Self::Unchanged,
      None => Self::Resample,
    }
  }
}

fn unassociate(pixel: &mut [u8; 4]) -> Result<(), Never> {
  let [red, green, blue, alpha] = pixel;
  let factor = match *alpha {
    0 => 0.0,
    coverage => 255.0 / f64::from(coverage),
  };

  for sample in [red, green, blue] {
    let Ok(value) = toward_zero_u8((f64::from(*sample) * factor).round());
    *sample = value;
  }

  Ok(())
}

pub fn catmull_rom(
  mut bytes: Vec<u8>,
  source: Size<u32>,
  target: Size<u32>,
  channels: Channels,
) -> Result<Vec<u8>, ResizeError> {
  let length = sample_length(target, channels)?;
  let expected = sample_length(source, channels)?;
  let Ok(actual) = fixed_index(bytes.len());

  match (actual.cmp(&expected), ResizePlan::from(source == target)) {
    (Ordering::Less | Ordering::Greater, ResizePlan::Unchanged | ResizePlan::Resample) => {
      return Err(ResizeError::Buffer);
    }
    (Ordering::Equal, ResizePlan::Unchanged) => return Ok(bytes),
    (Ordering::Equal, ResizePlan::Resample) => {}
  }

  let across = Axis {
    source: source.width,
    target: target.width,
  };
  let down = Axis {
    source: source.height,
    target: target.height,
  };
  let Ok(count) = channels.count();
  let stride = u64::from(source.width).saturating_mul(u64::from(count));
  let mut horizontal = Vec::new();
  reserve(&mut horizontal, u64::from(target.width))?;
  let mut weights = Vec::new();

  for column in 0..target.width {
    let span = across.append(column, &mut weights)?;
    horizontal.push(span);
  }

  let Ok(samples_per_row) = index(stride);
  let mut row = Vec::new();
  reserve(&mut row, stride)?;
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "reserve checked the row in bytes against MAX_WORK_BYTES and obtained its capacity"
    )
  )]
  row.resize(samples_per_row, 0.0);

  let mut output = Vec::new();
  reserve(&mut output, length)?;
  let mut vertical = Vec::new();
  let Ok(alpha) = associate(&mut bytes, channels);

  for y in 0..target.height {
    vertical.clear();
    let lines = down.append(y, &mut vertical)?;
    row.fill(0.0);

    let Ok(start) = index(u64::from(lines.first).saturating_mul(stride));
    let input = bytes.get(start..).ok_or(ResizeError::Buffer)?;

    for (line, weight) in input.chunks_exact(samples_per_row.max(1)).zip(&vertical) {
      for (sum, sample) in row.iter_mut().zip(line) {
        *sum += f64::from(*sample) * weight;
      }
    }

    for value in &mut row {
      let Ok(rounded) = toward_zero_u8(value.round());
      *value = f64::from(rounded);
    }

    write_row(&row, &horizontal, &weights, count, alpha, &mut output)?;
  }

  Ok(output)
}

fn write_row(
  row: &[f64],
  columns: &[Span],
  weights: &[f64],
  count: u32,
  alpha: Alpha,
  output: &mut Vec<u8>,
) -> Result<(), ResizeError> {
  let Ok(samples_per_pixel) = index(count);

  for column in columns {
    let Ok(first) = index(u64::from(column.first).saturating_mul(u64::from(count)));
    let samples = row.get(first..).ok_or(ResizeError::Buffer)?;
    let Ok(begin) = index(column.coefficients.start);
    let Ok(end) = index(column.coefficients.end);
    let coefficients = weights.get(begin..end).ok_or(ResizeError::Buffer)?;
    let mut sums = [0.0; 4];

    for (tap, weight) in samples
      .chunks_exact(samples_per_pixel.max(1))
      .zip(coefficients)
    {
      for (sum, sample) in sums.iter_mut().zip(tap) {
        *sum += sample * weight;
      }
    }

    let mut pixel = sums.map(|sum| {
      let Ok(rounded) = toward_zero_u8(sum.round());
      rounded
    });

    match alpha {
      Alpha::Associated => {
        let Ok(()) = unassociate(&mut pixel);
      }
      Alpha::Straight => {}
    }

    let included = pixel.get(..samples_per_pixel).ok_or(ResizeError::Buffer)?;
    output.extend_from_slice(included);
  }

  Ok(())
}
