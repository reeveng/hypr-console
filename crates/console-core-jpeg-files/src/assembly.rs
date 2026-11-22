//! The planes of a picture put together into the pixels a screen draws.
//!
//! A JPEG keeps a photograph as brightness and two differences of color,
//! YCbCr, each in a plane of its own and the color usually smaller. Here the
//! three are read across a row together, turned into red, green and blue the
//! way JFIF says, and set down where the orientation puts them, so the picture
//! that comes out is the one the camera saw.
//!
//! A color plane smaller than the picture is stretched the way libjpeg
//! stretches one, each pixel taken from the two samples nearest its middle in
//! proportion to how near, across and down. Halfway values round in alternate
//! directions. With only one or two samples across, libjpeg repeats each
//! sample when doubling the width.
//! Read at a half or less, the color already arrives at the brightness's own
//! size and is used as it is.
//!
//! Three components are RGB rather than YCbCr when an Adobe segment says so
//! or the components are named R, G and B; one component is grey. Four are
//! ink, and Photoshop, which is what writes four, writes each as how much of
//! the paper is left showing rather than how much ink is on it -- as YCbCr
//! first when its Adobe segment says so -- so the paper left showing under the
//! black is the color, and that is how ffmpeg drew them here before.
//!
//! A row is colored once every row of every plane it reads from is drawn, so a
//! picture read in one scan is colored a round at a time, a round behind: the
//! rows one round finished are colored while the next is drawn and the one
//! after that is read. The planes hold those two rounds and the row or two the
//! earlier one reads from above it, the way libjpeg keeps a few rows rather
//! than the picture. The
//! colored rows go straight into the picture that is handed back, or, for a
//! photograph held some other way up, into a band that is then set down where
//! the orientation puts it. So the picture is never made twice.

use alloc::vec;
use alloc::vec::Vec;

use alloc::borrow::Cow;
use core::cmp::Ordering;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32};
use console_core_picture_scaling::{ChromaRounding, chroma_2x_rows};
use console_core_pixel_buffers::RowLayout;

use crate::JpegError;
use crate::exif::{Orientation, Placing};
use crate::frame::{self, Frame, Geometry, Layout};
use crate::scans::Image;
use crate::strips::{self, Spread, Task, Work};

const RGBA: u64 = 4;

const BAND_ROWS: u32 = 16;

const MIDDLE: i32 = 128;

const SCALE_BITS: u32 = 16;

const HALF: i32 = 1 << 15;

const RED_FROM_RED: i32 = 91_881;

const GREEN_FROM_BLUE: i32 = 22_554;

const GREEN_FROM_RED: i32 = 46_802;

const BLUE_FROM_BLUE: i32 = 116_130;

const WEIGHT_BITS: u32 = 8;

const WHOLE_WEIGHT: u32 = 1 << 8;

enum RowReadiness {
  Ready,
  Pending,
}

