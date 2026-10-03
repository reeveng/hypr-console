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
//! proportion to how near, across and down. Repeating each sample instead is
//! cheaper and leaves a staircase of color along every edge at full size.
//! Read at a half or less, the color already arrives at the brightness's own
//! size and is used as it is.
//!
//! Three components are RGB rather than YCbCr when an Adobe segment says so
//! or the components are named R, G and B; one component is grey.
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

use std::borrow::Cow;

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32, whole_u32};

use crate::exif::{Orientation, Placing};
use crate::frame::{self, Frame, Geometry, Layout};
use crate::scans::Image;
use crate::strips::{self, Spread, Task, Work};
use crate::JpegError;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Colors {
    Grey,
    YCbCr,
    Rgb,
}

pub(crate) struct Painting {
    pub(crate) rgba: Vec<u8>,
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
    pub(crate) colors: Colors,
    pub(crate) through: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct Coloring<'a> {
    planes: &'a [Window<'a>],
    layouts: &'a [Layout],
    columns: &'a [Vec<Taps>],
    colors: Colors,
    wide: u32,
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

pub(crate) fn colors(frame: &Frame, adobe: Option<u8>) -> Result<Colors, JpegError> {
    match (frame.components.as_slice(), adobe) {
        ([_], None | Some(_)) => Ok(Colors::Grey),
        ([_, _, _], Some(0)) => Ok(Colors::Rgb),
        ([_, _, _], Some(1..)) => Ok(Colors::YCbCr),
        ([first, second, third], None) => Ok(match (first.id, second.id, third.id) {
            (b'R', b'G', b'B') => Colors::Rgb,
            (_, _, _) => Colors::YCbCr,
        }),
        (_, None | Some(_)) => Err(JpegError::Corrupt),
    }
}

pub(crate) fn painting(geometry: &Geometry, orientation: Orientation) -> Result<Painting, Never> {
    let Ok(upright) = orientation.upright(geometry.output);
    let Ok(area) = area(upright);
    let Ok(area) = index(area);
    let mut columns = Vec::new();

    for layout in &geometry.layouts {
        let Ok(taps) = column_taps(*layout, geometry.output.width);

        columns.push(taps);
    }

    Ok(Painting { rgba: vec![0; area], colored: 0, orientation, columns })
}

pub(crate) fn painted(image: &mut Image, ready: &[u32], colors: Colors, spread: Spread<'_>) -> Result<(), JpegError> {
    let Image { geometry, planes, painting, .. } = image;
    let Ok(through) = through(geometry, ready, painting.colored);
    let held: Vec<Window<'_>> = planes.iter().map(|plane| Window { samples: &plane.samples, top: plane.top }).collect();
    let mut band = Vec::new();
    let mut tasks = Vec::new();

    coloring(Paint { held: &held, geometry, colors, through }, (&mut *painting, &mut band), &mut tasks)?;
    spread(&tasks)?;

    drop(tasks);

    set_down(painting, (&band, geometry.output), through, spread)
}

pub(crate) fn coloring<'a>(paint: Paint<'a>, into: (&'a mut Painting, &'a mut Vec<u8>), tasks: &mut Vec<Task<'a>>) -> Result<(), JpegError> {
    let Paint { held, geometry, colors, through } = paint;
    let (painting, band) = into;
    let Painting { rgba, colored, orientation, columns } = painting;
    let from = *colored;

    match through > from {
        true => {},
        false => return Ok(()),
    }

    let stored = geometry.output;
    let Ok(line) = index(u64::from(stored.width).saturating_mul(RGBA));
    let Ok(start) = index(from);
    let Ok(end) = index(through);
    let coloring = Coloring { planes: held, layouts: &geometry.layouts, columns, colors, wide: stored.width, first: from };

    let Ok(placing) = orientation.placing();

    let rows = match placing {
        Placing::Straight => match rgba.get_mut(start.saturating_mul(line)..end.saturating_mul(line)) {
            Some(rows) => rows,
            None => return Err(JpegError::Corrupt),
        },
        Placing::Turned => {
            band.clear();
            band.resize(end.saturating_sub(start).saturating_mul(line), 0);
            band.as_mut_slice()
        },
    };

    colored_rows(rows, coloring, stored, tasks)
}

