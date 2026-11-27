use console_core_api_compatibility::NativeSize;
use console_core_geometry::{Point, Size};
use console_core_number_conversion::{index, indices};
use console_core_pixel_buffers::{RowLayout, RowLayoutError};
use console_core_video_interpolation::round_shift;
use core::cmp::Ordering;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

const MAX_BLOCK: u64 = 128;
pub const SCRATCH_COLUMNS: NativeSize = 128;
pub const SCRATCH_ROWS: NativeSize = 135;

#[derive(Clone, Copy)]
pub struct EightTap {
  pub horizontal: Option<&'static [i8; 8]>,
  pub vertical: Option<&'static [i8; 8]>,
}

impl EightTap {
  pub fn filtered_axes(self) -> Result<Point<bool>, console_core_never::Never> {
    Ok(Point {
      x: self.horizontal.is_some(),
      y: self.vertical.is_some(),
    })
  }
}

#[derive(Clone, Copy)]
pub struct FilterFootprint {
  pub before: Point<u64>,
  pub size: Size<u64>,
}

/// AV1's eight taps are centered three samples before the output coordinate.
/// Each filtered axis needs three preceding and four following samples.
pub fn read_extent(
  size: Size<u64>,
  filtered: Point<bool>,
) -> Result<FilterFootprint, RowLayoutError> {
  match (size.width, size.height) {
    (1..=MAX_BLOCK, 1..=MAX_BLOCK) => {}
    _invalid_size => return Err(RowLayoutError::Bounds),
  }

  let horizontal = u64::from(filtered.x);
  let vertical = u64::from(filtered.y);
  Ok(FilterFootprint {
    before: Point {
      x: horizontal.wrapping_mul(3),
      y: vertical.wrapping_mul(3),
    },
    size: Size {
      width: size.width.wrapping_add(horizontal.wrapping_mul(7)),
      height: size.height.wrapping_add(vertical.wrapping_mul(7)),
    },
  })
}

impl FilterFootprint {
  /// Finds the first sample of the validated input rectangle.
  pub fn start(self, origin: u64, stride: i64) -> Result<u64, RowLayoutError> {
    let above = i64::try_from(self.before.y).map_err(|_| RowLayoutError::Bounds)?;
    let displacement = above
      .wrapping_neg()
      .checked_mul(stride)
      .ok_or(RowLayoutError::Bounds)?;
    origin
      .checked_add_signed(displacement)
      .and_then(|row| row.checked_sub(self.before.x))
      .ok_or(RowLayoutError::Bounds)
  }
}

pub struct PlaneRef<'a, Pixel> {
  data: &'a [Pixel],
  layout: RowLayout,
  stride: i64,
  first: u64,
}

impl<'a, Pixel> PlaneRef<'a, Pixel> {
  pub fn new(
    data: &'a [Pixel],
    size: Size<u64>,
    first: u64,
    stride: i64,
  ) -> Result<Self, RowLayoutError> {
    let layout = RowLayout::new(size, first, stride)?;
    layout.validate_len(data.len())?;

    Ok(Self {
      data,
      layout,
      stride,
      first,
    })
  }

  #[inline]
  pub fn row(&self, row: u64) -> Result<&'a [Pixel], RowLayoutError> {
    self.layout.row(self.data, row)
  }
}

pub struct PlaneMut<'a, Pixel> {
  data: &'a mut [Pixel],
  layout: RowLayout,
  size: Size<u64>,
}

impl<'a, Pixel> PlaneMut<'a, Pixel> {
  pub fn new(
    data: &'a mut [Pixel],
    size: Size<u64>,
    first: u64,
    stride: i64,
  ) -> Result<Self, RowLayoutError> {
    let layout = RowLayout::new(size, first, stride)?;
    layout.validate_len(data.len())?;

    Ok(Self { data, layout, size })
  }
}

pub struct Prediction<'a, Output> {
  source: PlaneRef<'a, u8>,
  output: PlaneMut<'a, Output>,
  filter: EightTap,
}

impl<'a, Output> Prediction<'a, Output> {
  pub fn new(
    output: PlaneMut<'a, Output>,
    input: &'a [u8],
    origin: u64,
    stride: i64,
    filter: EightTap,
  ) -> Result<Self, RowLayoutError> {
    let Ok(axes) = filter.filtered_axes();
    let footprint = read_extent(output.size, axes)?;
    let start = footprint.start(origin, stride)?;
    let source = PlaneRef::new(input, footprint.size, start, stride)?;

    Ok(Self {
      source,
      output,
      filter,
    })
  }
}

/// One aligned 128×135 buffer per decoder worker, separate from prep output.
/// The first pass requires at most the block height plus seven filter rows.
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct MotionScratch([[i16; SCRATCH_COLUMNS]; SCRATCH_ROWS]);

