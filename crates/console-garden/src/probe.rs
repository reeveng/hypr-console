//! The picture measured while it is still pixels.
//!
//! A check that wants to know whether what is on the screen is this wallpaper
//! has to compare against something, and it cannot decode the file: an
//! animated WebP is a VP8 bitstream and nothing on the device will take one
//! apart. So the drawing is measured here.

use cairo::ImageSurface;
use indexmap::IndexMap;

use console_number::{Float, toward_zero_usize, whole_u8};

use crate::fault::{Drawing, Fault};

/// One of cairo's dimensions, as a count of pixels.
///
/// Cairo says a size in `i32` and has never said a negative one, so this does
/// not fail in practice. It is written as a conversion that can rather than as
/// a cast that cannot, because the first says what happens if cairo is ever
/// wrong and the second only says that nobody thought about it.
fn measured(size: i32) -> Drawing<usize> {
    usize::try_from(size).map_err(|_| Fault::Sized(size))
}

/// Places to look, as fractions of the picture.
///
/// Chosen to be flat: no tree, no petal, no crest, so what is read there is
/// the ground or the air and a difference means the wrong picture rather than
/// the wrong pixel. They are spread apart on purpose, so that a picture
/// painted at the wrong size, the wrong shape or the wrong way up fails at
/// least one of them.
pub const PROBES: [(f64, f64); 5] = [
    (0.06, 0.46),
    (0.95, 0.49),
    (0.88, 0.62),
    (0.52, 0.86),
    (0.24, 0.63),
];

/// How far a probe has to sit from the colour the screen is when nothing has
/// painted it. A probe that lands on that colour is a probe that passes
/// against a dead wallpaper daemon, which is the one thing this is for.
pub const CLEAR_OF_NOTHING: i32 = 14;

/// A drawn picture, as the bytes behind it.
///
/// Taken off the surface once so that everything measuring the picture is a
/// function of numbers rather than of a live brush.
pub struct Pixels {
    pub data: Vec<u8>,
    pub stride: usize,
    pub width: usize,
    pub height: usize,
}

impl Pixels {
    /// The bytes of a drawn picture.
    ///
    /// Borrowing them fails while a brush is still holding the surface, which
    /// is a caller that has not dropped its `Context` yet. That is a mistake in
    /// the calling code rather than anything about the picture, and it says so.
    pub fn of(surface: &mut ImageSurface) -> Drawing<Self> {
        let (stride, width, height) = (
            measured(surface.stride())?,
            measured(surface.width())?,
            measured(surface.height())?,
        );
        surface.flush();
        let data = surface.data()?.to_vec();
        Ok(Pixels {
            data,
            stride,
            width,
            height,
        })
    }

    /// One pixel, as red, green and blue. The bytes are laid down the other
    /// way round, because the machine is.
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let at = y * self.stride + x * 4;
        [self.data[at + 2], self.data[at + 1], self.data[at]]
    }
}

fn hexcode([red, green, blue]: [u8; 3]) -> String {
    format!("{red:02x}{green:02x}{blue:02x}")
}

/// The colour most of the resting picture is.
pub fn commonest(pixels: &Pixels) -> String {
    let seen = (0..pixels.data.len().saturating_sub(4))
        .step_by(4 * 37)
        .map(|at| [pixels.data[at + 2], pixels.data[at + 1], pixels.data[at]])
        .fold(IndexMap::<[u8; 3], usize>::new(), |mut tally, colour| {
            *tally.entry(colour).or_default() += 1;
            tally
        });
    let most = seen
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(colour, _)| *colour)
        .unwrap_or([0, 0, 0]);
    hexcode(most)
}

