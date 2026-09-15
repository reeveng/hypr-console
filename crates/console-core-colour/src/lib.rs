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
//!
//! What "clears it" means is two things at once. WCAG's ratio is the one the
//! law asks for and the one every checker reports. APCA is the one that knows
//! which of the two colours is the paper, and on a dark desktop that is the
//! difference that decides whether a shade is actually readable. They
//! disagree, and where they disagree this asks for both and takes whichever
//! binds harder. See `Floor`.
//!
//! ## Why a colour is not three numbers, and a pair is not two strings
//!
//! `oklch_to_rgb(lightness, chroma, hue)` was three `f64` in an order that was
//! right because somebody remembered it, and the compiler took them in any
//! order and said nothing. `Oklch` names its three, so the order is gone: a
//! colour is written `Oklch { lightness, chroma, hue }` and there is nothing
//! left to get backwards.
//!
//! The pairs are worse, because they are asymmetric and read as if they are
//! not. `lc(ink, ground)` answers a signed number whose sign is which of the
//! two is the paper, so swapping them does not fail, it lies. `Ink` and
//! `Ground` are one word each at the call site and the swap stops compiling.
//! `contrast` takes them too, though its arithmetic is symmetric: every caller
//! has an ink and a ground in hand, and a reader should not have to know which
//! of the two functions cares.


use console_core_never::Never;
use console_core_number_conversion::toward_zero_u8;
pub mod spent;

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Short(pub String);

