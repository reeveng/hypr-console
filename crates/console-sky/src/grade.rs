//! Somebody else's picture, brought into this palette.
//!
//! The wallpapers are drawn by an artist who never heard of this machine, so
//! they arrive in their own colours: a river in bright greens, a campfire in
//! olive and brown. The bar sits over them in pink on plum, and a picture that
//! shares no colour with the thing standing on it reads as two pictures.
//!
//! What is done about it is not a filter chosen by eye. The palette already
//! holds a ramp from its darkest ground to its lightest ink, and that ramp has
//! a hue: this whole theme is plum. So a pixel is asked how light it is, the
//! ramp is asked what colour the theme is at that lightness, and the answer is
//! mixed with the colour the pixel already had. How much of each is the one
//! decision, and it is declared per picture in `theme/sky.toml` rather than
//! written here.
//!
//! Mixing happens in Oklab's a and b, not in hue and chroma. Hue is an angle,
//! and the average of two angles is a question with two answers; the average of
//! two points on a plane is one point. A green pulled halfway to plum through
//! the plane passes through grey, which is what fading a colour out looks like,
//! and pulled through the angle it would pass through orange, which is what a
//! different picture looks like.

use console_colour::{fit, oklch_to_rgb, to_oklch};

/// A colour as Oklab holds it: how light, and where on the plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lab {
    pub lightness: f64,
    pub a: f64,
    pub b: f64,
}

impl Lab {
    /// From a hexcode, without its hash.
    pub fn of(code: &str) -> Self {
        let (lightness, chroma, hue) = to_oklch(code);
        Lab::polar(lightness, chroma, hue)
    }

    /// From how light, how much colour, and which colour.
    pub fn polar(lightness: f64, chroma: f64, hue: f64) -> Self {
        let radians = hue.to_radians();
        Lab { lightness, a: chroma * radians.cos(), b: chroma * radians.sin() }
    }

    /// How much colour it holds.
    pub fn chroma(&self) -> f64 {
        self.a.hypot(self.b)
    }

    /// Which colour it is, in degrees.
    pub fn hue(&self) -> f64 {
        self.b.atan2(self.a).to_degrees()
    }

    /// Somewhere between this one and another, on the straight line.
    pub fn towards(&self, other: &Lab, how_far: f64) -> Lab {
        let mix = |from: f64, to: f64| from + (to - from) * how_far;
        Lab {
            lightness: mix(self.lightness, other.lightness),
            a: mix(self.a, other.a),
            b: mix(self.b, other.b),
        }
    }

    /// The nearest real colour, as a screen would show it.
    ///
    /// Chroma is what gives way, because lightness is what the shape of the
    /// picture is made of: a leaf that has lost a little of its green is still
    /// a leaf, and one that has lost its lightness is a hole.
    pub fn rgb(&self) -> [f64; 3] {
        let (chroma, hue) = (self.chroma(), self.hue());
        let held = fit(self.lightness, chroma, hue);
        oklch_to_rgb(self.lightness, held, hue)
            .map(|channel| channel.clamp(0.0, 1.0))
    }
}

/// The theme's own greyscale: its grounds and its inks, darkest first.
///
/// These are the colours everything on this desktop is already made of, so a
/// picture graded through them cannot land on a colour the desktop does not
/// hold. They are looked up by lightness rather than by name, which is why the
/// order they are given in does not matter and their lightnesses do.
pub const RAMP: [&str; 6] = ["night", "ground", "panel", "ash", "soft", "text"];

/// The palette's ramp, as a curve a lightness can be looked up in.
pub struct Ramp {
    stops: Vec<Lab>,
}

impl Ramp {
    /// Built from the palette, out of the names in `RAMP`.
    pub fn read(colours: &dyn Fn(&str) -> Option<String>) -> Result<Self, String> {
        let mut stops: Vec<Lab> = RAMP
            .iter()
            .map(|name| {
                colours(name)
                    .map(|code| Lab::of(&code))
                    .ok_or_else(|| format!("the palette names no {name}"))
            })
            .collect::<Result<_, _>>()?;
        stops.sort_by(|one, other| one.lightness.total_cmp(&other.lightness));
        Ok(Ramp { stops })
    }

