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


use console_never::Never;
use console_number_conversion::{fitted, toward_zero_i64, toward_zero_u32};
use std::path::Path;

use console_screen::Screen;

pub const PATCH: f64 = 0.02;

pub struct Picture {
    pub width: u32,
    pub height: u32,
    bands: Vec<u8>,
}

impl Picture {
    pub fn read(path: &Path) -> Result<Self, String> {
        let mut file = std::fs::File::open(path)
            .map_err(|fault| format!("{}: {fault}", path.display()))?;
        let mut surface = cairo::ImageSurface::create_from_png(&mut file)
            .map_err(|fault| format!("{} is not a picture: {fault}", path.display()))?;
        let Ok(width) = fitted::<i32, u32>(surface.width());
        let Ok(height) = fitted::<i32, u32>(surface.height());
        let Ok(stride) = fitted::<i32, usize>(surface.stride());
        let Ok(room) = fitted(width.saturating_mul(height).saturating_mul(3));
        let Ok(rows) = fitted::<u32, usize>(height);
        let Ok(columns) = fitted::<u32, usize>(width);

        let data = surface.data().map_err(|fault| format!("nothing to read: {fault}"))?;
        let mut bands = Vec::with_capacity(room);

        for down in 0..rows {
            for across in 0..columns {
                let at = down.saturating_mul(stride).saturating_add(across.saturating_mul(4));

                match data.get(at..at.saturating_add(3)) {
                    Some([blue, green, red]) => bands.extend([*red, *green, *blue]),
                    Some(_) | None => bands.extend([0, 0, 0]),
                }
            }
        }

        Ok(Picture { width, height, bands })
    }

    fn band(&self, across: u32, down: u32) -> Result<[u8; 3], Never> {
        let Ok(at) =
            fitted::<u32, usize>(down.saturating_mul(self.width).saturating_add(across).saturating_mul(3));

        Ok(match self.bands.get(at..at.saturating_add(3)) {
            Some([red, green, blue]) => [*red, *green, *blue],
            Some(_) | None => [0, 0, 0],
        })
    }

    pub fn at(&self, across: f64, down: f64) -> Result<String, String> {
        let Ok(across) = toward_zero_i64(across);
        let Ok(down) = toward_zero_i64(down);

        let inside = (0..i64::from(self.width)).contains(&across)
            && (0..i64::from(self.height)).contains(&down);

        match inside {
            true => {},
            false => {
                return Err(format!(
                    "{across},{down} is off the edge of a {}x{} picture",
                    self.width, self.height
                ));
            }
        }

        let Ok(column) = fitted(across);
        let Ok(row) = fitted(down);
        let Ok(band) = self.band(column, row);
        let Ok(said) = said(band);

        Ok(said)
    }

    pub fn average(&self, across: f64, down: f64, size: f64) -> Result<String, Never> {
        let Ok(side) = toward_zero_u32(f64::from(self.width) * size);
        let Ok(from_the_left) = toward_zero_u32(f64::from(self.width) * across);
        let Ok(from_the_top) = toward_zero_u32(f64::from(self.height) * down);

        let wide = side.max(1);
        let left = from_the_left
            .saturating_sub(wide.saturating_div(2))
            .min(self.width.saturating_sub(wide));
        let top = from_the_top
            .saturating_sub(wide.saturating_div(2))
            .min(self.height.saturating_sub(wide));
        let mut totals = [0u64; 3];
        let mut seen = 0u64;

        for down in top..top.saturating_add(wide).min(self.height) {
            for across in left..left.saturating_add(wide).min(self.width) {
                let Ok(bands) = self.band(across, down);

                for (total, band) in totals.iter_mut().zip(bands) {
                    *total = total.saturating_add(u64::from(band));
                }

                seen = seen.saturating_add(1);
            }
        }

        let seen = seen.max(1);

        let mut bands = [0u8; 3];

        for (band, total) in bands.iter_mut().zip(totals) {
            let Ok(much) = fitted(total.saturating_div(seen));

            *band = much;
        }

        said(bands)
    }

    pub fn commonest(&self) -> Result<String, Never> {
        let mut seen: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        let every = |size: u32| size.saturating_div(64).max(1);

        let Ok(rows) = fitted(every(self.height));
        let Ok(columns) = fitted(every(self.width));

        for down in (0..self.height).step_by(rows) {
            for across in (0..self.width).step_by(columns) {
                let Ok(band) = self.band(across, down);
                let Ok(said) = said(band);
                let often = seen.entry(said).or_insert(0);

                *often = often.saturating_add(1);
            }
        }

        Ok(seen
            .into_iter()
            .max_by_key(|(_, often)| *often)
            .map(|(colour, _)| colour)
            .unwrap_or_default())
    }
}

fn said([red, green, blue]: [u8; 3]) -> Result<String, Never> {
    Ok(format!("{red:02x}{green:02x}{blue:02x}"))
}

pub fn where_(picture: &Picture, across: f64, down: f64, screen: &Screen) -> Result<String, String> {
    let Ok(logical) = screen.logical();

    let each = f64::from(picture.width) / f64::from(logical.0);
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

    #[test]
    fn a_patch_is_the_average_of_what_is_in_it() {
        let picture = drawn("halves", 100, 100, |context| {
            context.set_source_rgb(0.0, 0.0, 0.0);
            let _ = context.paint();
            context.set_source_rgb(1.0, 1.0, 1.0);
            context.rectangle(0.0, 0.0, 50.0, 100.0);
            let _ = context.fill();
        });
        assert_eq!(picture.average(0.25, 0.5, 0.02), Ok("ffffff".to_string()));
        assert_eq!(picture.average(0.75, 0.5, 0.02), Ok("000000".to_string()));
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
        assert_eq!(
            picture.commonest(),
            Ok("191919".to_string()),
            "the ground, not the square on it"
        );
    }
}
