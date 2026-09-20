//! How big everything on the screen is.
//!
//! A rung is how much of the desktop a window sees, and not a density. That is
//! the whole of what changed here, and it is what lets this desktop be put on a
//! panel nobody here owns.
//!
//! ## A rung is a canvas, and the density falls out of the panel
//!
//! The five rungs were five densities -- 1.0, 2.0, 2.5, 3.2, 4.0 -- and every
//! one of them was a number about a screen that is 2560 by 1600. 2560 and 1600
//! share 320, the ladder was 320 over a whole number, and a panel that does not
//! share 320 takes some of those rungs and leaves a fraction the compositor
//! warns about and then rounds off on its own: a size somebody chooses and does
//! not get.
//!
//! So a rung says how many points across the panel is cut into, and the scale
//! is that divided into the panel's own width. On this device the five come out
//! at exactly the numbers they always were. On a 1920 by 1200 laptop they come
//! out somewhere else and mean the same thing, which is the only way five words
//! can be offered on a machine this repository has never seen.
//!
//! [`console_screen::DRAWN_AT`] is Normal, and it is the one number here that is
//! about this desktop rather than about a screen: every surface was drawn and
//! measured at 1024 across. The rungs either side are a quarter apart or so --
//! far enough that changing rung is a change somebody meant to make.
//!
//! The bottom rung is the odd one, and it is here on purpose. [`Across::Pixels`]
//! is the panel at its own pixels, whatever that comes to, which on this device
//! is about a third the size everything in this repository was drawn to be read
//! and hit at. It is below what this device is designed for and it is not below
//! what somebody else's eyes, or somebody else's use of this desktop, might
//! want. So it is offered rather than left out because the machine it was
//! written on does not want it. What it costs is said here and not in the row: a
//! list whose ends argue with themselves is a list nobody reads to the bottom
//! of.
//!
//! Five words and no numbers. "2.0" is a number about a compositor; what a
//! person is choosing is how big things are, and the plainest ladder for that is
//! the one anybody would say out loud.
//!
//! ## A panel that cannot wear a rung says so
//!
//! A canvas wider than the panel is a density below one, which is the
//! compositor drawing less than the screen can show. [`Across::fits`] is what
//! answers that, rather than a rung that is offered and does nothing.
//!
//! ## Said to the compositor, not written into its file
//!
//! The compositor's file is this repository's, byte for byte -- `console check`
//! reports it as drift the moment anything on the machine edits it. So the file
//! goes on declaring the size this device is set up as, and a machine standing
//! somewhere else says so under `~/.config/console/screens`, which is nobody's
//! to check. Same shape as [`crate::warm`], and for the same reason.
//!
//! It is a rung per screen rather than a rung for the machine, and
//! [`crate::screens`] is the argument: a rung is a canvas divided into the
//! panel's own width, so the same word is a different density on every screen
//! it is said about, and the one thing it cannot be is a number the machine
//! holds once.
//!
//! It is also a rung per shape the screen stands in, for the second half of the
//! same sentence. Which of a panel's two sides is its width is what a turn
//! changes, so **Normal** on this handheld held landscape is 1024 points across
//! 2560 pixels and **Normal** stood on its end is 1024 across 1600: the same
//! word, and everything on the screen a third larger. A person who turns the
//! device and then picks a size is not correcting the size they chose for the
//! other way up, they are saying what this way up should be -- so the two are
//! remembered apart, and turning back finds the rung that was chosen there.
//! Two shapes rather than four quarters, because the half turn is the same
//! width as the quarter it is opposite and a rung is only ever about the
//! width.
//!
//! ## The touchscreen turns with it or a finger lands turned
//!
//! A touch panel reports in its own orientation, so the compositor is told
//! which quarter to read it through, and that quarter is the screen's. It was
//! a number in the compositor's file -- right for the one way up this device
//! was ever set up, and left behind the moment the screen could be turned: a
//! desktop standing at a quarter with its touches still read at the mounting
//! is one where every panel answers a press somebody did not make. So the
//! touch device is part of describing the screen and goes in the same `eval`,
//! for the same reason the density does.
//!
//! The live change is `hyprctl eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."*, which is the same trap
//! `docs/screen.md` describes for `dispatch`: the obvious command every example
//! on the internet gives comes back with a complaint nothing here would have
//! seen, and the only symptom is a setting that does nothing.

use console_core_never::Never;
use console_core_words::Words;
use console_screen::{Canvas, DRAWN_AT, Fits, Screen, Shape};

use crate::screens::{Output, Unnamed};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum Size {
    #[words(written = "tiny")]
    Tiny,
    #[words(written = "smaller")]
    Smaller,
    #[words(written = "normal")]
    Normal,
    #[words(written = "bigger")]
    Bigger,
    #[words(written = "huge")]
    Huge,
}