pub struct VerticalInput<'a> {
  source: &'a PlaneRef<'a, u8>,
  row: u64,
  width: u64,
}

impl<'a> VerticalInput<'a> {
  pub fn rows(&self) -> Result<[&'a [u8]; 8], RowLayoutError> {
    let mut rows = [&[][..]; 8];
    for (target, ordinal) in rows.iter_mut().zip(self.row..) {
      *target = self.source.row(ordinal)?;
    }

    Ok(rows)
  }

  /// Returns the eight-row hull of a forward-strided view for vector
  /// kernels that load taps at a fixed distance. Negative strides use rows().
  pub fn forward_span(&self) -> Result<Option<(&'a [u8], i64)>, RowLayoutError> {
    let stride = self.source.stride;
    match stride.cmp(&0) {
      Ordering::Less | Ordering::Equal => return Ok(None),
      Ordering::Greater => {}
    }

    let pitch = stride.cast_unsigned();
    let start = self.source.first.wrapping_add(self.row.wrapping_mul(pitch));
    let end = start
      .wrapping_add(pitch.wrapping_mul(7))
      .wrapping_add(self.width);
    let Ok(range) = indices(start..end);
    let data = self.source.data.get(range).ok_or(RowLayoutError::Bounds)?;
    Ok(Some((data, stride)))
  }
}

pub struct RowFilters<CopyRow, Horizontal, Intermediate, Vertical, Separable> {
  pub copy: CopyRow,
  pub horizontal: Horizontal,
  pub intermediate: Intermediate,
  pub vertical: Vertical,
  pub separable: Separable,
}

#[inline]
pub fn apply<Output>(
  prediction: Prediction<'_, Output>,
  scratch: &mut MotionScratch,
  filters: RowFilters<
    impl Fn(&mut [Output], &[u8]) -> Result<(), RowLayoutError>,
    impl Fn(&mut [Output], &[u8], &[i8; 8]) -> Result<(), RowLayoutError>,
    impl Fn(&mut [i16], &[u8], &[i8; 8]) -> Result<(), RowLayoutError>,
    impl Fn(
      &mut [Output],
      VerticalInput<'_>,
      &[i8; 8],
      &mut [[i16; SCRATCH_COLUMNS]; 8],
    ) -> Result<(), RowLayoutError>,
    impl Fn(&mut [Output], &[[i16; SCRATCH_COLUMNS]], &[i8; 8]) -> Result<(), RowLayoutError>,
  >,
) -> Result<(), RowLayoutError> {
  let Prediction {
    source,
    output,
    filter,
  } = prediction;
  let rows = output.layout.rows_mut(output.data)?;
  match (filter.horizontal, filter.vertical) {
    (None, None) => {
      for (target, input) in rows.zip(source.layout.rows(source.data)?) {
        (filters.copy)(target, input)?;
      }
    }
    (Some(taps), None) => {
      for (target, input) in rows.zip(source.layout.rows(source.data)?) {
        (filters.horizontal)(target, input, taps)?;
      }
    }
    (None, Some(taps)) => {
      let intermediate = scratch
        .0
        .first_chunk_mut::<8>()
        .ok_or(RowLayoutError::Bounds)?;
      for (row, target) in (0..).zip(rows) {
        (filters.vertical)(
          target,
          VerticalInput {
            source: &source,
            row,
            width: output.size.width,
          },
          taps,
          intermediate,
        )?;
      }
    }
    (Some(horizontal), Some(vertical)) => {
      let Ok(width) = index(output.size.width);
      let Ok(count) = index(output.size.height.wrapping_add(7));
      let intermediate = scratch.0.get_mut(..count).ok_or(RowLayoutError::Bounds)?;

      for (target, input) in intermediate
        .iter_mut()
        .zip(source.layout.rows(source.data)?)
      {
        let samples = target.get_mut(..width).ok_or(RowLayoutError::Bounds)?;
        (filters.intermediate)(samples, input, horizontal)?;
      }

      for (row, target) in rows.enumerate() {
        let tail = intermediate.get(row..).ok_or(RowLayoutError::Bounds)?;
        (filters.separable)(target, tail, vertical)?;
      }
    }
  }

  Ok(())
}

pub type Put8 = for<'a> fn(Prediction<'a, u8>, &mut MotionScratch) -> Result<(), RowLayoutError>;
pub type Prep8 =
  for<'a> fn(Prediction<'a, i16>, &mut MotionScratch) -> Result<(), RowLayoutError>;

#[derive(Clone, Copy)]
pub struct MotionCompensation {
  pub put: Put8,
  pub prep: Prep8,
}

impl Default for MotionCompensation {
  fn default() -> Self {
    Self {
      put: scalar_put,
      prep: scalar_prep,
    }
  }
}