pub(crate) fn set_down(painting: &mut Painting, band: (&[u8], Size<u32>), through: u32, spread: Spread<'_>) -> Result<(), JpegError> {
    let (band, stored) = band;
    let from = painting.colored;

    let Ok(placing) = painting.orientation.placing();

    match (placing, through > from) {
        (_, false) => return Ok(()),
        (Placing::Straight, true) => {},
        (Placing::Turned, true) => {
            let turning = Turning { band, from, orientation: painting.orientation, stored, first: 0, columns: (0, 0) };

            turned(&mut painting.rgba, turning, through, spread)?;
        },
    }

    painting.colored = through;

    Ok(())
}

pub(crate) fn through(geometry: &Geometry, ready: &[u32], from: u32) -> Result<u32, Never> {
    let mut through = from;

    for down in from..geometry.output.height {
        let drawn = geometry.layouts.iter().zip(ready).all(|(layout, ready)| {
            let Ok((_, last)) = needed(*layout, down);

            last < *ready
        });

        match drawn {
            true => through = down.saturating_add(1),
            false => break,
        }
    }

    Ok(through)
}

pub(crate) fn needed(layout: Layout, down: u32) -> Result<(u32, u32), Never> {
    let Ok(size) = frame::plane(layout);
    let Ok(rows) = taps(down, Stretch { shift: layout.stretched.height, samples: size.height });

    Ok(match (layout.stretched.width, layout.stretched.height) {
        (0, 0) => (rows.near, rows.near),
        (_, _) => (rows.near, rows.far),
    })
}

fn colored_rows<'a>(rows: &'a mut [u8], coloring: Coloring<'a>, stored: Size<u32>, tasks: &mut Vec<Task<'a>>) -> Result<(), JpegError> {
    let band = band(stored)?;
    let Ok(band) = index(band);

    for (at, strip) in (0u32..).zip(rows.chunks_mut(band)) {
        let first = coloring.first.saturating_add(at.saturating_mul(BAND_ROWS));
        let Ok(one) = strips::task(strip, Work::Colored(Coloring { first, ..coloring }));

        tasks.push(one);
    }

    Ok(())
}

fn turned(rgba: &mut [u8], turning: Turning<'_>, through: u32, spread: Spread<'_>) -> Result<(), JpegError> {
    let Turning { from, orientation, stored, .. } = turning;
    let Ok(upright) = orientation.upright(stored);
    let height = stored.height;

    let (rows, columns) = match orientation {
        Orientation::AsStored | Orientation::Mirrored => ((from, through), (0, upright.width)),
        Orientation::HalfTurn | Orientation::Flipped => ((height.saturating_sub(through), height.saturating_sub(from)), (0, upright.width)),
        Orientation::Transposed | Orientation::QuarterTurnCounterclockwise => ((0, upright.height), (from, through)),
        Orientation::QuarterTurnClockwise | Orientation::Transversed => {
            ((0, upright.height), (height.saturating_sub(through), height.saturating_sub(from)))
        },
    };

    let Ok(line) = index(u64::from(upright.width).saturating_mul(RGBA));
    let Ok(start) = index(rows.0);
    let Ok(end) = index(rows.1);
    let band = band(upright)?;
    let Ok(band) = index(band);

    let touched = match rgba.get_mut(start.saturating_mul(line)..end.saturating_mul(line)) {
        Some(touched) => touched,
        None => return Err(JpegError::Corrupt),
    };

    let mut tasks = Vec::new();

    for (at, strip) in (0u32..).zip(touched.chunks_mut(band)) {
        let first = rows.0.saturating_add(at.saturating_mul(BAND_ROWS));
        let Ok(one) = strips::task(strip, Work::Turned(Turning { first, columns, ..turning }));

        tasks.push(one);
    }

    spread(&tasks)
}

