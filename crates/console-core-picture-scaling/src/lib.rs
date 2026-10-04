//! A picture drawn again at the size it is wanted, from any part of it.
//!
//! A JPEG comes back from its decoder at the smallest of its own sizes that
//! covers what was asked, which is never more than twice that across, so what
//! is left is a short step down -- or, for a picture smaller than the room it
//! is shown in, a step up. A PNG comes back whole, so a screenshot made into a
//! thumbnail is a long step down. All of them are the one filter: each pixel
//! made is the pixels it lies over, weighted by how near their middles are to
//! its own, which is bilinear going up and an average over everything it covers
//! going down, so a picture made smaller never shimmers. It is done across and
//! then down, in whole numbers, the way Pillow does it.
//!
//! The part drawn from is a region, so a square cut from the middle of a
//! sleeve is the same call as the whole of a photograph.
//!
//! It is a crate of its own because the decoders are, and what reads a
//! picture for somebody else -- Orbit's thumbnails, which are a server's and
//! not this desktop's -- wants the step after the decoding without the
//! desktop's way of opening a file. It knows no format: it is handed RGBA.
//!
//! A lossy WebP arrives a row at a time, only the part that is wanted, and
//! [`Scaling`] draws each row across as it comes. What is kept is the rows
//! already drawn across, as wide as the picture being made, and they are
//! drawn down once the last has come, so the arithmetic is the same as for a
//! picture handed over whole.

use console_core_geometry::{Rectangle, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u32, whole_i32};

const WEIGHT_BITS: u32 = 14;

const WHOLE_WEIGHT: f64 = 16384.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source<'a> {
    pub rgba: &'a [u8],
    pub width: u32,
    pub region: Rectangle<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    from: u32,
    to: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Taps {
    first: u32,
    weights: Vec<i32>,
}

#[derive(Debug)]
pub struct Scaling {
    width: u32,
    region: Rectangle<u32>,
    made: Size<u32>,
    columns: Vec<Taps>,
    rows: Vec<Taps>,
    narrowed: Vec<[u8; 4]>,
    next: u32,
}

impl Scaling {
    pub fn new(width: u32, region: Rectangle<u32>, to: Size<u32>) -> Result<Scaling, Never> {
        let Ok(columns) = taps(Span { from: region.size.width, to: to.width });
        let Ok(rows) = taps(Span { from: region.size.height, to: to.height });

        let Ok(area) = index(u64::from(to.width).saturating_mul(u64::from(region.size.height)));

        Ok(Scaling { width, region, made: to, columns, rows, narrowed: vec![[0; 4]; area], next: 0 })
    }

    pub fn received(&mut self, rgba: &[u8]) -> Result<(), Never> {
        let Ok(width) = index(self.width);
        let Ok(left) = index(self.region.origin.x);
        let top = self.region.origin.y;
        let bottom = top.saturating_add(self.region.size.height);

        for line in rgba.as_chunks::<4>().0.chunks_exact(width.max(1)) {
            match (self.next >= top, self.next < bottom) {
                (true, true) => {
                    let part = match line.get(left..) {
                        Some(part) => part,
                        None => &[],
                    };

                    let Ok(made_width) = index(self.made.width);
                    let Ok(row) = index(self.next.saturating_sub(top));

                    match self.narrowed.chunks_exact_mut(made_width.max(1)).nth(row) {
                        Some(into) => {
                            let Ok(()) = horizontally(part, (&self.columns, into));
                        },
                        None => {},
                    }
                },
                (_, _) => {},
            }

            self.next = self.next.saturating_add(1);
        }

        Ok(())
    }

    pub fn finished(self) -> Result<Vec<u8>, Never> {
        let Ok(made_width) = index(self.made.width);

        match (self.width, made_width, self.made.height) {
            (0, _, _) | (_, 0, _) | (_, _, 0) => return Ok(Vec::new()),
            (_, _, _) => {},
        }

        let mut made = Vec::new();

        for row in &self.rows {
            let mut sums = vec![[0i32; 4]; made_width];
            let Ok(first) = index(row.first);

            for (line, weight) in self.narrowed.chunks(made_width).skip(first).zip(&row.weights) {
                for (sum, pixel) in sums.iter_mut().zip(line) {
                    let Ok(()) = added(sum, pixel, *weight);
                }
            }

            for sum in sums {
                let Ok(pixel) = descaled(sum);

                made.extend_from_slice(&pixel);
            }
        }

        Ok(made)
    }
}

pub fn scaled(source: Source<'_>, to: Size<u32>) -> Result<Vec<u8>, Never> {
    let Ok(mut scaling) = Scaling::new(source.width, source.region, to);
    let Ok(()) = scaling.received(source.rgba);

    scaling.finished()
}

fn horizontally(part: &[[u8; 4]], made: (&[Taps], &mut [[u8; 4]])) -> Result<(), Never> {
    let (columns, across) = made;

    for (column, into) in columns.iter().zip(across) {
        let mut sum = [0i32; 4];
        let Ok(first) = index(column.first);

        for (pixel, weight) in part.iter().skip(first).zip(&column.weights) {
            let Ok(()) = added(&mut sum, pixel, *weight);
        }

        let Ok(pixel) = descaled(sum);

        *into = pixel;
    }

    Ok(())
}

fn added(sum: &mut [i32; 4], pixel: &[u8; 4], weight: i32) -> Result<(), Never> {
    for (channel, value) in sum.iter_mut().zip(pixel) {
        *channel = channel.wrapping_add(weight.wrapping_mul(i32::from(*value)));
    }

    Ok(())
}

fn descaled(sum: [i32; 4]) -> Result<[u8; 4], Never> {
    Ok(sum.map(|channel| {
        let Ok(value) = fitted::<i32, u8>(channel.wrapping_add(1i32.wrapping_shl(WEIGHT_BITS.saturating_sub(1))).wrapping_shr(WEIGHT_BITS));

        value
    }))
}

fn taps(span: Span) -> Result<Vec<Taps>, Never> {
    let scale = f64::from(span.from) / f64::from(span.to.max(1));
    let reach = scale.max(1.0);

    Ok((0..span.to)
        .map(|at| {
            let middle = (f64::from(at) + 0.5) * scale;
            let Ok(first) = toward_zero_u32((middle - reach).floor().max(0.0));
            let Ok(last) = toward_zero_u32((middle + reach).ceil());
            let last = last.min(span.from);

            let near: Vec<f64> = (first..last)
                .map(|source| (1.0 - ((f64::from(source) + 0.5 - middle) / reach).abs()).max(0.0))
                .collect();
            let total: f64 = near.iter().sum();
            let total = match total > 0.0 {
                true => total,
                false => 1.0,
            };

            let weights = near
                .iter()
                .map(|weight| {
                    let Ok(weight) = whole_i32(weight / total * WHOLE_WEIGHT);

                    weight
                })
                .collect();
            Taps { first, weights }
        })
        .collect())
}