    /// How dark the theme goes, and how light.
    pub fn ends(&self) -> (f64, f64) {
        let first = self.stops.first().expect("the ramp has stops");
        let last = self.stops.last().expect("the ramp has stops");
        (first.lightness, last.lightness)
    }

    /// The theme's colour at a given lightness, interpolated between stops.
    ///
    /// Outside the ramp's own ends there is nothing to interpolate towards, so
    /// the end itself is the answer: a highlight brighter than the lightest ink
    /// takes that ink's colour, and keeps its own lightness, which is applied
    /// separately.
    pub fn at(&self, lightness: f64) -> Lab {
        let above = self.stops.iter().position(|stop| stop.lightness >= lightness);
        match above {
            None => *self.stops.last().expect("the ramp has stops"),
            Some(0) => self.stops[0],
            Some(next) => {
                let (under, over) = (self.stops[next - 1], self.stops[next]);
                let span = over.lightness - under.lightness;
                let how_far = match span > 0.0 {
                    true => (lightness - under.lightness) / span,
                    false => 0.0,
                };
                under.towards(&over, how_far)
            }
        }
    }
}

/// How far a picture is brought over, declared per picture.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Grade {
    /// How much of the picture's own colour survives. One is the artist's
    /// colour untouched, zero is the theme's colour at that lightness.
    pub keep: f64,
    /// How much of the theme's colour is added on top of what survived. This
    /// is what makes a green river read as plum rather than as a grey one.
    pub pull: f64,
    /// Where the picture's blacks and whites land, as a share of the lightest
    /// ink the theme holds. A picture that keeps its own full range fights the
    /// bar standing on it; one with its top taken off is a picture the bar sits
    /// on top of. A floor of zero is a real black, which is darker than any
    /// colour the palette holds and is the right ground for a night sky.
    pub floor: f64,
    pub ceiling: f64,
}

impl Default for Grade {
    fn default() -> Self {
        Grade { keep: 0.55, pull: 0.45, floor: 0.0, ceiling: 0.72 }
    }
}

/// One colour, brought over.
pub fn grade(ramp: &Ramp, how: &Grade, rgb: [f64; 3]) -> [f64; 3] {
    let code = format!(
        "{:02x}{:02x}{:02x}",
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8
    );
    let was = Lab::of(&code);
    let theme = ramp.at(was.lightness);

    let (_, light) = ramp.ends();
    let (low, high) = (light * how.floor, light * how.ceiling);

    Lab {
        lightness: (low + (high - low) * was.lightness).clamp(0.0, 1.0),
        a: was.a * how.keep + theme.a * how.pull,
        b: was.b * how.keep + theme.b * how.pull,
    }
    .rgb()
}

