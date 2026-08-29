//! The picture measured while it is still pixels.
//!
//! A check that wants to know whether what is on the screen is this wallpaper
//! has to compare against something, and it cannot decode the file: an
//! animated WebP is a VP8 bitstream and nothing on the device will take one
//! apart. So the drawing is measured here.

use cairo::ImageSurface;
use indexmap::IndexMap;

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
    pub fn of(surface: &mut ImageSurface) -> Self {
        let (stride, width, height) = (
            surface.stride() as usize,
            surface.width() as usize,
            surface.height() as usize,
        );
        surface.flush();
        let data = surface.data().expect("the picture is drawn").to_vec();
        Pixels {
            data,
            stride,
            width,
            height,
        }
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
    let half_x = (pixels.width as f64 * patch / 2.0) as usize;
    let half_y = (pixels.height as f64 * patch / 2.0) as usize;
    let x0 = (pixels.width as f64 * across) as usize;
    let y0 = (pixels.height as f64 * down) as usize;
    let (total, seen) = (y0.saturating_sub(half_y)..(y0 + half_y).min(pixels.height))
        .flat_map(|y| {
            (x0.saturating_sub(half_x)..(x0 + half_x).min(pixels.width)).map(move |x| (x, y))
        })
        .fold(([0usize; 3], 0usize), |(total, seen), (x, y)| {
            let colour = pixels.at(x, y);
            (
                [
                    total[0] + colour[0] as usize,
                    total[1] + colour[1] as usize,
                    total[2] + colour[2] as usize,
                ],
                seen + 1,
            )
        });
    let average = |channel: usize| (channel as f64 / seen as f64).round() as u8;
    hexcode([average(total[0]), average(total[1]), average(total[2])])
}

/// How far two colours are apart, by their furthest channel.
fn apart(one: &str, other: &str) -> i32 {
    let channel = |code: &str, at: usize| i32::from_str_radix(&code[at..at + 2], 16).unwrap_or(0);
    (0..3)
        .map(|which| (channel(one, which * 2) - channel(other, which * 2)).abs())
        .max()
        .unwrap_or(0)
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
pub fn blind(probes: &[((f64, f64), String)], fallback: &str) -> Vec<Blind> {
    probes
        .iter()
        .map(|((across, down), colour)| Blind {
            across: *across,
            down: *down,
            colour: colour.clone(),
            apart: apart(colour, fallback),
        })
        .filter(|found| found.apart < CLEAR_OF_NOTHING)
        .collect()
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
        assert_eq!(dark.len(), 1);
        assert_eq!((dark[0].across, dark[0].apart), (0.1, 2));
    }
}
