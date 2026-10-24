//! A decoded picture, and the red, green and blue it is drawn as.
//!
//! A decoder hands back luma at the size of the picture and two planes of
//! chroma at that size or smaller -- half across and half down for nearly
//! every film -- in eight bits or in ten or twelve. Each pixel is its luma and
//! the chroma sample that covers it, with no blending between chroma samples:
//! what is drawn from this is nearly always made smaller afterwards, and the
//! shrinking averages more neighbours than a blend would have.
//!
//! The weights are the ones the film says it was made with. BT.709 is high
//! definition, BT.601 is what came before it and what a film that says
//! nothing is taken to be below 720 rows, and BT.2020 is what a phone writes
//! in HDR; a film whose samples are already red, green and blue says so, and
//! is read as it stands. Studio range puts black at 16 and white at 235, and a
//! film that says it is full range is read from 0 to 255. Deeper samples are
//! brought down to eight bits before any of it, which is all a screen here
//! shows. Each weight is a whole number with fourteen bits below the point,
//! worked out once a picture from the two numbers that define the matrix.
//!
//! A phone held upright records a film lying down and says how to turn it, and
//! the pixels come back turned, so what callers measure is what is seen.
//!
//! A picture wanted smaller than it was written -- every frame of a film
//! played on a card -- is not coloured whole and then shrunk. [`Picture::rgba_at`]
//! shrinks each plane to the size it is shown at, chroma straight from its own
//! size, and colours only those pixels; colouring a frame whole and shrinking
//! it took longer than a frame lasts. The three planes are drawn on cores of
//! their own, counted once by whoever draws many pictures.

use std::borrow::Cow;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, whole_i32};
use console_concurrency::Cores;
use console_core_picture_scaling::{Plane, scaled_plane};

use crate::tracks::{Turn, Video};

const FRACTION: u32 = 14;

const ONE: f64 = 16384.0;

const HALF: i32 = 8192;

const MIDDLE: i32 = 128;

const STUDIO_BLACK: i32 = 16;