impl fmt::Display for Short {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl std::error::Error for Short {}

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

const TO_LAB: [[f64; 3]; 3] = [
    [0.2104542553, 0.7936177850, -0.0040720468],
    [1.9779984951, -2.4285922050, 0.4505937099],
    [0.0259040371, 0.7827717662, -0.8086757660],
];

const FROM_LCH: [[f64; 2]; 3] = [
    [0.3963377774, 0.2158037573],
    [-0.1055613458, -0.0638541728],
    [-0.0894841775, -1.2914855480],
];

fn round_half_even(value: f64) -> Result<f64, Never> {
    let floor = value.floor();
    let rest = value - floor;
    let up = match rest == 0.5 {
        true => (floor / 2.0).fract() != 0.0,
        false => rest > 0.5,
    };

    Ok(match up {
        true => floor + 1.0,
        false => floor,
    })
}

fn to_srgb(channel: f64) -> Result<f64, Never> {
    Ok(match channel <= 0.0031308 {
        true => 12.92 * channel,
        false => 1.055 * channel.powf(1.0 / 2.4) - 0.055,
    })
}

pub fn to_linear(channel: f64) -> Result<f64, Never> {
    Ok(match channel <= 0.04045 {
        true => channel / 12.92,
        false => ((channel + 0.055) / 1.055).powf(2.4),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub lightness: f64,
    pub chroma: f64,
    pub hue: f64,
}

impl Oklch {
    pub fn of(colour: Rgba) -> Result<Oklch, Never> {
        let Ok(red) = to_linear(f64::from(colour.red) / 255.0);
        let Ok(green) = to_linear(f64::from(colour.green) / 255.0);
        let Ok(blue) = to_linear(f64::from(colour.blue) / 255.0);
        let linear = [red, green, blue];
        let mut roots = [0.0; 3];

        for (root, row) in roots.iter_mut().zip(TO_LMS) {
            let mixed: f64 = row.iter().zip(&linear).map(|(weight, channel)| weight * channel).sum();

            *root = mixed.cbrt();
        }

        let mut lab = [0.0; 3];

        for (axis, row) in lab.iter_mut().zip(TO_LAB) {
            let mixed: f64 = row.iter().zip(&roots).map(|(weight, root)| weight * root).sum();

            *axis = mixed;
        }

        let [lightness, a, b] = lab;

        Ok(Oklch {
            lightness,
            chroma: a.hypot(b),
            hue: b.atan2(a).to_degrees().rem_euclid(360.0),
        })
    }

    pub fn at(self, lightness: f64) -> Result<Self, Never> {
        Ok(Oklch { lightness, ..self })
    }

    pub fn with(self, chroma: f64) -> Result<Self, Never> {
        Ok(Oklch { chroma, ..self })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ink<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ground<'a>(pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    Length(usize),
    Digit(char),
}

impl fmt::Display for Unreadable {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unreadable::Length(many) => {
                write!(to, "a colour is six hex digits, and this is {many}")
            }
            Unreadable::Digit(said) => write!(to, "{said} is not a hex digit"),
        }
    }
}

impl std::error::Error for Unreadable {}

impl Rgba {
    pub const OPAQUE: u8 = 0xff;

    pub fn of(six: &str) -> Result<Rgba, Unreadable> {
        let said = match six.strip_prefix('#') {
            Some(after) => after,
            None => six,
        };

        match said.chars().count() {
            6 => {},
            many => return Err(Unreadable::Length(many)),
        }

        let mut digits = said.chars();
        let red = band(&mut digits)?;
        let green = band(&mut digits)?;
        let blue = band(&mut digits)?;

        Ok(Rgba { red, green, blue, alpha: Rgba::OPAQUE })
    }

    pub fn at(self, alpha: u8) -> Result<Rgba, Never> {
        Ok(Rgba { alpha, ..self })
    }
}

fn band(digits: &mut impl Iterator<Item = char>) -> Result<u8, Unreadable> {
    let high = match digits.next() {
        Some(digit) => digit,
        None => return Err(Unreadable::Length(0)),
    };
    let low = match digits.next() {
        Some(digit) => digit,
        None => return Err(Unreadable::Length(0)),
    };
    let sixteens = nibble(high)?;
    let ones = nibble(low)?;

    Ok(sixteens.saturating_mul(16).saturating_add(ones))
}

fn nibble(digit: char) -> Result<u8, Unreadable> {
    match digit.to_digit(16) {
        Some(many) => {
            let Ok(byte) = toward_zero_u8(f64::from(many));

            Ok(byte)
        }
        None => Err(Unreadable::Digit(digit)),
    }
}

pub fn oklch_to_rgb(colour: Oklch) -> Result<[f64; 3], Never> {
    let radians = colour.hue.to_radians();
    let (a, b) = (colour.chroma * radians.cos(), colour.chroma * radians.sin());
    let cubed: Vec<f64> = FROM_LCH
        .iter()
        .map(|&[on_a, on_b]| (colour.lightness + on_a * a + on_b * b).powf(3.0))
        .collect();
    let mut rgb = [0.0; 3];

    for (channel, row) in rgb.iter_mut().zip(FROM_LMS) {
        let mixed: f64 = row.iter().zip(&cubed).map(|(weight, cube)| weight * cube).sum();

        let Ok(said) = to_srgb(mixed);

        *channel = said;
    }

    Ok(rgb)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gamut {
    Inside,
    Outside,
}

pub fn in_gamut(colour: Oklch) -> Result<Gamut, Never> {
    let Ok(rgb) = oklch_to_rgb(colour);

    let inside = rgb.iter().all(|channel| (-0.0001..=1.0001).contains(channel));

    Ok(match inside {
        true => Gamut::Inside,
        false => Gamut::Outside,
    })
}

pub fn fit(colour: Oklch) -> Result<f64, Never> {
    let Ok(whole) = in_gamut(colour);

    match whole {
        Gamut::Inside => return Ok(colour.chroma),
        Gamut::Outside => {}
    }

    let (mut low, mut high) = (0.0, colour.chroma);

    for _ in 0..40 {
        let middle = (low + high) / 2.0;

        let Ok(narrowed) = colour.with(middle);
        let Ok(gamut) = in_gamut(narrowed);

        match gamut {
            Gamut::Inside => low = middle,
            Gamut::Outside => high = middle,
        }
    }

    Ok(low)
}

pub fn hexcode(colour: Oklch) -> Result<String, Never> {
    let Ok(fitted) = fit(colour);
    let Ok(narrowed) = colour.with(fitted);
    let Ok(rgb) = oklch_to_rgb(narrowed);

    let mut code = String::new();

    for channel in rgb {
        let Ok(rounded) = round_half_even(channel.clamp(0.0, 1.0) * 255.0);
        let Ok(byte) = toward_zero_u8(rounded);

        code.push_str(&format!("{byte:02x}"));
    }

    Ok(code)
}

fn bytes(code: &str) -> Result<[u8; 3], Never> {
    let code = code.trim_start_matches('#').as_bytes();
    let mut out = [0u8; 3];

    for (channel, i) in out.iter_mut().zip([0usize, 2, 4]) {
        let two = match code.get(i..i.saturating_add(2)) {
            Some(two) => two,
            None => {
                eprintln!("a colour is too short to hold three channels; read as nought");
                *channel = 0;
                continue;
            }
        };

        let pair = match std::str::from_utf8(two) {
            Ok(said) => said,
            Err(_) => {
                eprintln!("a colour holds bytes that are not text; read as nought");
                *channel = 0;
                continue;
            },
        };

        *channel = match u8::from_str_radix(pair, 16) {
            Ok(number) => number,
            Err(_) => {
                eprintln!("{pair:?} in a colour is not a hex number; read as nought");
                0
            },
        };
    }

    Ok(out)
}

fn channels(code: &str) -> Result<[f64; 3], Never> {
    let Ok(bytes) = bytes(code);

    Ok(bytes.map(|channel| f64::from(channel) / 255.0))
}

pub fn luminance(code: &str) -> Result<f64, Never> {
    let Ok([red, green, blue]) = channels(code);
    let Ok(red) = to_linear(red);
    let Ok(green) = to_linear(green);
    let Ok(blue) = to_linear(blue);

    Ok(0.2126 * red + 0.7152 * green + 0.0722 * blue)
}

pub fn contrast(ink: Ink<'_>, ground: Ground<'_>) -> Result<f64, Never> {
    let Ok(first) = luminance(ink.0);
    let Ok(second) = luminance(ground.0);

    Ok((first.max(second) + 0.05) / (first.min(second) + 0.05))
}

const BLACK_THRESHOLD: f64 = 0.022;
const BLACK_CLAMP: f64 = 1.414;

const GROUND_ON_LIGHT: f64 = 0.56;
const INK_ON_LIGHT: f64 = 0.57;
const INK_ON_DARK: f64 = 0.62;
const GROUND_ON_DARK: f64 = 0.65;
const SCALE: f64 = 1.14;
const OFFSET: f64 = 0.027;

const CLIP: f64 = 0.1;

const SAME: f64 = 0.0005;

fn apca_luminance(code: &str) -> Result<f64, Never> {
    let Ok([red, green, blue]) = channels(code);

    let luminance =
        0.2126729 * red.powf(2.4) + 0.7151522 * green.powf(2.4) + 0.0721750 * blue.powf(2.4);

    Ok(match luminance > BLACK_THRESHOLD {
        true => luminance,
        false => luminance + (BLACK_THRESHOLD - luminance).powf(BLACK_CLAMP),
    })
}

pub fn lc(ink: Ink<'_>, ground: Ground<'_>) -> Result<f64, Never> {
    let Ok(ink) = apca_luminance(ink.0);
    let Ok(ground) = apca_luminance(ground.0);

    match (ground - ink).abs() < SAME {
        true => return Ok(0.0),
        false => {}
    }

    let (raw, offset) = match ground > ink {
        true => (
            (ground.powf(GROUND_ON_LIGHT) - ink.powf(INK_ON_LIGHT)) * SCALE,
            -OFFSET,
        ),
        false => (
            (ground.powf(GROUND_ON_DARK) - ink.powf(INK_ON_DARK)) * SCALE,
            OFFSET,
        ),
    };

    Ok(match raw.abs() < CLIP {
        true => 0.0,
        false => (raw + offset) * 100.0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Floor {
    pub ratio: f64,
    pub lc: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clears {
    Yes,
    No,
}

impl Floor {
    pub fn cleared_by(self, ink: Ink<'_>, ground: Ground<'_>) -> Result<Clears, Never> {
        let Ok(contrast) = contrast(ink, ground);
        let Ok(lc) = lc(ink, ground);

        Ok(match contrast >= self.ratio && lc.abs() >= self.lc {
            true => Clears::Yes,
            false => Clears::No,
        })
    }

    pub fn clears_all(self, ink: Ink<'_>, grounds: &[String]) -> Result<Clears, Never> {
        for ground in grounds {
            let Ok(cleared) = self.cleared_by(ink, Ground(ground));

            match cleared {
                Clears::Yes => {}
                Clears::No => return Ok(Clears::No),
            }
        }

        Ok(Clears::Yes)
    }
}

impl fmt::Display for Floor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}:1 and Lc {}", self.ratio, self.lc)
    }
}

pub fn over(ink: Ink<'_>, ground: Ground<'_>, alpha: f64) -> Result<String, Never> {
    let Ok(top) = bytes(ink.0);
    let Ok(bottom) = bytes(ground.0);

    let mut code = String::new();

    for (front, back) in top.iter().zip(bottom) {
        let mixed = f64::from(*front) * alpha + f64::from(back) * (1.0 - alpha);

        let Ok(rounded) = round_half_even(mixed);
        let Ok(byte) = toward_zero_u8(rounded);

        code.push_str(&format!("{byte:02x}"));
    }

    Ok(code)
}

pub fn lightest_clearing(
    from: Oklch,
    grounds: &[String],
    floor: Floor,
) -> Result<f64, Short> {
    match grounds.is_empty() {
        true => return Ok(from.lightness),
        false => {}
    }

    let clears = |lightness: f64| {
        let Ok(at) = from.at(lightness);
        let Ok(code) = hexcode(at);
        let Ok(clears) = floor.clears_all(Ink(&code), grounds);

        clears
    };

    match clears(from.lightness) {
        Clears::Yes => return Ok(from.lightness),
        Clears::No => {}
    }

    let (mut low, mut high) = (from.lightness, 1.0);

    match clears(high) {
        Clears::Yes => {}
        Clears::No => {
            let hue = from.hue;

            return Err(Short(format!("nothing at hue {hue} clears {floor} against {grounds:?}")));
        }
    }

    for _ in 0..48 {
        let middle = (low + high) / 2.0;

        match clears(middle) {
            Clears::Yes => high = middle,
            Clears::No => low = middle,
        }
    }

    Ok(high)
}

pub fn darkest_clearing(
    from: Oklch,
    ceilings: &[String],
    floor: Floor,
) -> Result<f64, Short> {
    let clears = |lightness: f64| {
        let Ok(at) = from.at(lightness);
        let Ok(code) = hexcode(at);
        let Ok(clears) = floor.clears_all(Ink(&code), ceilings);

        clears
    };

    let nothing_to_clear = ceilings.is_empty() || clears(from.lightness) == Clears::Yes;

    match nothing_to_clear {
        true => return Ok(from.lightness),
        false => {}
    }

    let (mut low, mut high) = (0.0, from.lightness);

    match clears(low) {
        Clears::Yes => {}
        Clears::No => {
            let hue = from.hue;

            return Err(Short(format!("nothing at hue {hue} clears {floor} under {ceilings:?}")));
        }
    }

    for _ in 0..48 {
        let middle = (low + high) / 2.0;

        match clears(middle) {
            Clears::Yes => low = middle,
            Clears::No => high = middle,
        }
    }

    Ok(low)
}

pub fn to_oklch(code: &str) -> Result<Oklch, Never> {
    let Ok([red, green, blue]) = channels(code);
    let Ok(red) = to_linear(red);
    let Ok(green) = to_linear(green);
    let Ok(blue) = to_linear(blue);

    let [long, medium, short] = TO_LMS.map(|[on_red, on_green, on_blue]| {
        (on_red * red + on_green * green + on_blue * blue).powf(1.0 / 3.0)
    });
    let lightness = 0.2104542553 * long + 0.7936177850 * medium - 0.0040720468 * short;
    let a = 1.9779984951 * long - 2.4285922050 * medium + 0.4505937099 * short;
    let b = 0.0259040371 * long + 0.7827717662 * medium - 0.8086757660 * short;

    Ok(Oklch {
        lightness,
        chroma: a.hypot(b),
        hue: b.atan2(a).to_degrees().rem_euclid(360.0),
    })
}

pub fn lift(code: &str, amount: f64) -> Result<String, Never> {
    let Ok(colour) = to_oklch(code);
    let Ok(lifted) = colour.at((colour.lightness + amount).min(1.0));

    hexcode(lifted)
}

#[cfg(test)]
mod the_channels {
    use super::*;

    #[test]
    fn six_hex_digits_are_three_channels_and_an_opaque_fourth() {
        assert_eq!(
            Rgba::of("deadbe"),
            Ok(Rgba { red: 0xde, green: 0xad, blue: 0xbe, alpha: 0xff })
        );
    }

    #[test]
    fn a_leading_hash_is_the_same_colour() {
        assert_eq!(Rgba::of("#1a2b3c"), Rgba::of("1a2b3c"));
    }

    #[test]
    fn a_colour_the_wrong_length_says_so_rather_than_coming_out_black() {
        assert_eq!(Rgba::of("1a2b3"), Err(Unreadable::Length(5)));
        assert_eq!(Rgba::of(""), Err(Unreadable::Length(0)));
    }

    #[test]
    fn something_that_is_not_a_colour_names_the_digit_it_gave_up_on() {
        assert_eq!(Rgba::of("ff00zz"), Err(Unreadable::Digit('z')));
    }

    #[test]
    fn what_the_palette_spends_is_read_back_the_same_either_case() {
        assert_eq!(Rgba::of("FF8800"), Rgba::of("ff8800"));
    }
}

#[cfg(test)]
mod the_space {
    use super::*;

    fn round_trip(six: &str) -> String {
        let colour = match Rgba::of(six) {
            Ok(colour) => colour,
            Err(why) => panic!("{six} should read: {why}"),
        };
        let Ok(oklch) = Oklch::of(colour);
        let Ok(again) = hexcode(oklch);

        again
    }

    #[test]
    fn a_colour_the_palette_spends_comes_back_as_itself() {
        for six in ["110b12", "231b26", "372c3a", "723b5f", "8a7d8e", "f7e7f3", "ffc2e7"] {
            assert_eq!(round_trip(six), six, "{six} did not survive the space");
        }
    }

    #[test]
    fn the_ends_of_every_channel_survive() {
        for six in ["000000", "ffffff", "ff0000", "00ff00", "0000ff"] {
            assert_eq!(round_trip(six), six, "{six} did not survive the space");
        }
    }

    #[test]
    fn grey_has_no_chroma_to_speak_of() {
        let colour = match Rgba::of("808080") {
            Ok(colour) => colour,
            Err(why) => panic!("grey should read: {why}"),
        };
        let Ok(oklch) = Oklch::of(colour);

        assert!(oklch.chroma < 0.0001, "grey came out with a hue: {oklch:?}");
    }
}