impl From<bool> for RowReadiness {
  fn from(ready: bool) -> Self {
    match bool::then_some(ready, ()) {
      Some(()) => Self::Ready,
      None => Self::Pending,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
  TwoComponent,
  Grey,
  YCbCr,
  Rgb,
  Cmyk,
  Ycck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Packing {
  Rgba,
  Components(u8),
}

impl Packing {
  fn channels(self) -> Result<u64, Never> {
    Ok(match self {
      Self::Rgba => RGBA,
      Self::Components(count) => u64::from(count),
    })
  }
}

pub(crate) struct Painting {
  pub(crate) bytes: Vec<u8>,
  packing: Packing,
  pub(crate) colored: u32,
  pub(crate) orientation: Orientation,
  columns: Vec<Vec<Taps>>,
}

#[derive(Clone, Copy)]
pub(crate) struct Window<'a> {
  pub(crate) samples: &'a [u8],
  pub(crate) top: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Paint<'a> {
  pub(crate) held: &'a [Window<'a>],
  pub(crate) geometry: &'a Geometry,
  pub(crate) colors: ColorSpace,
  pub(crate) through: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Coloring<'a> {
  planes: &'a [Window<'a>],
  layouts: &'a [Layout],
  columns: &'a [Vec<Taps>],
  colors: ColorSpace,
  packing: Packing,
  stored: Size<u32>,
  first: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Turning<'a> {
  band: &'a [u8],
  from: u32,
  orientation: Orientation,
  stored: Size<u32>,
  first: u32,
  columns: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stretch {
  shift: u32,
  samples: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Taps {
  near: u32,
  far: u32,
  toward: u32,
}

pub(crate) fn colors(frame: &Frame, adobe: Option<u8>) -> Result<ColorSpace, JpegError> {
  match (frame.components.as_slice(), adobe) {
    ([_, _], _) => Ok(ColorSpace::TwoComponent),
    ([_], None | Some(_)) => Ok(ColorSpace::Grey),
    ([_, _, _], Some(0)) => Ok(ColorSpace::Rgb),
    ([_, _, _], Some(1..)) => Ok(ColorSpace::YCbCr),
    ([first, second, third], None) => Ok(match (first.id, second.id, third.id) {
      (b'R', b'G', b'B') => ColorSpace::Rgb,
      (_, _, _) => ColorSpace::YCbCr,
    }),
    ([_, _, _, _], Some(2)) => Ok(ColorSpace::Ycck),
    ([_, _, _, _], None | Some(0 | 1 | 3..)) => Ok(ColorSpace::Cmyk),
    (_, None | Some(_)) => Err(JpegError::Corrupt),
  }
}

pub(crate) fn painting(
  geometry: &Geometry,
  orientation: Orientation,
  packing: Packing,
) -> Result<Painting, Never> {
  let Ok(upright) = orientation.upright(geometry.output);
  let Ok(channels) = packing.channels();
  let bytes = u64::from(upright.width)
    .saturating_mul(u64::from(upright.height))
    .saturating_mul(channels);
  let Ok(output_bytes) = index(bytes);
  let mut columns = Vec::new();

  for layout in &geometry.layouts {
    let Ok(taps) = column_taps(*layout, geometry.output.width);

    columns.push(taps);
  }

  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "image calls frame::affordable before creating the painting; rotation preserves the checked RGBA byte count"
    )
  )]
  let rgba = vec![0; output_bytes];

  Ok(Painting {
    bytes: rgba,
    packing,
    colored: 0,
    orientation,
    columns,
  })
}

pub(crate) fn painted(
  image: &mut Image,
  ready: &[u32],
  colors: ColorSpace,
  spread: Spread<'_>,
) -> Result<(), JpegError> {
  let Image {
    geometry,
    planes,
    painting,
    ..
  } = image;
  let Ok(through) = through(geometry, ready, painting.colored);
  let held: Vec<Window<'_>> = planes
    .iter()
    .map(|plane| Window {
      samples: &plane.samples,
      top: plane.top,
    })
    .collect();
  let mut band = Vec::new();
  let mut tasks = Vec::new();

  coloring(
    Paint {
      held: &held,
      geometry,
      colors,
      through,
    },
    (&mut *painting, &mut band),
    &mut tasks,
  )?;
  spread(&tasks)?;

  drop(tasks);

  set_down(painting, (&band, geometry.output), through, spread)
}

pub(crate) fn coloring<'a>(
  paint: Paint<'a>,
  into: (&'a mut Painting, &'a mut Vec<u8>),
  tasks: &mut Vec<Task<'a>>,
) -> Result<(), JpegError> {
  let Paint {
    held,
    geometry,
    colors,
    through,
  } = paint;
  let (painting, band) = into;
  let Painting {
    bytes: rgba,
    packing,
    colored,
    orientation,
    columns,
  } = painting;
  let from = *colored;

  match through.cmp(&from) {
    Ordering::Greater => {}
    Ordering::Less | Ordering::Equal => return Ok(()),
  }

  let stored = geometry.output;
  let Ok(channels) = packing.channels();
  let Ok(line) = index(u64::from(stored.width).saturating_mul(channels));
  let Ok(start) = index(from);
  let Ok(end) = index(through);
  let coloring = Coloring {
    planes: held,
    layouts: &geometry.layouts,
    columns,
    colors,
    packing: *packing,
    stored,
    first: from,
  };

  let Ok(placing) = orientation.placing();

  let rows = match placing {
    Placing::Straight => match rgba.get_mut(start.saturating_mul(line)..end.saturating_mul(line))
    {
      Some(rows) => rows,
      None => return Err(JpegError::Corrupt),
    },
    Placing::Turned => {
      let Ok(length) = index(end.saturating_sub(start).saturating_mul(line));
      let window = rgba.get(..length).ok_or(JpegError::Corrupt)?;

      band.clear();
      band.resize(window.len(), 0);
      band.as_mut_slice()
    }
  };

  colored_rows(rows, coloring, stored, tasks)
}

