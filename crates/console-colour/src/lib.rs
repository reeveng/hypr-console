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


use console_number::toward_zero_u8;
pub mod spent;

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
    // Exactly between the two integers is the case this function exists for,
    // and it goes to the even one: `floor` where floor is even, the one above
    // it where floor is odd. Anywhere else is simply the nearer of the two.
    let up = match rest == 0.5 {
        true => (floor / 2.0).fract() != 0.0,
        false => rest > 0.5,
    };

    match up {
        true => floor + 1.0,
        false => floor,
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

/// Whether a screen can actually show a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gamut {
    /// Every channel lands inside sRGB, so the screen shows what was asked for.
    Inside,
    /// At least one does not, and the screen would clip it to something else.
    Outside,
}

/// Whether a screen can actually show it.
pub fn in_gamut(lightness: f64, chroma: f64, hue: f64) -> Gamut {
    let inside = oklch_to_rgb(lightness, chroma, hue)
        .iter()
        .all(|channel| (-0.0001..=1.0001).contains(channel));

    match inside {
        true => Gamut::Inside,
        false => Gamut::Outside,
    }
}

/// The same colour with just enough chroma taken out of it to be real.
///
/// Lightness is what the contrast was worked out from, so it is held and the
/// saturation gives way. A pastel that has lost a little chroma is still the
/// colour it was meant to be; one that has lost lightness is a different one.
pub fn fit(lightness: f64, chroma: f64, hue: f64) -> f64 {
    if in_gamut(lightness, chroma, hue) == Gamut::Inside {
        return chroma;
    }

    let (mut low, mut high) = (0.0, chroma);

    for _ in 0..40 {
        let middle = (low + high) / 2.0;

        match in_gamut(lightness, middle, hue) {
            Gamut::Inside => low = middle,
            Gamut::Outside => high = middle,
        }
    }

    low
}

/// A colour as six hex digits, fitted into the gamut on the way.
pub fn hexcode(lightness: f64, chroma: f64, hue: f64) -> String {
    oklch_to_rgb(lightness, fit(lightness, chroma, hue), hue)
        .iter()
        .map(|channel| {
            format!("{:02x}", toward_zero_u8(round_half_even(channel.clamp(0.0, 1.0) * 255.0)))
        })
        .collect()
}