fn area(size: Size<u32>) -> Result<u64, Never> {
    Ok(u64::from(size.width).saturating_mul(u64::from(size.height)).saturating_mul(RGBA))
}

fn band(size: Size<u32>) -> Result<u64, JpegError> {
    match u64::from(size.width).saturating_mul(RGBA).saturating_mul(u64::from(BAND_ROWS)) {
        0 => Err(JpegError::Corrupt),
        band => Ok(band),
    }
}

pub(crate) fn colored(strip: &mut [u8], coloring: &Coloring<'_>) -> Result<(), JpegError> {
    let Ok(line) = index(u64::from(coloring.wide).saturating_mul(RGBA));

    match line {
        0 => return Err(JpegError::Corrupt),
        _ => {},
    }

    for (down, pixels) in (coloring.first..).zip(strip.chunks_mut(line)) {
        let rows = rows_at(coloring, down)?;

        row(pixels, coloring.colors, &rows)?;
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
        _ => {},
    }

    let stored = turning.band.as_chunks::<4>().0;

    for (down, pixels) in (turning.first..).zip(strip.chunks_mut(line)) {
        for (across, pixel) in (left..).zip(pixels.as_chunks_mut::<4>().0.iter_mut().skip(skipped).take(taken)) {
            let Ok(from) = turning.orientation.source(Point { x: across, y: down }, turning.stored);

            let row = match from.y.checked_sub(turning.from) {
                Some(row) => row,
                None => return Err(JpegError::Corrupt),
            };

            let Ok(at) = index(u64::from(row).saturating_mul(u64::from(turning.stored.width)).saturating_add(u64::from(from.x)));

            match stored.get(at) {
                Some(source) => *pixel = *source,
                None => return Err(JpegError::Corrupt),
            }
        }
    }

    Ok(())
}

fn column_taps(layout: Layout, wide: u32) -> Result<Vec<Taps>, Never> {
    let Ok(size) = frame::plane(layout);

    Ok(match (layout.stretched.width, layout.stretched.height) {
        (0, 0) => Vec::new(),
        (_, _) => (0..wide)
            .map(|at| {
                let Ok(tapped) = taps(at, Stretch { shift: layout.stretched.width, samples: size.width });

                tapped
            })
            .collect::<Vec<Taps>>(),
    })
}

fn rows_at<'i>(coloring: &Coloring<'i>, down: u32) -> Result<Vec<Cow<'i, [u8]>>, JpegError> {
    let mut rows = Vec::new();

    for ((plane, layout), taps) in coloring.planes.iter().zip(coloring.layouts).zip(coloring.columns) {
        let line = line(plane, *layout, (taps, down))?;

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
    let Ok(toward) = whole_u32((middle - f64::from(near)) * f64::from(WHOLE_WEIGHT));

    Ok(Taps { near: near.min(last), far: near.saturating_add(1).min(last), toward })
}

fn plane_row<'p>(plane: &Window<'p>, size: Size<u32>, row: u32) -> Result<&'p [u8], JpegError> {
    let held = match row.checked_sub(plane.top) {
        Some(held) => held,
        None => return Err(JpegError::Corrupt),
    };

    let Ok(start) = index(u64::from(held).saturating_mul(u64::from(size.width)));
    let Ok(wide) = index(size.width);

    match plane.samples.get(start..start.saturating_add(wide)) {
        Some(samples) => Ok(samples),
        None => Err(JpegError::Corrupt),
    }
}