pub(crate) fn set_down(
  painting: &mut Painting,
  band: (&[u8], Size<u32>),
  through: u32,
  spread: Spread<'_>,
) -> Result<(), JpegError> {
  let (band_2, stored) = band;
  let from = painting.colored;

  let Ok(placing) = painting.orientation.placing();

  match (placing, through.cmp(&from)) {
    (Placing::Straight | Placing::Turned, Ordering::Less | Ordering::Equal) => return Ok(()),
    (Placing::Straight, Ordering::Greater) => {}
    (Placing::Turned, Ordering::Greater) => {
      let turning = Turning {
        band: band_2,
        from,
        orientation: painting.orientation,
        stored,
        first: 0,
        columns: (0, 0),
      };

      turned(&mut painting.bytes, turning, through, spread)?;
    }
  }

  painting.colored = through;

  Ok(())
}

pub(crate) fn through(geometry: &Geometry, ready: &[u32], from: u32) -> Result<u32, Never> {
  let mut through = from;

  for down in from..geometry.output.height {
    let readiness = RowReadiness::from(
      geometry.layouts.iter().zip(ready).all(|(layout, ready_2)| {
        let Ok((_, last)) = needed(*layout, down);

        last < *ready_2
      }),
    );

    match readiness {
      RowReadiness::Ready => through = down.saturating_add(1),
      RowReadiness::Pending => break,
    }
  }

  Ok(through)
}

pub(crate) fn needed(layout: Layout, down: u32) -> Result<(u32, u32), Never> {
  let Ok(size) = frame::plane(layout);
  let Ok(rows) = taps(
    down,
    Stretch {
      shift: layout.stretched.height,
      samples: size.height,
    },
  );

  Ok(match (layout.stretched.width, layout.stretched.height) {
    (0, 0) => (rows.near, rows.near),
    (_, _) => (rows.near, rows.far),
  })
}

fn colored_rows<'a>(
  rows: &'a mut [u8],
  coloring: Coloring<'a>,
  stored: Size<u32>,
  tasks: &mut Vec<Task<'a>>,
) -> Result<(), JpegError> {
  let Ok(channels) = coloring.packing.channels();
  let band = band(stored, channels)?;
  let Ok(band_2) = index(band);

  for (at, strip) in (0u32..).zip(rows.chunks_mut(band_2.max(1))) {
    let first = coloring.first.saturating_add(at.saturating_mul(BAND_ROWS));
    let Ok(one) = strips::task(strip, Work::Colored(Coloring { first, ..coloring }));

    tasks.push(one);
  }

  Ok(())
}

fn turned(
  rgba: &mut [u8],
  turning: Turning<'_>,
  through: u32,
  spread: Spread<'_>,
) -> Result<(), JpegError> {
  let Turning {
    from,
    orientation,
    stored,
    ..
  } = turning;
  let Ok(upright) = orientation.upright(stored);
  let height = stored.height;

  let (rows, columns) = match orientation {
    Orientation::AsStored | Orientation::Mirrored => ((from, through), (0, upright.width)),
    Orientation::HalfTurn | Orientation::Flipped => (
      (height.saturating_sub(through), height.saturating_sub(from)),
      (0, upright.width),
    ),
    Orientation::Transposed | Orientation::QuarterTurnCounterclockwise => {
      ((0, upright.height), (from, through))
    }
    Orientation::QuarterTurnClockwise | Orientation::Transversed => (
      (0, upright.height),
      (height.saturating_sub(through), height.saturating_sub(from)),
    ),
  };

  let Ok(line) = index(u64::from(upright.width).saturating_mul(RGBA));
  let Ok(start) = index(rows.0);
  let Ok(end) = index(rows.1);
  let band = band(upright, RGBA)?;
  let Ok(band_2) = index(band);

  let touched = match rgba.get_mut(start.saturating_mul(line)..end.saturating_mul(line)) {
    Some(touched) => touched,
    None => return Err(JpegError::Corrupt),
  };

  let mut tasks = Vec::new();

  for (at, strip) in (0u32..).zip(touched.chunks_mut(band_2.max(1))) {
    let first = rows.0.saturating_add(at.saturating_mul(BAND_ROWS));
    let Ok(one) = strips::task(
      strip,
      Work::Turned(Turning {
        first,
        columns,
        ..turning
      }),
    );

    tasks.push(one);
  }

  spread(&tasks)
}