pub const EVERY: [Size; 5] =
    [Size::Tiny, Size::Smaller, Size::Normal, Size::Bigger, Size::Huge];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Across {
    Pixels,
    Points(Canvas),
}

impl Across {
    pub fn scale_on(self, screen: &Screen) -> Result<f64, Never> {
        Ok(match self {
            Across::Pixels => 1.0,
            Across::Points(canvas) => {
                let Ok(scale) = canvas.scale_on(screen);

                scale
            }
        })
    }

    pub fn fits(self, screen: &Screen) -> Result<Fits, Never> {
        match self {
            Across::Pixels => Ok(Fits::OnThePanel),
            Across::Points(canvas) => canvas.fits(screen),
        }
    }
}

impl Size {
    pub fn across(self) -> Result<Across, Never> {
        Ok(match self {
            Size::Tiny => Across::Pixels,
            Size::Smaller => Across::Points(Canvas(1280)),
            Size::Normal => Across::Points(DRAWN_AT),
            Size::Bigger => Across::Points(Canvas(800)),
            Size::Huge => Across::Points(Canvas(640)),
        })
    }

    pub fn scale_on(self, screen: &Screen) -> Result<f64, Never> {
        let Ok(across) = self.across();

        across.scale_on(screen)
    }

    pub fn of(said: &str) -> Result<Option<Self>, Never> {
        Ok(EVERY.into_iter().find(|size| {
            let Ok(written) = size.written();

            written == said.trim()
        }))
    }
}

pub const NAMED: &str = "scale";

pub fn shaped(shape: Shape) -> Result<&'static str, Never> {
    Ok(match shape {
        Shape::Wider => "wider",
        Shape::Taller => "taller",
    })
}

pub fn at(
    home: &std::path::Path,
    panel: Output<'_>,
    shape: Shape,
) -> Result<std::path::PathBuf, Unnamed> {
    let Ok(shaped) = shaped(shape);

    panel.keeping(home, &format!("{NAMED}-{shaped}"))
}

pub fn standing(monitors: &serde_json::Value) -> Result<Option<Size>, Never> {
    let Ok(shown) = console_screen::shown(monitors);

    let screen = match shown {
        Some(screen) => screen,
        None => return Ok(None),
    };

    Ok(EVERY.into_iter().find(|size| {
        let Ok(scale) = size.scale_on(&screen);

        (scale - screen.scale).abs() < f64::EPSILON
    }))
}

