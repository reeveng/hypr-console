//! How big everything on the screen is.
//!
//! The panel is 2560 by 1600 and the desktop is laid out at two and a half
//! times the density it is drawn at, so a window sees 1024 by 640. That number
//! is the size of everything: the rows of this panel, the words in a browser,
//! how much of a folder fits on the screen at once. It was a constant in the
//! compositor's file and nothing on the machine could reach it.
//!
//! ## The rungs are the numbers that divide
//!
//! A density is not a free number here. The compositor lays the desktop out in
//! whole logical pixels, and a scale that leaves a fraction is one it warns
//! about and then rounds off on its own -- so the offered sizes are the ones
//! where both sides of the panel come out whole. 2560 and 1600 share 320, so
//! the ladder is 320 over a whole number, and the rungs around the middle are
//! about a quarter apart: far enough that changing rung is a change somebody
//! meant to make.
//!
//! The bottom rung is the odd one, and it is here on purpose. 1.0 is the panel
//! at its own pixels -- 2560 by 1600 on eight and a half inches, which is about
//! a third the size everything in this repository was drawn to be read and hit
//! at. It is below what this device is designed for and it is not below what
//! somebody else's eyes, or somebody else's use of this desktop, might want. So
//! it is offered rather than left out because the machine it was written on
//! does not want it. What it costs is said here and in `docs/screen.md`, and
//! not in the row: a list whose ends argue with themselves is a list nobody
//! reads to the bottom of.
//!
//! Five words and no numbers. "2.0" is a number about a compositor; what a
//! person is choosing is how big things are, and the plainest ladder for that
//! is the one anybody would say out loud. Tiny, Smaller, Normal, Bigger, Huge
//! -- two either side of the size this device is set up as.
//!
//! ## A panel that is not this one
//!
//! The rungs are numbers about a screen that is 2560 by 1600. Put this desktop
//! on a panel that does not share 320 and some of them stop dividing it, which
//! is a size somebody chooses and does not get.
//! `the_offered_sizes_divide_the_panel_into_whole_pixels` reads the compositor's
//! own declaration and fails if that ever happens, so a fork that changes the
//! screen is told to change the ladder rather than finding out on the device.
//!
//! ## Said to the compositor, not written into its file
//!
//! The compositor's file is this repository's, byte for byte -- `console check`
//! reports it as drift the moment anything on the machine edits it. So the file
//! goes on declaring the size this device is set up as, and a machine standing
//! somewhere else says so in `~/.config/console/scale`, which is nobody's to
//! check. Same shape as [`crate::warm`], and for the same reason.
//!
//! The live change is `hyprctl eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."*, which is the same trap
//! `docs/screen.md` describes for `dispatch`: the obvious command every example
//! on the internet gives comes back with a complaint nothing here would have
//! seen, and the only symptom is a setting that does nothing.

use console_never::Never;
use console_screen::Screen;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    Tiny,
    Smaller,
    Normal,
    Bigger,
    Huge,
}

pub const EVERY: [Size; 5] =
    [Size::Tiny, Size::Smaller, Size::Normal, Size::Bigger, Size::Huge];

pub const SHARED: u32 = 320;

impl Size {
    pub fn scale(self) -> Result<f64, Never> {
        Ok(match self {
            Size::Tiny => 1.0,
            Size::Smaller => 2.0,
            Size::Normal => 2.5,
            Size::Bigger => 3.2,
            Size::Huge => 4.0,
        })
    }

    pub fn written(self) -> Result<&'static str, Never> {
        Ok(match self {
            Size::Tiny => "tiny",
            Size::Smaller => "smaller",
            Size::Normal => "normal",
            Size::Bigger => "bigger",
            Size::Huge => "huge",
        })
    }

    pub fn of(said: &str) -> Result<Option<Self>, Never> {
        Ok(EVERY.into_iter().find(|size| {
            let Ok(written) = size.written();

            written == said.trim()
        }))
    }
}

pub const UNDER: &str = ".config/console/scale";

pub fn at(home: &str) -> Result<std::path::PathBuf, Never> {
    Ok(std::path::Path::new(home).join(UNDER))
}

pub fn standing(said: &str) -> Result<Option<Size>, Never> {
    let Ok(now) = scale_of(said);

    let Some(now) = now else { return Ok(None) };

    Ok(EVERY.into_iter().find(|size| {
        let Ok(scale) = size.scale();

        (scale - now).abs() < f64::EPSILON
    }))
}