fn band(size: Size<u32>, channels: u64) -> Result<u64, JpegError> {
  match u64::from(size.width)
    .saturating_mul(channels)
    .saturating_mul(u64::from(BAND_ROWS))
  {
    0 => Err(JpegError::Corrupt),
    band => Ok(band),
  }
}

pub(crate) fn colored(strip: &mut [u8], coloring: &Coloring<'_>) -> Result<(), JpegError> {
  let Ok(channels) = coloring.packing.channels();
  let Ok(line) = index(u64::from(coloring.stored.width).saturating_mul(channels));

  match line {
    0 => return Err(JpegError::Corrupt),
    _ => {}
  }

  for (down, pixels) in (coloring.first..).zip(strip.chunks_mut(line.max(1))) {
    let rows = rows_at(coloring, down)?;

    match coloring.packing {
      Packing::Rgba => row(pixels, coloring.colors, &rows)?,
      Packing::Components(count) => {
        let Ok(stride) = index(count);

        for (component, source) in rows.iter().enumerate() {
          let column = pixels.iter_mut().skip(component).step_by(stride.max(1));

          for (sample, original) in column.zip(source.iter()) {
            *sample = *original;
          }
        }
      }
    }
  }

  Ok(())
}

pub(crate) fn turned_into(strip: &mut [u8], turning: &Turning<'_>) -> Result<(), JpegError> {
  let Ok(upright) = turning.orientation.upright(turning.stored);
  let Ok(line) = index(u64::from(upright.width).saturating_mul(RGBA));
  let (left, right) = turning.columns;
  let Ok(skipped) = index(left);
  let Ok(taken) = index(right.saturating_sub(left));

  match line {
    0 => return Err(JpegError::Corrupt),
    _ => {}
  }

  let stored = turning.band.as_chunks::<4>().0;

  for (down, pixels) in (turning.first..).zip(strip.chunks_mut(line.max(1))) {
    for (across, pixel) in (left..).zip(
      pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .skip(skipped)
        .take(taken),
    ) {
      let Ok(from) = turning
        .orientation
        .source(Point { x: across, y: down }, turning.stored);

      let row = match from.y.checked_sub(turning.from) {
        Some(row) => row,
        None => return Err(JpegError::Corrupt),
      };

      let Ok(at) = index(
        u64::from(row)
          .saturating_mul(u64::from(turning.stored.width))
          .saturating_add(u64::from(from.x)),
      );

      match stored.get(at) {
        Some(source) => *pixel = *source,
        None => return Err(JpegError::Corrupt),
      }
    }
  }

  Ok(())
}

fn column_taps(layout: Layout, wide: u32) -> Result<Vec<Taps>, Never> {
  let samples = wide.div_ceil(1u32.wrapping_shl(layout.stretched.width));

  Ok(match (layout.stretched.width, layout.stretched.height) {
    (0, 0) | (1, 1) => Vec::new(),
    (_, _) => (0..wide)
      .map(|at| {
        let Ok(tapped) = taps(
          at,
          Stretch {
            shift: layout.stretched.width,
            samples,
          },
        );

        tapped
      })
      .collect::<Vec<Taps>>(),
  })
}

fn rows_at<'i>(coloring: &Coloring<'i>, down: u32) -> Result<Vec<Cow<'i, [u8]>>, JpegError> {
  let mut rows = Vec::new();

  for ((plane, layout), taps) in coloring
    .planes
    .iter()
    .zip(coloring.layouts)
    .zip(coloring.columns)
  {
    let line = line(plane, *layout, (taps, down, coloring.stored))?;

    rows.push(line);
  }

  Ok(rows)
}

fn taps(at: u32, stretch: Stretch) -> Result<Taps, Never> {
  let Stretch { shift, samples } = stretch;
  let factor = f64::from(1u32.wrapping_shl(shift));
  let middle = ((f64::from(at) + 0.5) / factor - 0.5).max(0.0);
  let Ok(near) = toward_zero_u32(middle);
  let last = samples.saturating_sub(1);
  let Ok(toward) = toward_zero_u32((middle - f64::from(near)) * f64::from(WHOLE_WEIGHT) + 0.5);

  Ok(Taps {
    near: near.min(last),
    far: near.saturating_add(1).min(last),
    toward,
  })
}