pub fn lua(panel: Output<'_>, screen: &Screen, scale: f64) -> Result<String, Never> {
    let (wide, tall) = (screen.mode.wide, screen.mode.tall);
    let named = panel.0;
    let transform = screen.transform;

    Ok(format!(
        r#"hl.monitor({{ output = "{named}", mode = "{wide}x{tall}@{}", position = "auto", scale = {scale}, transform = {transform} }}) hl.config({{ input = {{ touchdevice = {{ output = "{named}", transform = {transform} }} }} }})"#,
        screen.refresh
    ))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn scale(size: Size) -> f64 {
        let Ok(scale) = size.scale_on(&screen());

        scale
    }

    fn scale_on(size: Size, screen: &Screen) -> f64 {
        let Ok(scale) = size.scale_on(screen);

        scale
    }

    fn laptop() -> Screen {
        Screen {
            mode: console_core_geometry::Size { wide: 1920, tall: 1200 },
            refresh: 60,
            scale: 1.0,
            transform: 0,
        }
    }

    fn written(size: Size) -> &'static str {
        let Ok(written) = size.written();

        written
    }

    fn of(said: &str) -> Option<Size> {
        let Ok(size) = Size::of(said);

        size
    }

    fn said(text: &str) -> serde_json::Value {
        console_compositor::read(text).expect("what hyprctl said")
    }

    fn standing(text: &str) -> Option<Size> {
        let Ok(size) = super::standing(&said(text));

        size
    }

    fn lua(screen: &Screen, at: f64) -> String {
        let Ok(said) = super::lua(Output("eDP-1"), screen, at);

        said
    }

    fn pixels(screen: &Screen) -> console_core_geometry::Size<u32> {
        let Ok(pixels) = screen.pixels();

        pixels
    }

    fn screen() -> Screen {
        console_screen::declared().expect("the compositor's file declares a screen")
    }

    #[test]
    fn the_offered_sizes_divide_the_panel_into_whole_pixels() {
        let screen = screen();
        let held = pixels(&screen);
        let (wide, tall) = (held.wide, held.tall);
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
        }
    }

    #[test]
    fn every_rung_divides_a_laptops_panel_into_whole_pixels_too() {
        let laptop = laptop();
        let Ok(held) = laptop.pixels();

        for size in EVERY {
            for side in [held.wide, held.tall] {
                let logical = f64::from(side) / scale_on(size, &laptop);

                assert_eq!(
                    logical.fract(),
                    0.0,
                    "{} leaves {side} at {logical} on a panel this repository has never seen",
                    written(size)
                );
            }
        }
    }

    #[test]
    fn the_rungs_are_the_numbers_they_always_were_on_this_device() {
        let every: Vec<f64> = EVERY.into_iter().map(scale).collect();

        assert_eq!(every, vec![1.0, 2.0, 2.5, 3.2, 4.0], "a canvas said as a density");
    }

    #[test]
    fn the_same_five_words_are_five_other_densities_on_a_laptop() {
        let laptop = laptop();
        let every: Vec<f64> = EVERY.into_iter().map(|size| scale_on(size, &laptop)).collect();

        assert_eq!(every, vec![1.0, 1.5, 1.875, 2.4, 3.0]);
    }

    #[test]
    fn a_rung_wider_than_the_panel_is_one_the_panel_cannot_wear() {
        let small = Screen {
            mode: console_core_geometry::Size { wide: 1280, tall: 800 },
            refresh: 60,
            scale: 1.0,
            transform: 0,
        };

        let Ok(smaller) = Size::Smaller.across();
        let Ok(normal) = Size::Normal.across();

        assert_eq!(smaller.fits(&small), Ok(Fits::OnThePanel), "1280 across on a 1280 panel");
        assert_eq!(normal.fits(&small), Ok(Fits::OnThePanel));

        let Ok(tiny) = Size::Tiny.across();

        assert_eq!(tiny.fits(&small), Ok(Fits::OnThePanel), "the panel's own pixels always fit");
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
        let said = r#"[{"name": "eDP-1", "width": 1600, "height": 2560, "refreshRate": 144.0,
            "scale": 2.5, "transform": 1}]"#;

        assert_eq!(standing(said), Some(Size::Normal));
        assert_eq!(standing(&said.replace("2.5", "3.2")), Some(Size::Bigger));
    }

    #[test]
    fn the_same_density_is_a_different_rung_on_a_different_panel() {
        let said = r#"[{"name": "eDP-1", "width": 1920, "height": 1200, "refreshRate": 60.0,
            "scale": 1.5, "transform": 0}]"#;

        assert_eq!(standing(said), Some(Size::Smaller), "1280 across on a laptop's panel");
        assert_eq!(standing(&said.replace("1.5", "2.5")), None, "the device's rung, elsewhere");
    }

    #[test]
    fn a_density_that_is_none_of_the_five_marks_none_of_them() {
        let said = r#"[{"name": "eDP-1", "width": 1600, "height": 2560, "refreshRate": 144.0,
            "scale": 1.75, "transform": 1}]"#;

        assert_eq!(standing(said), None);
    }

    #[test]
    fn a_screen_whose_panel_is_half_said_stands_on_no_rung_at_all() {
        assert_eq!(standing(r#"{"eDP-1": {"levels": {}}}"#), None);
        assert_eq!(standing(r#"[{"width": 1600, "scale": 2.5}]"#), None);
        assert_eq!(
            standing(r#"[{"name": "eDP-1", "scale": 2.5}]"#),
            None,
            "a density is not a rung until the panel it is on is known"
        );
    }

    #[test]
    fn the_compositor_is_handed_a_whole_screen_and_not_just_a_number() {
        let said = lua(&screen(), scale(Size::Bigger));
        assert!(said.contains("transform = 1"), "{said}");
        assert!(said.contains("1600x2560@144"), "{said}");
        assert!(said.contains("scale = 3.2"), "{said}");
        assert!(said.contains("eDP-1"), "{said}");
    }

    #[test]
    fn the_touchscreen_is_read_through_the_quarter_the_picture_is_drawn_at() {
        for transform in 0..4 {
            let screen = Screen { transform, ..screen() };
            let said = lua(&screen, 2.5);
            let both = said.matches(&format!("transform = {transform}")).count();

            assert!(said.contains("touchdevice"), "the finger was not told anything: {said}");
            assert_eq!(
                both, 2,
                "the screen and the finger on it are at two different quarters: {said}"
            );
        }
    }

    #[test]
    fn the_two_shapes_a_screen_stands_in_are_two_rungs_and_not_one() {
        let home = std::path::Path::new("/home/ada");
        let wider = at(home, Output("eDP-1"), Shape::Wider);
        let taller = at(home, Output("eDP-1"), Shape::Taller);

        assert_eq!(
            wider,
            Ok(std::path::PathBuf::from("/home/ada/.config/console/screens/eDP-1/scale-wider"))
        );
        assert_ne!(wider, taller, "a turn would have found the other shape's rung");
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