fn line<'p>(plane: &Window<'p>, layout: Layout, at: (&[Taps], u32)) -> Result<Cow<'p, [u8]>, JpegError> {
    let (columns, down) = at;
    let Ok(size) = frame::plane(layout);
    let Ok(rows) = taps(down, Stretch { shift: layout.stretched.height, samples: size.height });
    let near = plane_row(plane, size, rows.near)?;

    match (layout.stretched.width, layout.stretched.height) {
        (0, 0) => return Ok(Cow::Borrowed(near)),
        (_, _) => {},
    }

    let far = plane_row(plane, size, rows.far)?;

    let staying = WHOLE_WEIGHT.saturating_sub(rows.toward);

    let mixed: Vec<u32> = near
        .iter()
        .zip(far)
        .map(|(near, far)| u32::from(*near).saturating_mul(staying).saturating_add(u32::from(*far).saturating_mul(rows.toward)))
        .collect();

    let mut stretched = Vec::with_capacity(columns.len());
    let rounding = 1u32.wrapping_shl(WEIGHT_BITS.saturating_mul(2).saturating_sub(1));

    for tapped in columns {
        let Ok(near) = index(tapped.near);
        let Ok(far) = index(tapped.far);
        let kept = WHOLE_WEIGHT.saturating_sub(tapped.toward);

        let level = match (mixed.get(near), mixed.get(far)) {
            (Some(near), Some(far)) => near.saturating_mul(kept).saturating_add(far.saturating_mul(tapped.toward)),
            (None, _) | (_, None) => return Err(JpegError::Corrupt),
        };

        let Ok(level) = fitted::<u32, u8>(level.saturating_add(rounding).wrapping_shr(WEIGHT_BITS.saturating_mul(2)));

        stretched.push(level);
    }

    Ok(Cow::Owned(stretched))
}


fn row(pixels: &mut [u8], colors: Colors, rows: &[Cow<'_, [u8]>]) -> Result<(), JpegError> {
    match (colors, rows) {
        (Colors::Grey, [grey]) => written(pixels, grey.iter().map(|level| [*level; 3])),
        (Colors::YCbCr, [luma, blue, red]) => written(
            pixels,
            luma.iter().zip(blue.iter().zip(red.iter())).map(|(luma, (blue, red))| {
                let Ok(rgb) = converted([*luma, *blue, *red]);

                rgb
            }),
        ),
        (Colors::Rgb, [red, green, blue]) => written(pixels, red.iter().zip(green.iter().zip(blue.iter())).map(|(red, (green, blue))| [*red, *green, *blue])),
        (Colors::Grey | Colors::YCbCr | Colors::Rgb, _) => Err(JpegError::Corrupt),
    }
}

fn written(pixels: &mut [u8], colors: impl Iterator<Item = [u8; 3]>) -> Result<(), JpegError> {
    for (slot, [red, green, blue]) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(colors) {
        *slot = [red, green, blue, u8::MAX];
    }

    Ok(())
}

fn converted(sample: [u8; 3]) -> Result<[u8; 3], Never> {
    let [luma, blue, red] = sample.map(i32::from);
    let blue = blue.wrapping_sub(MIDDLE);
    let red = red.wrapping_sub(MIDDLE);
    let reddened = RED_FROM_RED.wrapping_mul(red).wrapping_add(HALF).wrapping_shr(SCALE_BITS);
    let greened = HALF.wrapping_sub(GREEN_FROM_BLUE.wrapping_mul(blue)).wrapping_sub(GREEN_FROM_RED.wrapping_mul(red)).wrapping_shr(SCALE_BITS);
    let blued = BLUE_FROM_BLUE.wrapping_mul(blue).wrapping_add(HALF).wrapping_shr(SCALE_BITS);
    let Ok(r) = fitted::<i32, u8>(luma.wrapping_add(reddened));
    let Ok(g) = fitted::<i32, u8>(luma.wrapping_add(greened));
    let Ok(b) = fitted::<i32, u8>(luma.wrapping_add(blued));

    Ok([r, g, b])
}