fn plane_row<'p>(plane: &Window<'p>, size: Size<u32>, row: u32) -> Result<&'p [u8], JpegError> {
  let held = match row.checked_sub(plane.top) {
    Some(held) => held,
    None => return Err(JpegError::Corrupt),
  };

  let rows = Size {
    width: u64::from(size.width),
    height: u64::from(held).wrapping_add(1),
  };
  RowLayout::new(rows, 0, i64::from(size.width))
    .and_then(|layout| layout.row(plane.samples, u64::from(held)))
    .map_err(|_| JpegError::Corrupt)
}

fn line<'p>(
  plane: &Window<'p>,
  layout: Layout,
  at: (&[Taps], u32, Size<u32>),
) -> Result<Cow<'p, [u8]>, JpegError> {
  let (columns, down, stored) = at;
  let Ok(size) = frame::plane(layout);
  let sample_rows = stored
    .height
    .div_ceil(1u32.wrapping_shl(layout.stretched.height));

  match (
    layout.stretched.width,
    layout.stretched.height,
    stored.width,
  ) {
    (1, shift @ (0 | 1), 0..=4) => {
      let source = plane_row(plane, size, down.wrapping_shr(shift))?;
      let Ok(width) = index(stored.width);
      let samples = source
        .iter()
        .flat_map(|sample| [*sample; 2])
        .take(width)
        .collect();

      return Ok(Cow::Owned(samples));
    }
    (1, 1, _) => {
      let (near, far) = chroma_2x_rows(down, sample_rows);
      let Ok(width) = index(stored.width);
      let Ok(samples) = index(stored.width.div_ceil(2));
      let near_row = plane_row(plane, size, near)?;
      let far_row = plane_row(plane, size, far)?;
      let near_samples = near_row.get(..samples).ok_or(JpegError::Corrupt)?;
      let far_samples = far_row.get(..samples).ok_or(JpegError::Corrupt)?;
      let mut output = vec![0; width.min(near_samples.len().saturating_mul(2))];

      ChromaRounding::AlternateColumns
        .upsample_2x(near_samples, far_samples, &mut output)
        .map_err(|_| JpegError::Corrupt)?;

      return Ok(Cow::Owned(output));
    }
    (_, _, _) => {}
  }

  let Ok(rows) = taps(
    down,
    Stretch {
      shift: layout.stretched.height,
      samples: sample_rows,
    },
  );
  let near = plane_row(plane, size, rows.near)?;

  match (layout.stretched.width, layout.stretched.height) {
    (0, 0) => return Ok(Cow::Borrowed(near)),
    (_, _) => {}
  }

  let far = plane_row(plane, size, rows.far)?;

  let staying = WHOLE_WEIGHT.saturating_sub(rows.toward);

  let mixed: Vec<u32> = near
    .iter()
    .zip(far)
    .map(|(near_2, far_2)| {
      u32::from(*near_2)
        .saturating_mul(staying)
        .saturating_add(u32::from(*far_2).saturating_mul(rows.toward))
    })
    .collect();

  let mut stretched = Vec::with_capacity(columns.len());
  let half = 1u32.wrapping_shl(WEIGHT_BITS.saturating_mul(2).saturating_sub(1));
  let biases = match (layout.stretched.width, layout.stretched.height) {
    (1, 0) => [half.saturating_sub(1), half],
    (0, 1) => [half.saturating_sub(u32::from(down & 1 == 0)); 2],
    (_, _) => [half; 2],
  };

  let rounding = core::iter::repeat_n(biases, columns.len().div_ceil(2)).flatten();

  for (tapped, bias) in columns.iter().zip(rounding) {
    let Ok(near_3) = index(tapped.near);
    let Ok(far_3) = index(tapped.far);
    let kept = WHOLE_WEIGHT.saturating_sub(tapped.toward);

    let level = match (mixed.get(near_3), mixed.get(far_3)) {
      (Some(near_4), Some(far_4)) => near_4
        .saturating_mul(kept)
        .saturating_add(far_4.saturating_mul(tapped.toward)),
      (None, _) | (_, None) => return Err(JpegError::Corrupt),
    };

    let Ok(level_2) = fitted::<u32, u8>(
      level
        .saturating_add(bias)
        .wrapping_shr(WEIGHT_BITS.saturating_mul(2)),
    );

    stretched.push(level_2);
  }

  Ok(Cow::Owned(stretched))
}