/// The three channels of six hex digits, as the bytes they are written as.
///
/// Everything that reaches here is a code this crate generated or one out of a
/// palette it generated, so a pair that will not read is a bug upstream rather
/// than somebody's input. The channel is nought and the run goes on -- the
/// arithmetic below has no way to say "this is not a colour" and every caller
/// of it is drawing something -- but it is said out loud, because a theme that
/// comes out with one channel quietly black is a fault nobody would think to
/// look for here.
fn bytes(code: &str) -> [u8; 3] {
    let code = code.trim_start_matches('#').as_bytes();
    let mut out = [0u8; 3];

    for (channel, i) in out.iter_mut().zip([0, 2, 4]) {
        let pair = match std::str::from_utf8(&code[i..i + 2]) {
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

    out
}

/// The same three, as floats from zero to one.
fn channels(code: &str) -> [f64; 3] {
    bytes(code).map(|channel| f64::from(channel) / 255.0)
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

/// Where APCA stops trusting a luminance, and the shape of the lift it gives
/// it instead.
///
/// The disagreement between the two measures lives almost entirely in these
/// two numbers. WCAG adds a flat term to both sides of its ratio, standing in
/// for the light the room throws on the screen; the effect is that two dark
/// colours are divided by a constant that swamps them and the ratio comes out
/// generous. This lifts the dark end rather than flattening it, so the
/// difference between two near-blacks is scored as the small thing it is.
const BLACK_THRESHOLD: f64 = 0.022;
const BLACK_CLAMP: f64 = 1.414;

/// The exponents either side of the polarity, the scale that puts the answer
/// on a hundred-point run, and the offset taken off it at the end.
///
/// Ink and ground are raised to different powers, and to different powers
/// again depending on which of them is the darker. That asymmetry is the whole
/// point of the measure: pale ink on a dark ground bleeds into the ground and
/// reads thinner than the same pair turned the other way up, and a ratio
/// cannot say so, because a ratio does not know which one is the paper.
const GROUND_ON_LIGHT: f64 = 0.56;
const INK_ON_LIGHT: f64 = 0.57;
const INK_ON_DARK: f64 = 0.62;
const GROUND_ON_DARK: f64 = 0.65;
const SCALE: f64 = 1.14;
const OFFSET: f64 = 0.027;

/// Under this, the answer is not a faint contrast. It is none, and saying so
/// is more honest than reporting a number nobody could have seen.
const CLIP: f64 = 0.1;

/// Two luminances nearer than this are one colour written twice.
const SAME: f64 = 0.0005;

/// Luminance as APCA reads it, which is not luminance as WCAG reads it.
///
/// A plain power rather than sRGB's piecewise curve, because the piecewise
/// segment near black describes an encoding and not an eye, and then the soft
/// clamp above.
fn apca_luminance(code: &str) -> f64 {
    let [red, green, blue] = channels(code);
    let luminance =
        0.2126729 * red.powf(2.4) + 0.7151522 * green.powf(2.4) + 0.0721750 * blue.powf(2.4);

    match luminance > BLACK_THRESHOLD {
        true => luminance,
        false => luminance + (BLACK_THRESHOLD - luminance).powf(BLACK_CLAMP),
    }
}

/// How far apart two colours are as APCA reads them, as a signed `Lc`.
///
/// The sign is the polarity rather than an error: dark ink on a light ground
/// comes back positive, pale ink on a dark ground negative, and the two are
/// not interchangeable. Everything on this desktop is the second kind.
///
/// The magnitude does not convert to a ratio and there is no table that turns
/// one into the other, because the two measures disagree about the thing being
/// measured. What the magnitude means is set out in `Floor`.
pub fn lc(ink: &str, ground: &str) -> f64 {
    let (ink, ground) = (apca_luminance(ink), apca_luminance(ground));

    if (ground - ink).abs() < SAME {
        return 0.0;
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

    match raw.abs() < CLIP {
        true => 0.0,
        false => (raw + offset) * 100.0,
    }
}

/// What a pairing has to clear, in both measures at once.
///
/// Not either of them, and not whichever is convenient. WCAG is what the law
/// asks for and what a checker will report; APCA is what the eye does. On a
/// palette this dark they disagree in one direction, and it is not the
/// flattering one: the ratio's flare term means a pair of dark colours can
/// clear AAA and still sit under what APCA calls readable at all. Asking for
/// both and letting whichever is harder decide is the only reading of "as
/// strong as we can make it" that does not quietly drop one of them.
///
/// `lc` is unsigned. Which way round a pairing sits is a fact about the
/// pairing, not something a floor is entitled to ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Floor {
    /// WCAG 2.x, from 1:1 to 21:1. AAA for text is 7, AA is 4.5, and 3 is the
    /// floor for a line that carries meaning without being read.
    pub ratio: f64,
    /// APCA `Lc`. Body text is wanted at 75 and preferred at 90, a headline or
    /// something deliberately quiet at 45, a border at 30. Nought asks for
    /// nothing, which is right for a pairing that only has to be seen as a
    /// different thing and is wrong everywhere else.
    pub lc: f64,
}

/// Whether a colour is far enough from what it is read against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clears {
    /// It clears the floor, on both measures at once.
    Yes,
    /// It falls short on at least one of them.
    No,
}

impl Floor {
    /// Whether one colour clears this against another, both ways at once.
    pub fn cleared_by(self, ink: &str, ground: &str) -> Clears {
        match contrast(ink, ground) >= self.ratio && lc(ink, ground).abs() >= self.lc {
            true => Clears::Yes,
            false => Clears::No,
        }
    }

    /// Whether it clears against every ground it is read on.
    pub fn clears_all(self, ink: &str, grounds: &[String]) -> Clears {
        match grounds.iter().all(|ground| self.cleared_by(ink, ground) == Clears::Yes) {
            true => Clears::Yes,
            false => Clears::No,
        }
    }
}

impl fmt::Display for Floor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}:1 and Lc {}", self.ratio, self.lc)
    }
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
            let mixed = f64::from(*front) * alpha + f64::from(back) * (1.0 - alpha);
            format!("{:02x}", toward_zero_u8(round_half_even(mixed)))
        })
        .collect()
}

/// The darkest lightness at which a hue clears `floor` against every ground.
///
/// Darkest, because a pastel that is lighter than it needs to be is a pastel
/// on its way to white, and ten of those are one colour. Both measures climb
/// with lightness against a dark ground and neither falls, so the answer is
/// still found by halving even though there are now two of them: a shade that
/// clears the harder of the two clears the other on the way.
pub fn lightest_clearing(
    chroma: f64,
    hue: f64,
    grounds: &[String],
    floor: Floor,
    from: f64,
) -> Result<f64, Short> {
    if grounds.is_empty() {
        return Ok(from);
    }

    let clears = |lightness: f64| floor.clears_all(&hexcode(lightness, chroma, hue), grounds);

    if clears(from) == Clears::Yes {
        return Ok(from);
    }

    let (mut low, mut high) = (from, 1.0);

    if clears(high) == Clears::No {
        return Err(Short(format!(
            "nothing at hue {hue} clears {floor} against {grounds:?}"
        )));
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

/// The lightest lightness at which a hue clears `floor` under every ceiling.
///
/// The mirror of the one above, for ink that is painted on top of a fill: the
/// fill is already decided and the ink has to be dark enough against it.
pub fn darkest_clearing(
    chroma: f64,
    hue: f64,
    ceilings: &[String],
    floor: Floor,
) -> Result<f64, Short> {
    let clears = |lightness: f64| floor.clears_all(&hexcode(lightness, chroma, hue), ceilings);

    if ceilings.is_empty() || clears(1.0) == Clears::Yes {
        return Ok(1.0);
    }

    let (mut low, mut high) = (0.0, 1.0);

    if clears(low) == Clears::No {
        return Err(Short(format!(
            "nothing at hue {hue} clears {floor} under {ceilings:?}"
        )));
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