fn horizontal_sum(source: &[u8], weights: &[i8; 8]) -> i32 {
  source
    .iter()
    .zip(weights)
    .fold(0, |sum, (&pixel, &weight)| {
      sum + i32::from(pixel) * i32::from(weight)
    })
}

fn horizontal(output: &mut [i16], input: &[u8], taps: &[i8; 8]) -> Result<(), RowLayoutError> {
  for (target, window) in output.iter_mut().zip(input.windows(8)) {
    let sum = round_shift(horizontal_sum(window, taps), 2);
    let Ok(pixel) = console_core_number_conversion::wrapped(sum);
    *target = pixel;
  }

  Ok(())
}

fn vertical<Sample: Copy + Into<i32>, Output>(
  output: &mut [Output],
  rows: [&[Sample]; 8],
  taps: &[i8; 8],
  mut finish: impl FnMut(&mut Output, i32),
) -> Result<(), RowLayoutError> {
  for (column, target) in output.iter_mut().enumerate() {
    let mut sum = 0_i32;
    for (row, &tap) in rows.iter().zip(taps) {
      let &sample = row.get(column).ok_or(RowLayoutError::Bounds)?;
      sum += sample.into() * i32::from(tap);
    }

    finish(target, sum);
  }

  Ok(())
}

fn byte(sum: i32) -> u8 {
  let Ok(pixel) = console_core_number_conversion::wrapped(sum.clamp(0, 255));
  pixel
}

pub fn copy_pixels(target: &mut [u8], source: &[u8]) -> Result<(), RowLayoutError> {
  for (pixel, &sample) in target.iter_mut().zip(source) {
    *pixel = sample;
  }
  Ok(())
}

pub fn copy_predictors(target: &mut [i16], source: &[u8]) -> Result<(), RowLayoutError> {
  for (pixel, &sample) in target.iter_mut().zip(source) {
    *pixel = i16::from(sample).wrapping_shl(4);
  }
  Ok(())
}

pub fn scalar_vertical_put(
  target: &mut [u8],
  source: VerticalInput<'_>,
  taps: &[i8; 8],
) -> Result<(), RowLayoutError> {
  vertical(target, source.rows()?, taps, |pixel, sum| {
    *pixel = byte(round_shift(sum, 6));
  })
}

fn scalar_put(
  prediction: Prediction<'_, u8>,
  scratch: &mut MotionScratch,
) -> Result<(), RowLayoutError> {
  apply(
    prediction,
    scratch,
    RowFilters {
      copy: copy_pixels,
      horizontal: |target: &mut [u8], source: &[u8], taps: &[i8; 8]| {
        for (pixel, window) in target.iter_mut().zip(source.windows(8)) {
          *pixel = byte((horizontal_sum(window, taps) + 34).wrapping_shr(6));
        }
        Ok(())
      },
      intermediate: horizontal,
      vertical: |target: &mut [u8],
                 source: VerticalInput<'_>,
                 taps: &[i8; 8],
                 _: &mut [[i16; SCRATCH_COLUMNS]; 8]| {
        scalar_vertical_put(target, source, taps)
      },
      separable: |target: &mut [u8], source: &[[i16; SCRATCH_COLUMNS]], taps: &[i8; 8]| {
        let rows = source.first_chunk::<8>().ok_or(RowLayoutError::Bounds)?;
        vertical(
          target,
          rows.each_ref().map(<[_; SCRATCH_COLUMNS]>::as_slice),
          taps,
          |pixel, sum| {
            *pixel = byte(round_shift(sum, 10));
          },
        )
      },
    },
  )
}

fn scalar_prep(
  prediction: Prediction<'_, i16>,
  scratch: &mut MotionScratch,
) -> Result<(), RowLayoutError> {
  let finish = |pixel: &mut i16, sum, shift| {
    let Ok(value) = console_core_number_conversion::wrapped(round_shift(sum, shift));
    *pixel = value;
  };
  apply(
    prediction,
    scratch,
    RowFilters {
      copy: copy_predictors,
      horizontal,
      intermediate: horizontal,
      vertical: |target: &mut [i16],
                 source: VerticalInput<'_>,
                 taps: &[i8; 8],
                 _: &mut [[i16; SCRATCH_COLUMNS]; 8]| {
        vertical(target, source.rows()?, taps, |pixel, sum| {
          finish(pixel, sum, 2)
        })
      },
      separable: |target: &mut [i16], source: &[[i16; SCRATCH_COLUMNS]], taps: &[i8; 8]| {
        let rows = source.first_chunk::<8>().ok_or(RowLayoutError::Bounds)?;
        vertical(
          target,
          rows.each_ref().map(<[_; SCRATCH_COLUMNS]>::as_slice),
          taps,
          |pixel, sum| finish(pixel, sum, 6),
        )
      },
    },
  )
}