const NEUTRAL: u8 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Matrix {
    Bt601,
    Bt709,
    Bt2020,
    Rgb,
    Unsaid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Range {
    Studio,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Colour {
    pub(crate) matrix: Matrix,
    pub(crate) range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Planes {
    Eight([Vec<u8>; 3]),
    Deep([Vec<u16>; 3], u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Strides {
    pub(crate) luma: u32,
    pub(crate) chroma: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    at: f64,
    stored: Size<u32>,
    chroma: Size<u32>,
    planes: Planes,
    strides: Strides,
    colour: Colour,
    turn: Turn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub size: Size<u32>,
    pub rgba: Vec<u8>,
}

pub(crate) struct Decoded {
    pub(crate) pts: i64,
    pub(crate) stored: Size<u32>,
    pub(crate) chroma: Size<u32>,
    pub(crate) planes: Planes,
    pub(crate) strides: Strides,
    pub(crate) colour: Colour,
}

impl Decoded {
    pub(crate) fn placed(self, video: &Video) -> Result<Picture, Never> {
        let Ok(at) = video.seconds(self.pts);

        Ok(Picture { at, stored: self.stored, chroma: self.chroma, planes: self.planes, strides: self.strides, colour: self.colour, turn: video.turn })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Weights {
    luma: i32,
    red_from_red: i32,
    green_from_blue: i32,
    green_from_red: i32,
    blue_from_blue: i32,
    black: i32,
}

fn weights(colour: Colour, height: u32) -> Result<Weights, Never> {
    let (red, blue) = match (colour.matrix, height >= 720) {
        (Matrix::Bt709, _) | (Matrix::Unsaid, true) => (0.2126, 0.0722),
        (Matrix::Bt601, _) | (Matrix::Unsaid, false) | (Matrix::Rgb, _) => (0.299, 0.114),
        (Matrix::Bt2020, _) => (0.2627, 0.0593),
    };

    let green = 1.0 - red - blue;

    let (luma_gain, chroma_gain, black) = match colour.range {
        Range::Studio => (255.0 / 219.0, 255.0 / 224.0, STUDIO_BLACK),
        Range::Full => (1.0, 1.0, 0),
    };

    let Ok(luma) = whole_i32(luma_gain * ONE);
    let Ok(red_from_red) = whole_i32(chroma_gain * 2.0 * (1.0 - red) * ONE);
    let Ok(blue_from_blue) = whole_i32(chroma_gain * 2.0 * (1.0 - blue) * ONE);
    let Ok(green_from_blue) = whole_i32(chroma_gain * 2.0 * blue * (1.0 - blue) / green * ONE);
    let Ok(green_from_red) = whole_i32(chroma_gain * 2.0 * red * (1.0 - red) / green * ONE);

    Ok(Weights { luma, red_from_red, green_from_blue, green_from_red, blue_from_blue, black })
}

#[inline(always)]
fn channel(value: i32) -> Result<u8, Never> {
    let [low, ..] = value.saturating_add(HALF).wrapping_shr(FRACTION).clamp(0, 255).to_le_bytes();

    Ok(low)
}

#[inline(always)]
fn pixel(weights: &Weights, sample: [u8; 3]) -> Result<[u8; 4], Never> {
    let [luma, blue, red] = sample;
    let bright = i32::from(luma).wrapping_sub(weights.black).wrapping_mul(weights.luma);
    let blue = i32::from(blue).wrapping_sub(MIDDLE);
    let red = i32::from(red).wrapping_sub(MIDDLE);
    let Ok(r) = channel(bright.wrapping_add(red.wrapping_mul(weights.red_from_red)));
    let Ok(g) = channel(bright.wrapping_sub(blue.wrapping_mul(weights.green_from_blue)).wrapping_sub(red.wrapping_mul(weights.green_from_red)));
    let Ok(b) = channel(bright.wrapping_add(blue.wrapping_mul(weights.blue_from_blue)));

    Ok([r, g, b, u8::MAX])
}

#[inline(always)]
fn as_it_stands(sample: [u8; 3]) -> Result<[u8; 4], Never> {
    let [green, blue, red] = sample;

    Ok([red, green, blue, u8::MAX])
}

trait Sample: Copy {
    fn eight(self, depth: u32) -> u8;
}

impl Sample for u8 {
    #[inline(always)]
    fn eight(self, _depth: u32) -> u8 {
        self
    }
}

impl Sample for u16 {
    #[inline(always)]
    fn eight(self, depth: u32) -> u8 {
        let [low, ..] = self.wrapping_shr(depth.saturating_sub(8)).min(255).to_le_bytes();

        low
    }
}

struct Layout {
    stored: Size<u32>,
    chroma: Size<u32>,
    strides: Strides,
    depth: u32,
}

fn scaled_down(at: u32, sides: (u32, u32)) -> Result<u32, Never> {
    let (chroma, whole) = sides;

    let scaled = match u64::from(at).saturating_mul(u64::from(chroma)).checked_div(u64::from(whole)) {
        Some(scaled) => scaled,
        None => 0,
    };

    fitted::<u64, u32>(scaled)
}

fn row_of<T: Sample>(plane: &[T], at: (u32, u32, u32)) -> Result<&[T], Never> {
    let (row, stride, width) = at;
    let Ok(start) = index(row.saturating_mul(stride));
    let Ok(wide) = index(width);

    Ok(match plane.get(start..).and_then(|samples| samples.get(..wide)) {
        Some(samples) => samples,
        None => &[],
    })
}

fn drawn<T: Sample>(planes: &[Vec<T>; 3], layout: &Layout, colour: Colour) -> Result<Vec<u8>, Never> {
    let Layout { stored, chroma, strides, depth } = *layout;
    let Ok(weights) = weights(colour, stored.height);
    let Ok(area) = index(u64::from(stored.width).saturating_mul(u64::from(stored.height)).saturating_mul(4));
    let Ok(line) = index(stored.width.saturating_mul(4));
    let mut rgba = vec![0u8; area];

    let across: Vec<u32> = (0..stored.width).map(|x| {
        let Ok(at) = scaled_down(x, (chroma.width, stored.width));

        at
    }).collect();

    let [luma, blue, red] = planes;

    for (out, y) in rgba.chunks_exact_mut(line.max(1)).zip(0..stored.height) {
        let Ok(luma_row) = row_of(luma, (y, strides.luma, stored.width));
        let Ok(down) = scaled_down(y, (chroma.height, stored.height));
        let Ok(blue_row) = row_of(blue, (down, strides.chroma, chroma.width));
        let Ok(red_row) = row_of(red, (down, strides.chroma, chroma.width));

        for ((into, luma), x) in out.as_chunks_mut::<4>().0.iter_mut().zip(luma_row).zip(&across) {
            let Ok(at) = index(*x);

            let (blue, red) = match (blue_row.get(at), red_row.get(at)) {
                (Some(blue), Some(red)) => (blue.eight(depth), red.eight(depth)),
                (_, _) => (NEUTRAL, NEUTRAL),
            };

            let sample = [luma.eight(depth), blue, red];

            let Ok(coloured) = match colour.matrix {
                Matrix::Rgb => as_it_stands(sample),
                Matrix::Bt601 | Matrix::Bt709 | Matrix::Bt2020 | Matrix::Unsaid => pixel(&weights, sample),
            };

            *into = coloured;
        }
    }

    Ok(rgba)
}

fn turned(rgba: &[u8], stored: Size<u32>, turn: Turn) -> Result<Vec<u8>, Never> {
    let pixels = rgba.as_chunks::<4>().0;
    let Size { width, height } = stored;

    let at = |x: u32, y: u32| -> [u8; 4] {
        let Ok(position) = index(u64::from(y).saturating_mul(u64::from(width)).saturating_add(u64::from(x)));

        match pixels.get(position) {
            Some(pixel) => *pixel,
            None => [0, 0, 0, u8::MAX],
        }
    };

    let last_across = width.saturating_sub(1);
    let last_down = height.saturating_sub(1);

    let out: Vec<[u8; 4]> = match turn {
        Turn::Upright => pixels.to_vec(),
        Turn::Half => pixels.iter().rev().copied().collect(),
        Turn::Quarter => (0..width).flat_map(|row| (0..height).map(move |column| (column, row))).map(|(column, row)| at(row, last_down.saturating_sub(column))).collect(),
        Turn::ThreeQuarters => (0..width).flat_map(|row| (0..height).map(move |column| (column, row))).map(|(column, row)| at(last_across.saturating_sub(row), column)).collect(),
    };

    Ok(out.into_flattened())
}

impl Picture {
    pub fn at(&self) -> Result<f64, Never> {
        Ok(self.at)
    }

    pub fn size(&self) -> Result<Size<u32>, Never> {
        Ok(match self.turn {
            Turn::Upright | Turn::Half => self.stored,
            Turn::Quarter | Turn::ThreeQuarters => Size { width: self.stored.height, height: self.stored.width },
        })
    }

    pub fn rgba(&self) -> Result<Bitmap, Never> {
        let depth = match &self.planes {
            Planes::Eight(_) => 8,
            Planes::Deep(_, depth) => *depth,
        };

        let layout = Layout { stored: self.stored, chroma: self.chroma, strides: self.strides, depth };

        let Ok(rgba) = match &self.planes {
            Planes::Eight(planes) => drawn(planes, &layout, self.colour),
            Planes::Deep(planes, _) => drawn(planes, &layout, self.colour),
        };

        let rgba = match self.turn {
            Turn::Upright => rgba,
            Turn::Half | Turn::Quarter | Turn::ThreeQuarters => {
                let Ok(turned) = turned(&rgba, self.stored, self.turn);

                turned
            },
        };

        let Ok(size) = self.size();

        Ok(Bitmap { size, rgba })
    }
}

fn eight_bit<T: Sample>(plane: &[T], depth: u32) -> Result<Vec<u8>, Never> {
    Ok(plane.iter().map(|sample| sample.eight(depth)).collect())
}

fn coloured_at(planes: [&[u8]; 3], made: Size<u32>, colour: Colour, tall: u32) -> Result<Vec<u8>, Never> {
    let [luma, blue, red] = planes;
    let Ok(weights) = weights(colour, tall);
    let Ok(area) = index(u64::from(made.width).saturating_mul(u64::from(made.height)));

    let (blue, red): (Cow<'_, [u8]>, Cow<'_, [u8]>) = match (blue.len() == area, red.len() == area) {
        (true, true) => (Cow::Borrowed(blue), Cow::Borrowed(red)),
        (_, _) => (Cow::Owned(vec![NEUTRAL; area]), Cow::Owned(vec![NEUTRAL; area])),
    };

    let mut rgba = [0, 0, 0, u8::MAX].repeat(area);
    let pixels = rgba.as_chunks_mut::<4>().0.iter_mut();
    let samples = luma.iter().zip(blue.iter()).zip(red.iter());

    match colour.matrix {
        Matrix::Rgb => {
            for (into, ((luma, blue), red)) in pixels.zip(samples) {
                let Ok(coloured) = as_it_stands([*luma, *blue, *red]);

                *into = coloured;
            }
        },
        Matrix::Bt601 | Matrix::Bt709 | Matrix::Bt2020 | Matrix::Unsaid => {
            for (into, ((luma, blue), red)) in pixels.zip(samples) {
                let Ok(coloured) = pixel(&weights, [*luma, *blue, *red]);

                *into = coloured;
            }
        },
    }

    Ok(rgba)
}

impl Picture {
    pub fn rgba_at(&self, shown: Size<u32>, cores: Cores) -> Result<Bitmap, Never> {
        let made = match self.turn {
            Turn::Upright | Turn::Half => shown,
            Turn::Quarter | Turn::ThreeQuarters => Size { width: shown.height, height: shown.width },
        };

        let [luma, blue, red]: [Cow<'_, [u8]>; 3] = match &self.planes {
            Planes::Eight([luma, blue, red]) => [Cow::Borrowed(luma.as_slice()), Cow::Borrowed(blue.as_slice()), Cow::Borrowed(red.as_slice())],
            Planes::Deep(planes, depth) => planes.each_ref().map(|plane| {
                let Ok(eight) = eight_bit(plane, *depth);

                Cow::Owned(eight)
            }),
        };

        let planes = [
            Plane { samples: &luma, stride: self.strides.luma, size: self.stored },
            Plane { samples: &blue, stride: self.strides.chroma, size: self.chroma },
            Plane { samples: &red, stride: self.strides.chroma, size: self.chroma },
        ];

        let scaled = console_concurrency::map(cores, &planes, |plane| {
            let Ok(scaled) = scaled_plane(*plane, made);

            scaled
        });

        let [luma, blue, red] = match scaled.map(<[Vec<u8>; 3]>::try_from) {
            Ok(Ok(three)) => three,
            Ok(Err(_not_three)) => [Vec::new(), Vec::new(), Vec::new()],
            Err(_a_plane_panicked) => [Vec::new(), Vec::new(), Vec::new()],
        };

        let Ok(rgba) = coloured_at([&luma, &blue, &red], made, self.colour, self.stored.height);

        let rgba = match self.turn {
            Turn::Upright => rgba,
            Turn::Half | Turn::Quarter | Turn::ThreeQuarters => {
                let Ok(turned) = turned(&rgba, made, self.turn);

                turned
            },
        };

        Ok(Bitmap { size: shown, rgba })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(colour: Colour, luma: u8) -> Result<[u8; 4], Never> {
        let Ok(weights) = weights(colour, 1080);

        pixel(&weights, [luma, 128, 128])
    }

    #[test]
    fn black_and_white_at_studio_range_are_black_and_white() {
        let studio = Colour { matrix: Matrix::Bt709, range: Range::Studio };

        assert_eq!(grey(studio, 16), Ok([0, 0, 0, 255]));
        assert_eq!(grey(studio, 235), Ok([255, 255, 255, 255]));
    }

    #[test]
    fn full_range_is_read_as_it_is_written() {
        let full = Colour { matrix: Matrix::Bt601, range: Range::Full };

        assert_eq!(grey(full, 0), Ok([0, 0, 0, 255]));
        assert_eq!(grey(full, 100), Ok([100, 100, 100, 255]));
    }

    #[test]
    fn pure_red_comes_back_red_under_each_matrix() {
        let cases = [(Matrix::Bt601, [81, 90, 240]), (Matrix::Bt709, [63, 102, 240]), (Matrix::Bt2020, [74, 97, 240])];

        for (matrix, sample) in cases {
            let Ok(weights) = weights(Colour { matrix, range: Range::Studio }, 1080);
            let Ok([red, green, blue, _]) = pixel(&weights, sample);

            assert!(red >= 250 && green <= 5 && blue <= 5, "{matrix:?} drew {red}, {green}, {blue}");
        }
    }

    #[test]
    fn a_quarter_turn_puts_the_bottom_left_at_the_top_left() -> Result<(), Never> {
        let rgba: Vec<u8> = [[1, 0, 0, 255], [2, 0, 0, 255], [3, 0, 0, 255], [4, 0, 0, 255], [5, 0, 0, 255], [6, 0, 0, 255]].concat();
        let stored = Size { width: 3, height: 2 };
        let Ok(quarter) = turned(&rgba, stored, Turn::Quarter);
        let Ok(three) = turned(&rgba, stored, Turn::ThreeQuarters);
        let reds = |rgba: &[u8]| rgba.as_chunks::<4>().0.iter().map(|[red, ..]| *red).collect::<Vec<u8>>();

        assert_eq!(reds(&quarter), vec![4, 1, 5, 2, 6, 3]);
        assert_eq!(reds(&three), vec![3, 6, 2, 5, 1, 4]);

        Ok(())
    }
}