fn row(pixels: &mut [u8], colors: ColorSpace, rows: &[Cow<'_, [u8]>]) -> Result<(), JpegError> {
  match (colors, rows) {
    (ColorSpace::Grey, [grey]) => written(pixels, grey.iter().map(|level| [*level; 3])),
    (ColorSpace::YCbCr, [luma, blue, red]) => written(
      pixels,
      luma
        .iter()
        .zip(blue.iter().zip(red.iter()))
        .map(|(luma_2, (blue_2, red_2))| {
          let Ok(rgb) = converted([*luma_2, *blue_2, *red_2]);

          rgb
        }),
    ),
    (ColorSpace::Rgb, [red, green, blue]) => written(
      pixels,
      red
        .iter()
        .zip(green.iter().zip(blue.iter()))
        .map(|(red_3, (green_2, blue_3))| [*red_3, *green_2, *blue_3]),
    ),
    (ColorSpace::Cmyk, [cyan, magenta, yellow, black]) => written(
      pixels,
      cyan
        .iter()
        .zip(magenta.iter())
        .zip(yellow.iter().zip(black.iter()))
        .map(|((cyan_2, magenta_2), (yellow_2, black_2))| {
          let Ok(rgb) = under_black([*cyan_2, *magenta_2, *yellow_2], *black_2);

          rgb
        }),
    ),
    (ColorSpace::Ycck, [luma, blue, red, black]) => written(
      pixels,
      luma
        .iter()
        .zip(blue.iter())
        .zip(red.iter().zip(black.iter()))
        .map(|((luma_3, blue_4), (red_4, black_3))| {
          let Ok(inked) = converted([*luma_3, *blue_4, *red_4]);
          let Ok(rgb) = under_black(inked.map(|level| u8::MAX.saturating_sub(level)), *black_3);

          rgb
        }),
    ),
    (
      ColorSpace::TwoComponent
      | ColorSpace::Grey
      | ColorSpace::YCbCr
      | ColorSpace::Rgb
      | ColorSpace::Cmyk
      | ColorSpace::Ycck,
      _,
    ) => Err(JpegError::Corrupt),
  }
}

fn written(pixels: &mut [u8], colors: impl Iterator<Item = [u8; 3]>) -> Result<(), JpegError> {
  for (slot, [red, green, blue]) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(colors) {
    *slot = [red, green, blue, u8::MAX];
  }

  Ok(())
}

fn under_black(showing: [u8; 3], black: u8) -> Result<[u8; 3], Never> {
  Ok(showing.map(|level| {
    let mixed = u32::from(level)
      .saturating_mul(u32::from(black))
      .saturating_mul(257)
      .wrapping_shr(16);
    let Ok(mixed_2) = fitted::<u32, u8>(mixed);

    mixed_2
  }))
}

pub(crate) fn converted(sample: [u8; 3]) -> Result<[u8; 3], Never> {
  let [luma, blue, red] = sample.map(i32::from);
  let blue_2 = blue.wrapping_sub(MIDDLE);
  let red_2 = red.wrapping_sub(MIDDLE);
  let reddened = RED_FROM_RED
    .wrapping_mul(red_2)
    .wrapping_add(HALF)
    .wrapping_shr(SCALE_BITS);
  let greened = HALF
    .wrapping_sub(GREEN_FROM_BLUE.wrapping_mul(blue_2))
    .wrapping_sub(GREEN_FROM_RED.wrapping_mul(red_2))
    .wrapping_shr(SCALE_BITS);
  let blued = BLUE_FROM_BLUE
    .wrapping_mul(blue_2)
    .wrapping_add(HALF)
    .wrapping_shr(SCALE_BITS);
  let Ok(r) = fitted::<i32, u8>(luma.wrapping_add(reddened));
  let Ok(g) = fitted::<i32, u8>(luma.wrapping_add(greened));
  let Ok(b) = fitted::<i32, u8>(luma.wrapping_add(blued));

  Ok([r, g, b])
}
