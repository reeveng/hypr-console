//! Reading a colour out of a screenshot.
//!
//! A screenshot nobody looks at agrees with anything. The wallpaper on the
//! device had not been painting for days: hyprpaper 0.8 changed its config
//! format, the old lines stopped meaning anything, and it did not fail. It
//! started, said the monitor had no target, painted nothing and reported
//! success. What was on screen was the compositor's own default, near enough to
//! a plain dark background that nobody went looking. A service being active
//! proves nothing about whether it is doing its job, and the only thing that
//! would have caught it is looking at the colour of the screen.


use console_number::{fitted, toward_zero_i64, toward_zero_u32};
use std::path::Path;

use console_screen::Screen;

/// How wide a patch is, as a fraction of the picture, when nobody says.
pub const PATCH: f64 = 0.02;

/// A picture, far enough decoded to be asked the colour of somewhere.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Three bands a pixel, in rows.
    bands: Vec<u8>,
}

impl Picture {
    pub fn read(path: &Path) -> Result<Self, String> {
        let mut file = std::fs::File::open(path)
            .map_err(|fault| format!("{}: {fault}", path.display()))?;
        let mut surface = cairo::ImageSurface::create_from_png(&mut file)
            .map_err(|fault| format!("{} is not a picture: {fault}", path.display()))?;
        let (width, height) = (fitted(surface.width()), fitted(surface.height()));
        let stride: usize = fitted(surface.stride());
        let data = surface.data().map_err(|fault| format!("nothing to read: {fault}"))?;
        let mut bands = Vec::with_capacity(fitted(width * height * 3));

        for down in 0..fitted::<u32, usize>(height) {
            for across in 0..fitted::<u32, usize>(width) {
                // Cairo holds a pixel as one machine word, so the bands come
                // out in the order the machine counts in rather than the order
                // they are written.
                let at = down * stride + across * 4;
                bands.extend([data[at + 2], data[at + 1], data[at]]);
            }
        }

        Ok(Picture { width, height, bands })
    }

    fn band(&self, across: u32, down: u32) -> [u8; 3] {
        let at: usize = fitted((down * self.width + across) * 3);
        [self.bands[at], self.bands[at + 1], self.bands[at + 2]]
    }

    /// The colour of one place, as it would be written in a stylesheet.
    pub fn at(&self, across: f64, down: f64) -> Result<String, String> {
        let (across, down) = (toward_zero_i64(across), toward_zero_i64(down));
        let inside = (0..i64::from(self.width)).contains(&across)
            && (0..i64::from(self.height)).contains(&down);

        if !inside {
            return Err(format!(
                "{across},{down} is off the edge of a {}x{} picture",
                self.width, self.height
            ));
        }

        Ok(said(self.band(fitted(across), fitted(down))))
    }

    /// The average colour of a small patch, placed by fraction not pixel.
    ///
    /// A fraction because the thing being compared was measured on a picture of
    /// another size, and an average because a petal that strayed into the patch
    /// should move the answer by less than the encoder does.
    pub fn average(&self, across: f64, down: f64, size: f64) -> String {
        let wide = toward_zero_u32(f64::from(self.width) * size).max(1);
        let left = toward_zero_u32(f64::from(self.width) * across)
            .saturating_sub(wide / 2)
            .min(self.width.saturating_sub(wide));
        let top = toward_zero_u32(f64::from(self.height) * down)
            .saturating_sub(wide / 2)
            .min(self.height.saturating_sub(wide));
        let mut totals = [0u64; 3];
        let mut seen = 0u64;

        for down in top..(top + wide).min(self.height) {
            for across in left..(left + wide).min(self.width) {
                let bands = self.band(across, down);

                for (total, band) in totals.iter_mut().zip(bands) {
                    *total += u64::from(band);
                }

                seen += 1;
            }
        }

        let seen = seen.max(1);
        said(totals.map(|total| fitted(total / seen)))
    }

    /// The colour most of the screen is, which is usually the background.
    pub fn commonest(&self) -> String {
        let mut seen: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        let every = |size: u32| (size / 64).max(1);

        for down in (0..self.height).step_by(fitted(every(self.height))) {
            for across in (0..self.width).step_by(fitted(every(self.width))) {
                *seen.entry(said(self.band(across, down))).or_insert(0) += 1;
            }
        }

        seen.into_iter().max_by_key(|(_, often)| *often).map(|(colour, _)| colour).unwrap_or_default()
    }
}

fn said(bands: [u8; 3]) -> String {
    format!("{:02x}{:02x}{:02x}", bands[0], bands[1], bands[2])
}

/// A place in the desktop's own layout, found on a picture of its pixels.
///
/// A check says where something is the way the compositor says it: in the size
/// the desktop is laid out in. A picture is the screen's own pixels, because
/// that is what the device draws and what a fault in drawing shows up in. This
/// is the one place that knows the difference between the two.
pub fn where_(picture: &Picture, across: f64, down: f64, screen: &Screen) -> Result<String, String> {
    let each = f64::from(picture.width) / f64::from(screen.logical().0);
    picture.at(across * each, down * each)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawn(named: &str, width: i32, height: i32, paint: impl Fn(&cairo::Context)) -> Picture {
        let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, width, height)
            .expect("a surface");
        let context = cairo::Context::new(&surface).expect("a context");
        paint(&context);
        drop(context);
        let at = std::env::temp_dir().join(format!("console-picture-{named}.png"));
        let mut file = std::fs::File::create(&at).expect("somewhere to write");
        surface.write_to_png(&mut file).expect("a png");
        drop(file);
        let picture = Picture::read(&at).expect("a picture");
        let _ = std::fs::remove_file(&at);
        picture
    }

    fn plain(named: &str, width: i32, height: i32, colour: (f64, f64, f64)) -> Picture {
        drawn(named, width, height, |context| {
            context.set_source_rgb(colour.0, colour.1, colour.2);
            let _ = context.paint();
        })
    }

    #[test]
    fn a_colour_is_read_as_a_stylesheet_would_write_it() {
        let picture = plain("one-colour", 8, 8, (1.0, 0.0, 0.5));
        assert_eq!(picture.at(0.0, 0.0), Ok("ff0080".to_string()));
    }

    #[test]
    fn somewhere_off_the_edge_is_said_rather_than_answered() {
        let picture = plain("off-the-edge", 8, 8, (0.0, 0.0, 0.0));
        assert!(picture.at(8.0, 0.0).is_err());
        assert!(picture.at(-1.0, 0.0).is_err());
    }

    /// A petal that strayed into the patch should move the answer by less than
    /// the encoder does.
    #[test]
    fn a_patch_is_the_average_of_what_is_in_it() {
        let picture = drawn("halves", 100, 100, |context| {
            context.set_source_rgb(0.0, 0.0, 0.0);
            let _ = context.paint();
            context.set_source_rgb(1.0, 1.0, 1.0);
            context.rectangle(0.0, 0.0, 50.0, 100.0);
            let _ = context.fill();
        });
        assert_eq!(picture.average(0.25, 0.5, 0.02), "ffffff");
        assert_eq!(picture.average(0.75, 0.5, 0.02), "000000");
    }

    #[test]
    fn the_commonest_colour_is_what_most_of_the_screen_is() {
        let picture = drawn("a-square", 100, 100, |context| {
            context.set_source_rgb(0.1, 0.1, 0.1);
            let _ = context.paint();
            context.set_source_rgb(1.0, 1.0, 1.0);
            context.rectangle(0.0, 0.0, 20.0, 20.0);
            let _ = context.fill();
        });
        assert_eq!(picture.commonest(), "191919", "the ground, not the square on it");
    }
}