/// The average colour of a small patch of the picture.
///
/// An average and not a pixel, so that a petal or a blade of grass that
/// strayed into the patch moves the answer by less than a lossy encoder does.
pub fn probe(pixels: &Pixels, across: f64, down: f64, patch: f64) -> String {
    let half_x = toward_zero_usize(pixels.width.float() * patch / 2.0);
    let half_y = toward_zero_usize(pixels.height.float() * patch / 2.0);
    let x0 = toward_zero_usize(pixels.width.float() * across);
    let y0 = toward_zero_usize(pixels.height.float() * down);
    let (total, seen) = (y0.saturating_sub(half_y)..(y0 + half_y).min(pixels.height))
        .flat_map(|y| {
            (x0.saturating_sub(half_x)..(x0 + half_x).min(pixels.width)).map(move |x| (x, y))
        })
        .fold(([0usize; 3], 0usize), |(total, seen), (x, y)| {
            let colour = pixels.at(x, y);
            (
                [
                    total[0] + usize::from(colour[0]),
                    total[1] + usize::from(colour[1]),
                    total[2] + usize::from(colour[2]),
                ],
                seen + 1,
            )
        });
    let average = |channel: usize| whole_u8(channel.float() / seen.float());
    hexcode([average(total[0]), average(total[1]), average(total[2])])
}

/// How far two colours are apart, by their furthest channel.
///
/// Fallible because one of the two comes from the palette rather than from a
/// picture, and a palette that says something which is not a colour should
/// stop the drawing rather than be read as black and quietly pass.
fn apart(one: &str, other: &str) -> Result<i32, String> {
    let channel = |code: &str, at: usize| -> Result<i32, String> {
        let pair = code
            .get(at..at + 2)
            .ok_or_else(|| format!("{code} is not six hex digits"))?;

        i32::from_str_radix(pair, 16).map_err(|_| format!("{code} is not six hex digits"))
    };

    let channels = (0..3)
        .map(|which| Ok((channel(one, which * 2)? - channel(other, which * 2)?).abs()))
        .collect::<Result<Vec<i32>, String>>()?;

    Ok(channels.into_iter().max().unwrap_or(0))
}

/// A probe that could not tell this picture from an unpainted screen.
pub struct Blind {
    pub across: f64,
    pub down: f64,
    pub colour: String,
    pub apart: i32,
}

/// Probes that could not tell this picture from an unpainted screen.
///
/// The compositor's own background is deliberately the picture's darkest
/// colour, so that a wallpaper daemon dying costs the right colour rather than
/// a grey nobody chose. That kindness is also a blindness, and it is a
/// blindness a check cannot see for itself: a probe on the dark part of the
/// sky reads exactly what a bare screen reads. So it is caught here, where
/// moving the composition is what would cause it.
pub fn blind(probes: &[((f64, f64), String)], fallback: &str) -> Result<Vec<Blind>, String> {
    // Written out rather than folded into the iterator chain: the `?` belongs
    // to `apart`, and a closure that can fail cannot hand it to the `filter`
    // that comes after it.
    let mut found = Vec::new();

    for ((across, down), colour) in probes {
        let apart = apart(colour, fallback)?;

        if apart < CLEAR_OF_NOTHING {
            found.push(Blind {
                across: *across,
                down: *down,
                colour: colour.clone(),
                apart,
            });
        }
    }

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(colour: [u8; 3], width: usize, height: usize) -> Pixels {
        let stride = width * 4;
        let data = (0..width * height)
            .flat_map(|_| [colour[2], colour[1], colour[0], 255])
            .collect();
        Pixels {
            data,
            stride,
            width,
            height,
        }
    }

    #[test]
    fn a_picture_of_one_colour_is_commonest_that_colour() {
        assert_eq!(commonest(&flat([0x12, 0x34, 0x56], 64, 64)), "123456");
    }

    #[test]
    fn a_probe_reads_the_colour_it_lands_on() {
        assert_eq!(
            probe(&flat([0x12, 0x34, 0x56], 64, 64), 0.5, 0.5, 0.2),
            "123456"
        );
    }

    #[test]
    fn a_probe_at_the_edge_reads_what_is_there_rather_than_running_off() {
        assert_eq!(
            probe(&flat([0xff, 0x00, 0x00], 64, 64), 0.99, 0.01, 0.2),
            "ff0000"
        );
    }

    #[test]
    fn a_probe_the_colour_of_a_bare_screen_is_blind() {
        let probes = [
            ((0.1, 0.2), "101010".to_string()),
            ((0.3, 0.4), "ffffff".to_string()),
        ];
        let dark = blind(&probes, "121212");
        let dark = dark.expect("colours that read");
        assert_eq!(dark.len(), 1);
        assert_eq!((dark[0].across, dark[0].apart), (0.1, 2));
    }
}