pub fn scale_of(said: &str) -> Result<Option<f64>, Never> {
    let Some(at) = said.find("\"scale\"") else { return Ok(None) };

    let Some(from) = said.get(at..) else { return Ok(None) };

    let Some(split) = from.split_once(':') else { return Ok(None) };

    let rest = split.1;
    let number: String =
        rest.trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();

    let Ok(scale) = number.parse::<f64>() else { return Ok(None) };

    Ok(Some(scale))
}

pub fn lua(screen: &Screen, scale: f64) -> Result<String, Never> {
    let (wide, tall) = screen.mode;

    Ok(format!(
        r#"hl.monitor({{ output = "{}", mode = "{wide}x{tall}@{}", position = "auto", scale = {scale}, transform = {} }})"#,
        OUTPUT, screen.refresh, screen.transform
    ))
}

pub const OUTPUT: &str = "eDP-1";

pub const BAR_UNDER: &str = ".config/console/bar.css";

pub fn bar_at(home: &str) -> Result<std::path::PathBuf, Never> {
    Ok(std::path::Path::new(home).join(BAR_UNDER))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(size: Size) -> f64 {
        let Ok(scale) = size.scale();

        scale
    }

    fn written(size: Size) -> &'static str {
        let Ok(written) = size.written();

        written
    }

    fn of(said: &str) -> Option<Size> {
        let Ok(size) = Size::of(said);

        size
    }

    fn standing(said: &str) -> Option<Size> {
        let Ok(size) = super::standing(said);

        size
    }

    fn lua(screen: &Screen, at: f64) -> String {
        let Ok(said) = super::lua(screen, at);

        said
    }

    fn pixels(screen: &Screen) -> (u32, u32) {
        let Ok(pixels) = screen.pixels();

        pixels
    }

    fn screen() -> Screen {
        console_screen::declared().expect("the compositor's file declares a screen")
    }

    #[test]
    fn the_offered_sizes_divide_the_panel_into_whole_pixels() {
        let screen = screen();
        let (wide, tall) = pixels(&screen);
        for size in EVERY {
            for side in [wide, tall] {
                let logical = f64::from(side) / scale(size);
                assert_eq!(
                    logical.fract(),
                    0.0,
                    "{} leaves {side} at {logical}, which is not a whole number of pixels",
                    written(size)
                );
            }
            assert_eq!(
                (f64::from(SHARED) / scale(size)).fract(),
                0.0,
                "{} is not {SHARED} over a whole number",
                written(size)
            );
        }
    }

    #[test]
    fn normal_is_the_size_this_device_is_set_up_as() {
        assert_eq!(scale(Size::Normal), screen().scale);
    }

    #[test]
    fn the_ladder_climbs_and_every_step_is_one_anybody_would_see() {
        for pair in EVERY.windows(2) {
            let (below, above) = (scale(pair[0]), scale(pair[1]));
            assert!(below < above, "{:?} is not below {:?}", pair[0], pair[1]);
            assert!(above / below > 1.2, "{below} and {above} are the same size to an eye");
        }
    }

    #[test]
    fn the_bottom_of_the_ladder_is_the_panel_at_its_own_pixels() {
        let screen = screen();
        assert_eq!(EVERY[0], Size::Tiny);
        let Ok(logical) = screen.logical_at(scale(Size::Tiny));

        assert_eq!(logical, pixels(&screen));
        assert!(EVERY.into_iter().all(|size| scale(size) >= 1.0), "a rung below the panel");
    }

    #[test]
    fn the_rung_being_stood_on_is_read_out_of_what_the_compositor_says() {
        let said = r#"[{"name": "eDP-1", "width": 1600, "scale": 2.5, "transform": 1}]"#;
        assert_eq!(standing(said), Some(Size::Normal));
        assert_eq!(standing(&said.replace("2.5", "3.2")), Some(Size::Bigger));
    }

    #[test]
    fn a_density_that_is_none_of_the_three_marks_none_of_them() {
        assert_eq!(standing(r#"[{"scale": 1.75}]"#), None);
        assert_eq!(standing("hyprctl said nothing at all"), None);
    }

    #[test]
    fn the_compositor_is_handed_a_whole_screen_and_not_just_a_number() {
        let said = lua(&screen(), scale(Size::Bigger));
        assert!(said.contains("transform = 1"), "{said}");
        assert!(said.contains("1600x2560@144"), "{said}");
        assert!(said.contains("scale = 3.2"), "{said}");
        assert!(said.contains(OUTPUT), "{said}");
    }

    #[test]
    fn the_answer_is_written_down_as_the_rung_it_names() {
        for size in EVERY {
            assert_eq!(of(written(size)), Some(size));
        }
        assert_eq!(of("tiny\n"), Some(Size::Tiny));
        assert_eq!(of("2.0"), None);
    }

}
