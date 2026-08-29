//! The device's screen, read out of the compositor's own file.
//!
//! The panel is mounted portrait and turned a quarter, and the desktop is laid
//! out at two and a half times the density everything is drawn at. Three
//! numbers, and they were written down in four places: the tool that nests the
//! desktop, the one that draws the wallpaper, a test, and a comment. This is
//! the one place, and it reads them from the file the device itself reads, so
//! a screen that changes changes them all.
//!
//! A test environment that is not the shape, the size or the density of the
//! thing it stands in for is a test environment that agrees with you. The
//! wallpaper was drawn portrait into a landscape screen for its whole life and
//! nothing said so.

/// What the compositor is told the screen is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub mode: (u32, u32),
    pub refresh: u32,
    pub scale: f64,
    pub transform: u32,
}

/// Where the compositor's declaration lives, under the source tree.
pub const CONFIG: &str = "files/home/@user@/.config/hypr/hyprland.lua";

impl Screen {
    /// Read from the compositor's declaration.
    ///
    /// Takes the text rather than a path, so that what is parsed can be tested
    /// without a file and so that the caller decides which tree it is reading.
    pub fn read(lua: &str) -> Result<Self, String> {
        let block = between(lua, "hl.monitor", '{', '}')
            .ok_or("the compositor's file declares no screen")?;
        let mode = after(&block, "mode").ok_or("the screen says nothing about its mode")?;
        let (wide, rest) = mode.split_once('x').ok_or("that mode is not WIDTHxHEIGHT")?;
        let (tall, refresh) = rest.split_once('@').ok_or("that mode names no refresh")?;
        Ok(Screen {
            mode: (number(wide)?, number(tall)?),
            refresh: number(refresh)?,
            scale: after(&block, "scale")
                .ok_or("the screen says nothing about its scale")?
                .parse()
                .map_err(|_| "that scale is not a number".to_string())?,
            transform: number(&after(&block, "transform").ok_or("the screen says nothing about its transform")?)?,
        })
    }

    /// Whether the compositor rotates it a quarter or three quarters.
    pub fn turned(&self) -> bool {
        self.transform % 2 == 1
    }

    /// The size of a picture of it, which is the mode after the turn.
    pub fn pixels(&self) -> (u32, u32) {
        match self.turned() {
            true => (self.mode.1, self.mode.0),
            false => self.mode,
        }
    }

    /// The size the desktop is laid out in, which is what a window sees.
    pub fn logical(&self) -> (u32, u32) {
        let (wide, tall) = self.pixels();
        (
            (wide as f64 / self.scale).round() as u32,
            (tall as f64 / self.scale).round() as u32,
        )
    }

    /// The same screen, made small enough to look at on one this size.
    ///
    /// Only the density is given up, and only as far as it has to be: the
    /// desktop is still laid out in the same logical size, so everything is
    /// still where it is on the device and only the pixels are fewer. Nothing
    /// is given up at all when it already fits.
    pub fn cut_to(&self, room: (u32, u32)) -> f64 {
        let (wide, tall) = self.pixels();
        let fits = (room.0 as f64 / wide as f64)
            .min(room.1 as f64 / tall as f64)
            .min(1.0);
        self.scale * fits
    }
}

/// The body of the first `name ... open ... close` in the text.
fn between(text: &str, name: &str, open: char, close: char) -> Option<String> {
    let at = text.find(name)? + name.len();
    let start = text[at..].find(open)? + at + 1;
    let end = text[start..].find(close)? + start;
    Some(text[start..end].to_string())
}

/// What `name = ...` says, unquoted.
fn after(block: &str, name: &str) -> Option<String> {
    let at = block.find(name)? + name.len();
    let rest = block[at..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let said: String = match rest.strip_prefix('"') {
        Some(quoted) => quoted.chars().take_while(|c| *c != '"').collect(),
        None => rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect(),
    };
    match said.is_empty() {
        true => None,
        false => Some(said),
    }
}

fn number(said: &str) -> Result<u32, String> {
    said.trim().parse().map_err(|_| format!("{said:?} is not a whole number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECLARED: &str = include_str!("../../../files/home/@user@/.config/hypr/hyprland.lua");

    #[test]
    fn the_screen_this_device_has_is_read() {
        let screen = Screen::read(DECLARED).expect("the compositor declares a screen");
        assert_eq!(screen.pixels(), (2560, 1600));
        assert!(screen.turned(), "the panel is mounted portrait and turned");
    }

    #[test]
    fn a_picture_of_a_turned_screen_is_the_mode_the_other_way_round() {
        let portrait = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(portrait.pixels(), (2560, 1600));
        let upright = Screen { transform: 0, ..portrait };
        assert_eq!(upright.pixels(), (1600, 2560));
    }

    #[test]
    fn the_desktop_is_laid_out_at_the_density_it_was_told() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.logical(), (1024, 640));
    }

    #[test]
    fn cutting_to_a_screen_it_already_fits_on_gives_up_nothing() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.cut_to((3840, 2160)), 2.5);
    }

    #[test]
    fn cutting_to_a_smaller_screen_gives_up_only_the_density() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        // Half the pixels across, so half the density, and the desktop is laid
        // out in the same logical size it always was.
        assert_eq!(screen.cut_to((1280, 1600)), 1.25);
    }

    #[test]
    fn a_file_that_declares_no_screen_says_so_rather_than_guessing() {
        let fault = Screen::read("-- nothing here\n").expect_err("no monitor");
        assert!(fault.contains("no screen"), "{fault}");
    }

    #[test]
    fn a_screen_missing_a_number_names_the_number() {
        let fault = Screen::read("hl.monitor({ mode = \"1600x2560@144\", scale = 2.5 })")
            .expect_err("no transform");
        assert!(fault.contains("transform"), "{fault}");
    }
}