/// The grade, written out as a cube ffmpeg can apply to every frame.
///
/// A LUT rather than a filter chain because the grade is arithmetic in a
/// perceptual space and ffmpeg's filters are arithmetic in sRGB. Working it out
/// here for a lattice of colours and letting ffmpeg interpolate between them
/// means the picture is graded by this file's rules, at ffmpeg's speed.
pub fn cube(ramp: &Ramp, how: &Grade, side: usize) -> String {
    let mut out = String::from("# The Blossom palette, as a grade.\n");
    out.push_str(&format!("LUT_3D_SIZE {side}\n"));
    let step = |index: usize| index as f64 / (side - 1) as f64;
    // Red runs fastest and blue slowest, which is the order a cube is read in.
    for blue in 0..side {
        for green in 0..side {
            for red in 0..side {
                let done = grade(ramp, how, [step(red), step(green), step(blue)]);
                out.push_str(&format!("{:.6} {:.6} {:.6}\n", done[0], done[1], done[2]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blossom(name: &str) -> Option<String> {
        let colours = [
            ("night", "110b12"),
            ("ground", "231b26"),
            ("panel", "372c3a"),
            ("ash", "916f8d"),
            ("soft", "cdb6c9"),
            ("text", "ebdce7"),
        ];
        colours
            .iter()
            .find(|(held, _)| *held == name)
            .map(|(_, code)| (*code).to_string())
    }

    /// A graded colour, back as a hexcode.
    fn hex(rgb: [f64; 3]) -> String {
        format!(
            "{:02x}{:02x}{:02x}",
            (rgb[0] * 255.0).round() as u8,
            (rgb[1] * 255.0).round() as u8,
            (rgb[2] * 255.0).round() as u8
        )
    }

    fn ramp() -> Ramp {
        Ramp::read(&blossom).expect("the ramp reads")
    }

    #[test]
    fn the_ramp_runs_from_the_darkest_ground_to_the_lightest_ink() {
        let (dark, light) = ramp().ends();
        assert!((dark - Lab::of("110b12").lightness).abs() < 1e-9);
        assert!((light - Lab::of("ebdce7").lightness).abs() < 1e-9);
    }

    /// A ramp asked for a stop it holds gives that stop back, not a blend of
    /// its neighbours.
    #[test]
    fn a_lightness_the_ramp_holds_reads_as_the_colour_it_holds_there() {
        let panel = Lab::of("372c3a");
        let found = ramp().at(panel.lightness);
        assert!((found.a - panel.a).abs() < 1e-9);
        assert!((found.b - panel.b).abs() < 1e-9);
    }


    /// The whole point of grading in the plane rather than in the angle: what
    /// `keep` holds on to is the colour, and it holds on to it exactly.
    /// Lightness is squeezed into the theme's range whatever `keep` says,
    /// because that is what `floor` and `ceiling` are for.
    #[test]
    fn keeping_all_of_a_colour_and_pulling_none_leaves_its_colour_alone() {
        let how = Grade { keep: 1.0, pull: 0.0, floor: 0.0, ceiling: 1.0 };
        let green = [0.227, 0.525, 0.329];
        let was = Lab::of("3a8654");
        let done = grade(&ramp(), &how, green);
        let is = Lab::of(&hex(done));
        assert!((was.hue() - is.hue()).abs() < 2.0, "{green:?} became {done:?}");
    }

    /// A picture is brought inside the theme's range from both ends, so the
    /// bar has something to stand on and the darkest corner is not a hole.
    #[test]
    fn a_graded_picture_lands_inside_the_range_it_was_given() {
        let (_, light) = ramp().ends();
        let how = Grade::default();
        let (low, high) = (light * how.floor, light * how.ceiling);
        for rgb in [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.227, 0.525, 0.329]] {
            let lightness = Lab::of(&hex(grade(&ramp(), &how, rgb))).lightness;
            assert!(
                lightness >= low - 0.01 && lightness <= high + 0.01,
                "{rgb:?} graded to a lightness of {lightness}, outside {low}..{high}"
            );
        }
    }

    #[test]
    fn dropping_a_colour_and_pulling_it_over_lands_it_on_the_theme() {
        let how = Grade { keep: 0.0, pull: 1.0, floor: 0.0, ceiling: 1.0 };
        let done = grade(&ramp(), &how, [0.227, 0.525, 0.329]);
        let landed = Lab::of(&format!(
            "{:02x}{:02x}{:02x}",
            (done[0] * 255.0).round() as u8,
            (done[1] * 255.0).round() as u8,
            (done[2] * 255.0).round() as u8
        ));
        // Plum leans red and blue, which is a positive a and a negative b.
        assert!(landed.a > 0.0, "a green pulled to plum stayed green: {done:?}");
        assert!(landed.b < 0.0, "a green pulled to plum stayed green: {done:?}");
    }

    /// A ceiling below one is the whole reason the bar can be read over this.
    #[test]
    fn a_lowered_ceiling_takes_the_top_off_the_picture() {
        let how = Grade { keep: 1.0, pull: 0.0, floor: 0.0, ceiling: 0.6 };
        let white = grade(&ramp(), &how, [1.0, 1.0, 1.0]);
        assert!(white.iter().all(|channel| *channel < 0.75), "{white:?} is still white");
    }

    #[test]
    fn a_cube_holds_a_line_for_every_colour_in_the_lattice() {
        let written = cube(&ramp(), &Grade::default(), 5);
        let lines = written.lines().filter(|line| !line.starts_with('#')).count();
        assert_eq!(lines, 1 + 5 * 5 * 5);
    }
}
