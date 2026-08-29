//! Colours, and how far apart two of them are.
//!
//! Every colour on this desktop is declared as a hue and how much of it, and
//! the lightness is worked out here rather than chosen: a colour is told what
//! it has to be readable against and comes back as the softest shade that
//! clears it. That is the whole reason this file exists. Picking pastels by
//! eye and then measuring them afterwards gets a palette that passed once;
//! asking for the palest colour that still clears 7:1 gets one that goes on
//! clearing it when the ground behind it changes.
//!
//! Oklch in, sRGB out. Lightness in oklch is close to lightness as an eye
//! reads it, so a binary search on it converges on the answer from either side
//! and the hue does not drift while it happens.

use std::fmt;

/// A pairing that does not clear what it was asked to clear.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Short(pub String);

impl fmt::Display for Short {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl std::error::Error for Short {}

/// Oklab, from Björn Ottosson's derivation. The two matrices are the transform
/// through the cone responses and back; the cube and cube root either side of
/// them are what make the space perceptual.
const TO_LMS: [[f64; 3]; 3] = [
    [0.4122214708, 0.5363325363, 0.0514459929],
    [0.2119034982, 0.6806995451, 0.1073969566],
    [0.0883024619, 0.2817188376, 0.6299787005],
];

const FROM_LMS: [[f64; 3]; 3] = [
    [4.0767416621, -3.3077115913, 0.2309699292],
    [-1.2684380046, 2.6097574011, -0.3413193965],
    [-0.0041960863, -0.7034186147, 1.7076147010],
];

/// The two columns that turn chroma and hue back into cone responses. The
/// first column is lightness itself and is added unweighted, so it is not
/// written here.
const FROM_LCH: [[f64; 2]; 3] = [
    [0.3963377774, 0.2158037573],
    [-0.1055613458, -0.0638541728],
    [-0.0894841775, -1.2914855480],
];

/// Half to even, which is the rounding a hex digit is quantised with.
///
/// Named because the obvious rounding is the other one. Half away from zero
/// moves a channel that lands exactly between two values in a direction that
/// depends on nothing, and two colours a blend apart then differ by a bit for
/// no reason anybody can point at.
fn round_half_even(value: f64) -> f64 {
    let floor = value.floor();
    let rest = value - floor;
    if rest > 0.5 {
        floor + 1.0
    } else if rest < 0.5 {
        floor
    } else if (floor / 2.0).fract() == 0.0 {
        floor
    } else {
        floor + 1.0
    }
}

/// A linear channel as sRGB writes it.
fn to_srgb(channel: f64) -> f64 {
    if channel <= 0.0031308 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// An sRGB channel as light.
pub fn to_linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// (r, g, b) as floats, which are outside 0..1 when the colour is not real.
pub fn oklch_to_rgb(lightness: f64, chroma: f64, hue: f64) -> [f64; 3] {
    let radians = hue.to_radians();
    let (a, b) = (chroma * radians.cos(), chroma * radians.sin());
    let cubed: Vec<f64> = FROM_LCH
        .iter()
        // `powf`, not `powi`. Cubing by multiplying rounds twice and lands a
        // bit or two away from what a `pow` does, and the binary searches
        // below run for 48 halvings, which is long enough for that bit to
        // become the answer's last three digits.
        .map(|row| (lightness + row[0] * a + row[1] * b).powf(3.0))
        .collect();
    let mut rgb = [0.0; 3];
    for (channel, row) in rgb.iter_mut().zip(FROM_LMS) {
        *channel = to_srgb((0..3).map(|i| row[i] * cubed[i]).sum());
    }
    rgb
}

/// Whether a screen can actually show it.
pub fn in_gamut(lightness: f64, chroma: f64, hue: f64) -> bool {
    oklch_to_rgb(lightness, chroma, hue)
        .iter()
        .all(|channel| (-0.0001..=1.0001).contains(channel))
}

/// The same colour with just enough chroma taken out of it to be real.
///
/// Lightness is what the contrast was worked out from, so it is held and the
/// saturation gives way. A pastel that has lost a little chroma is still the
/// colour it was meant to be; one that has lost lightness is a different one.
pub fn fit(lightness: f64, chroma: f64, hue: f64) -> f64 {
    if in_gamut(lightness, chroma, hue) {
        return chroma;
    }
    let (mut low, mut high) = (0.0, chroma);
    for _ in 0..40 {
        let middle = (low + high) / 2.0;
        if in_gamut(lightness, middle, hue) {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

/// A colour as six hex digits, fitted into the gamut on the way.
pub fn hexcode(lightness: f64, chroma: f64, hue: f64) -> String {
    oklch_to_rgb(lightness, fit(lightness, chroma, hue), hue)
        .iter()
        .map(|channel| {
            format!("{:02x}", round_half_even(channel.clamp(0.0, 1.0) * 255.0) as u32)
        })
        .collect()
}

/// The three channels of six hex digits, as the bytes they are written as.
fn bytes(code: &str) -> [u8; 3] {
    let code = code.trim_start_matches('#').as_bytes();
    let mut out = [0u8; 3];
    for (channel, i) in out.iter_mut().zip([0, 2, 4]) {
        let pair = std::str::from_utf8(&code[i..i + 2]).unwrap_or("00");
        *channel = u8::from_str_radix(pair, 16).unwrap_or(0);
    }
    out
}

/// The same three, as floats from zero to one.
fn channels(code: &str) -> [f64; 3] {
    bytes(code).map(|channel| channel as f64 / 255.0)
}

/// Relative luminance, as WCAG defines it, from six hex digits.
pub fn luminance(code: &str) -> f64 {
    let [red, green, blue] = channels(code).map(to_linear);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/// How far apart two colours are, from 1:1 to 21:1.
pub fn contrast(one: &str, other: &str) -> f64 {
    let (first, second) = (luminance(one), luminance(other));
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

/// `top` laid on `bottom` at `alpha`, as the screen would blend them.
///
/// Anything painted with transparency is a colour in its own right once it is
/// on screen, and it is that colour the text on top of it has to clear.
pub fn over(top: &str, bottom: &str, alpha: f64) -> String {
    let (top, bottom) = (bytes(top), bytes(bottom));
    top.iter()
        .zip(bottom)
        .map(|(front, back)| {
            let mixed = *front as f64 * alpha + back as f64 * (1.0 - alpha);
            format!("{:02x}", round_half_even(mixed) as u32)
        })
        .collect()
}

/// The darkest lightness at which a hue clears `ratio` against every ground.
///
/// Darkest, because a pastel that is lighter than it needs to be is a pastel
/// on its way to white, and ten of those are one colour. Contrast against a
/// dark ground climbs with lightness and never falls, so the answer is found
/// by halving.
pub fn lightest_clearing(
    chroma: f64,
    hue: f64,
    grounds: &[String],
    ratio: f64,
    floor: f64,
) -> Result<f64, Short> {
    if grounds.is_empty() {
        return Ok(floor);
    }
    let clears = |lightness: f64| {
        let code = hexcode(lightness, chroma, hue);
        grounds.iter().all(|ground| contrast(&code, ground) >= ratio)
    };

    if clears(floor) {
        return Ok(floor);
    }
    let (mut low, mut high) = (floor, 1.0);
    if !clears(high) {
        return Err(Short(format!(
            "nothing at hue {hue} clears {ratio}:1 against {grounds:?}"
        )));
    }
    for _ in 0..48 {
        let middle = (low + high) / 2.0;
        if clears(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    Ok(high)
}

/// The lightest lightness at which a hue clears `ratio` under every ceiling.
///
/// The mirror of the one above, for ink that is painted on top of a fill: the
/// fill is already decided and the ink has to be dark enough against it.
pub fn darkest_clearing(
    chroma: f64,
    hue: f64,
    ceilings: &[String],
    ratio: f64,
) -> Result<f64, Short> {
    let clears = |lightness: f64| {
        let code = hexcode(lightness, chroma, hue);
        ceilings
            .iter()
            .all(|ceiling| contrast(&code, ceiling) >= ratio)
    };

    if ceilings.is_empty() || clears(1.0) {
        return Ok(1.0);
    }
    let (mut low, mut high) = (0.0, 1.0);
    if !clears(low) {
        return Err(Short(format!(
            "nothing at hue {hue} clears {ratio}:1 under {ceilings:?}"
        )));
    }
    for _ in 0..48 {
        let middle = (low + high) / 2.0;
        if clears(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    Ok(low)
}

/// Six hex digits back to oklch, which is the same matrices in the other order.
///
/// Lived in `console-theme` while this was Python, reaching past the front of
/// this module for the linear transfer function to do it. It is colour and it
/// belongs here.
pub fn to_oklch(code: &str) -> (f64, f64, f64) {
    let [red, green, blue] = channels(code).map(to_linear);
    let [long, medium, short] = TO_LMS.map(|row| {
        (row[0] * red + row[1] * green + row[2] * blue).powf(1.0 / 3.0)
    });
    let lightness = 0.2104542553 * long + 0.7936177850 * medium - 0.0040720468 * short;
    let a = 1.9779984951 * long - 2.4285922050 * medium + 0.4505937099 * short;
    let b = 0.0259040371 * long + 0.7827717662 * medium - 0.8086757660 * short;
    (lightness, a.hypot(b), b.atan2(a).to_degrees().rem_euclid(360.0))
}

/// A colour moved towards white by `amount`, keeping its hue.
///
/// What a terminal means by bright: the same colour with more light in it.
/// Mixing towards white instead would take the colour out as well.
pub fn lift(code: &str, amount: f64) -> String {
    let (lightness, chroma, hue) = to_oklch(code);
    hexcode((lightness + amount).min(1.0), chroma, hue)
}
