//! The screen this desktop is standing on, asked of the machine and read out of
//! the compositor's file when there is no machine to ask.
//!
//! Three numbers -- the mode, the density and the quarter turn -- were written
//! down in four places: the tool that nests the desktop, the one that draws the
//! wallpaper, a test, and a comment. This is the one place they are spelled.
//!
//! They were read out of `hyprland.lua` alone, which is a file in this
//! repository, which made the panel a fact about the tree rather than about the
//! machine. That is right for exactly one machine. So [`shown`] is the screen
//! the compositor says it is actually driving -- the same reading
//! `console-test-stages` had written for itself against the device over ssh --
//! and [`declared`] is the tree's own answer, which is what a laptop building a
//! picture for a screen it cannot see still has to stand on.
//!
//! How dense the desktop is drawn is not one of the three either, and it is the
//! other half of installing this anywhere. A density is meaningless on its own:
//! what a person is choosing is how much of the desktop a window sees, and
//! [`Canvas`] is that -- the points across that the panel is cut into, with the
//! scale falling out of the panel's own width. [`DRAWN_AT`] is the canvas every
//! surface here was drawn and measured at, so it is the one number in this that
//! is about the desktop rather than about a screen.
//!
//! What a panel is mounted like is not one of the three. It is derived:
//! [`Mounted`] says a panel whose native mode is taller than it is wide is one
//! somebody screwed in sideways, which is what the Legion Go's 1600x2560 eDP is
//! and what no laptop's is. A desktop that has to be told that in a file per
//! machine is a desktop nobody can install on a machine nobody here owns.
//!
//! A test environment that is not the shape, the size or the density of the
//! thing it stands in for is a test environment that agrees with you. The
//! wallpaper was drawn portrait into a landscape screen for its whole life and
//! nothing said so.


pub mod kernel;

use std::fmt;

use console_compositor::Monitor;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, whole_u32};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub mode: Size<u32>,
    pub refresh: u32,
    pub scale: f64,
    pub transform: u32,
}

pub const CONFIG: &str = "files/home/@user@/.config/console/hypr/hyprland.lua";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undeclared {
    NoScreen,
    NoMode,
    ModeShape,
    NoRefresh,
    NoScale,
    ScaleNotANumber,
    NoTransform,
    NotAWholeNumber(String),
}

impl fmt::Display for Undeclared {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Undeclared::NoScreen => write!(to, "the compositor's file declares no screen"),
            Undeclared::NoMode => write!(to, "the screen says nothing about its mode"),
            Undeclared::ModeShape => write!(to, "that mode is not WIDTHxHEIGHT"),
            Undeclared::NoRefresh => write!(to, "that mode names no refresh"),
            Undeclared::NoScale => write!(to, "the screen says nothing about its scale"),
            Undeclared::ScaleNotANumber => write!(to, "that scale is not a number"),
            Undeclared::NoTransform => write!(to, "the screen says nothing about its transform"),
            Undeclared::NotAWholeNumber(said) => write!(to, "{said:?} is not a whole number"),
        }
    }
}

impl std::error::Error for Undeclared {}

pub fn declared() -> Result<Screen, Undeclared> {
    Screen::read(DECLARED)
}

const DECLARED: &str = include_str!("../../../files/home/@user@/.config/console/hypr/hyprland.lua");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turned {
    Sideways,
    Upright,
}

impl Screen {
    pub fn read(lua: &str) -> Result<Self, Undeclared> {
        let Ok(found) = between(lua, Named("hl.monitor"), Wrapped { open: '{', close: '}' });

        let block = found.ok_or(Undeclared::NoScreen)?;

        let Ok(said) = after(&block, Named("mode"));

        let mode = said.ok_or(Undeclared::NoMode)?;
        let (wide, rest) = mode.split_once('x').ok_or(Undeclared::ModeShape)?;
        let (tall, refresh) = rest.split_once('@').ok_or(Undeclared::NoRefresh)?;
        let wide = number(wide)?;
        let tall = number(tall)?;
        let refresh = number(refresh)?;
        let Ok(found) = after(&block, Named("scale"));

        let said = found.ok_or(Undeclared::NoScale)?;
        let scale = said.parse().map_err(|_| Undeclared::ScaleNotANumber)?;

        let Ok(found) = after(&block, Named("transform"));

        let turn = found.ok_or(Undeclared::NoTransform)?;
        let transform = number(&turn)?;

        Ok(Screen { mode: Size { wide, tall }, refresh, scale, transform })
    }

    pub fn turned(&self) -> Result<Turned, Never> {
        Ok(match self.transform & 1 == 1 {
            true => Turned::Sideways,
            false => Turned::Upright,
        })
    }

    pub fn pixels(&self) -> Result<Size<u32>, Never> {
        let Ok(turned) = self.turned();

        Ok(match turned {
            Turned::Sideways => Size { wide: self.mode.tall, tall: self.mode.wide },
            Turned::Upright => self.mode,
        })
    }

    pub fn on_the_panel(&self, at: Point<u32>) -> Result<Point<u32>, Never> {
        let Ok(room) = self.logical();
        let (across, down) = (
            f64::from(at.across) / f64::from(room.wide.max(1)),
            f64::from(at.down) / f64::from(room.tall.max(1)),
        );

        let (x, y) = match self.transform & 3 {
            0 => (across, down),
            1 => (down, 1.0 - across),
            2 => (1.0 - across, 1.0 - down),
            _ => (1.0 - down, across),
        };

        let Ok(across) = whole_u32(x * f64::from(self.mode.wide));
        let Ok(down) = whole_u32(y * f64::from(self.mode.tall));

        Ok(Point { across, down })
    }

    pub fn logical(&self) -> Result<Size<u32>, Never> {
        self.logical_at(self.scale)
    }

    pub fn logical_at(&self, scale: f64) -> Result<Size<u32>, Never> {
        let Ok(pixels) = self.pixels();

        let Ok(wide) = whole_u32(f64::from(pixels.wide) / scale);
        let Ok(tall) = whole_u32(f64::from(pixels.tall) / scale);

        Ok(Size { wide, tall })
    }

    pub fn cut_to(&self, room: Size<u32>) -> Result<f64, Never> {
        let Ok(pixels) = self.pixels();

        let fits = (f64::from(room.wide) / f64::from(pixels.wide))
            .min(f64::from(room.tall) / f64::from(pixels.tall))
            .min(1.0);

        Ok(self.scale * fits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canvas(pub u32);

pub const DRAWN_AT: Canvas = Canvas(1024);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fits {
    OnThePanel,
    WiderThanThePanel,
}

impl Canvas {
    pub fn scale_on(self, screen: &Screen) -> Result<f64, Never> {
        let Ok(pixels) = screen.pixels();

        Ok(f64::from(pixels.wide) / f64::from(self.0.max(1)))
    }

    pub fn fits(self, screen: &Screen) -> Result<Fits, Never> {
        let Ok(pixels) = screen.pixels();

        Ok(match self.0 > pixels.wide {
            true => Fits::WiderThanThePanel,
            false => Fits::OnThePanel,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mounted {
    Sideways,
    Upright,
}

pub const QUARTER: u32 = 1;

pub const SQUARE: u32 = 0;

pub const INTERNAL: &str = "eDP";

impl Mounted {
    pub fn of(mode: Size<u32>) -> Result<Self, Never> {
        Ok(match mode.tall > mode.wide {
            true => Mounted::Sideways,
            false => Mounted::Upright,
        })
    }

    pub fn transform(self) -> Result<u32, Never> {
        Ok(match self {
            Mounted::Sideways => QUARTER,
            Mounted::Upright => SQUARE,
        })
    }

    pub fn how_it_is_mounted(self) -> Result<&'static str, Never> {
        Ok(match self {
            Mounted::Sideways => "taller than it is wide, so it is turned a quarter",
            Mounted::Upright => "wider than it is tall, so it is not turned",
        })
    }
}

pub fn panel(monitors: &[Monitor]) -> Result<Option<&Monitor>, Never> {
    let built_in = monitors.iter().find(|monitor| monitor.named.starts_with(INTERNAL));

    Ok(built_in.or_else(|| monitors.first()))
}

pub fn driving(monitor: &Monitor) -> Result<Option<Screen>, Never> {
    let (wide, tall) = match monitor.size {
        Some(size) => size,
        None => return Ok(None),
    };

    let refresh = match monitor.refresh {
        Some(refresh) => refresh,
        None => return Ok(None),
    };

    let scale = match monitor.scale {
        Some(scale) => scale,
        None => return Ok(None),
    };

    let turn = match monitor.transform {
        Some(turn) => turn,
        None => return Ok(None),
    };

    let Ok(across) = fitted::<i64, u32>(wide);
    let Ok(down) = fitted::<i64, u32>(tall);
    let Ok(refresh) = whole_u32(refresh);
    let Ok(transform) = fitted::<i64, u32>(turn);

    Ok(Some(Screen { mode: Size { wide: across, tall: down }, refresh, scale, transform }))
}

pub fn shown(said: &serde_json::Value) -> Result<Option<Screen>, Never> {
    let Ok(monitors) = console_compositor::monitors(said);
    let Ok(panel) = panel(&monitors);

    match panel {
        Some(panel) => driving(panel),
        None => Ok(None),
    }
}

pub fn here() -> Result<Option<Screen>, console_compositor::Unanswered> {
    let said = console_compositor::asked(console_compositor::Asked::Monitors)?;
    let Ok(shown) = shown(&said);

    Ok(shown)
}

pub fn bar_css(screen: &Screen, scale: f64) -> Result<String, Never> {
    let Ok(room) = screen.logical_at(scale);
    let wide = room.wide;

    Ok(format!(
        "/* Written by console-scale. The apply strip is as wide as the screen,\n   \
         and how wide that is depends on the size the screen is set to. */\n\
         window#waybar #custom-updating {{\n  min-width: {wide}px;\n}}\n"
    ))
}

struct Wrapped {
    open: char,
    close: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Named<'a>(&'a str);

fn between(text: &str, name: Named<'_>, wrapped: Wrapped) -> Result<Option<String>, Never> {
    let Wrapped { open, close } = wrapped;
    let name = name.0;

    let found = match text.find(name) {
        Some(found) => found,
        None => return Ok(None),
    };

    let at = found.saturating_add(name.len());

    let after_name = match text.get(at..) {
        Some(after_name) => after_name,
        None => return Ok(None),
    };

    let opened = match after_name.find(open) {
        Some(opened) => opened,
        None => return Ok(None),
    };

    let start = at.saturating_add(opened).saturating_add(open.len_utf8());

    let inside = match text.get(start..) {
        Some(inside) => inside,
        None => return Ok(None),
    };

    let closed = match inside.find(close) {
        Some(closed) => closed,
        None => return Ok(None),
    };

    let end = start.saturating_add(closed);

    Ok(text.get(start..end).map(str::to_string))
}

fn after(block: &str, name: Named<'_>) -> Result<Option<String>, Never> {
    let name = name.0;

    let found = match block.find(name) {
        Some(found) => found,
        None => return Ok(None),
    };

    let at = found.saturating_add(name.len());

    let after_name = match block.get(at..) {
        Some(after_name) => after_name,
        None => return Ok(None),
    };

    let rest = after_name.trim_start();

    let valued = match rest.strip_prefix('=') {
        Some(valued) => valued,
        None => return Ok(None),
    };

    let rest = valued.trim_start();
    let said: String = match rest.strip_prefix('"') {
        Some(quoted) => quoted.chars().take_while(|c| *c != '"').collect(),
        None => rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect(),
    };

    Ok(match said.is_empty() {
        true => None,
        false => Some(said),
    })
}

fn number(said: &str) -> Result<u32, Undeclared> {
    said.trim()
        .parse()
        .map_err(|_| Undeclared::NotAWholeNumber(said.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANDHELD: &str = r#"[{"name":"eDP-1","width":1600,"height":2560,"refreshRate":144.0,
        "scale":2.5,"transform":1}]"#;

    const LAPTOP: &str = r#"[{"name":"eDP-1","width":1920,"height":1200,"refreshRate":60.003,
        "scale":1.0,"transform":0}]"#;

    const A_SCREEN_AND_THE_PANEL: &str = r#"[{"name":"DP-3","width":3840,"height":2160,
        "refreshRate":60.0,"scale":1.5,"transform":0},{"name":"eDP-1","width":1920,"height":1200,
        "refreshRate":60.003,"scale":1.0,"transform":0}]"#;

    fn monitors(said: &str) -> serde_json::Value {
        match serde_json::from_str(said) {
            Ok(read) => read,
            Err(fault) => panic!("the fixture is not json: {fault}"),
        }
    }

    fn asked(said: &str) -> Screen {
        let Ok(shown) = shown(&monitors(said));

        match shown {
            Some(screen) => screen,
            None => panic!("the compositor named a monitor and this read nothing off it"),
        }
    }

    fn size(wide: u32, tall: u32) -> Size<u32> {
        Size { wide, tall }
    }

    fn point(across: u32, down: u32) -> Point<u32> {
        Point { across, down }
    }


    #[test]
    fn the_screen_this_device_has_is_read() {
        let screen = declared().expect("the compositor declares a screen");
        assert_eq!(screen.pixels(), Ok(size(2560, 1600)));
        assert_eq!(
            screen.turned(),
            Ok(Turned::Sideways),
            "the panel is mounted portrait and turned"
        );
    }

    #[test]
    fn a_finger_lands_where_the_picture_says_it_should() {
        let screen = Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.on_the_panel(point(204, 151)), Ok(point(378, 2050)));

        assert_eq!(screen.on_the_panel(point(0, 0)), Ok(point(0, 2560)), "the top left of the picture");
        assert_eq!(screen.on_the_panel(point(1024, 640)), Ok(point(1600, 0)), "the bottom right");
    }

    #[test]
    fn a_screen_that_is_not_turned_leaves_a_finger_where_it_was() {
        let upright = Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform: 0 };
        let Ok(room) = upright.logical();

        assert_eq!(upright.on_the_panel(point(0, 0)), Ok(point(0, 0)));
        assert_eq!(
            upright.on_the_panel(point(room.wide, room.tall)),
            Ok(point(1600, 2560))
        );
    }

    #[test]
    fn every_turn_puts_the_corner_somewhere_of_its_own() {
        let corners: Vec<Point<u32>> = (0..4)
            .map(|transform| {
                let screen =
                    Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform };

                let Ok(corner) = screen.on_the_panel(point(0, 0));

                corner
            })
            .collect();

        for (at, corner) in corners.iter().enumerate() {
            for other in &corners[at + 1..] {
                assert_ne!(corner, other, "two turns put the top left corner in one place");
            }
        }
    }

    #[test]
    fn a_picture_of_a_turned_screen_is_the_mode_the_other_way_round() {
        let portrait = Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(portrait.pixels(), Ok(size(2560, 1600)));

        let upright = Screen { transform: 0, ..portrait };

        assert_eq!(upright.pixels(), Ok(size(1600, 2560)));
    }

    #[test]
    fn the_desktop_is_laid_out_at_the_density_it_was_told() {
        let screen = Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.logical(), Ok(size(1024, 640)));
    }

    #[test]
    fn cutting_to_a_screen_it_already_fits_on_gives_up_nothing() {
        let screen = Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.cut_to(size(3840, 2160)), Ok(2.5));
    }

    #[test]
    fn cutting_to_a_smaller_screen_gives_up_only_the_density() {
        let screen = Screen { mode: Size { wide: 1600, tall: 2560 }, refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.cut_to(size(1280, 1600)), Ok(1.25));
    }

    #[test]
    fn the_bar_is_told_how_wide_the_screen_became() {
        let screen = declared().expect("the compositor declares a screen");
        let Ok(said) = bar_css(&screen, 2.0);
        let Ok(at_scale) = bar_css(&screen, screen.scale);

        assert!(said.contains("min-width: 1280px"), "{said}");
        assert!(said.contains("window#waybar #custom-updating"), "the rule cannot outrank");
        assert!(at_scale.contains("min-width: 1024px"));
    }

    #[test]
    fn a_file_that_declares_no_screen_says_so_rather_than_guessing() {
        assert_eq!(Screen::read("-- nothing here\n"), Err(Undeclared::NoScreen));
    }

    #[test]
    fn a_screen_missing_a_number_names_the_number() {
        assert_eq!(
            Screen::read("hl.monitor({ mode = \"1600x2560@144\", scale = 2.5 })"),
            Err(Undeclared::NoTransform)
        );
    }

    #[test]
    fn the_screen_the_compositor_says_it_is_driving_is_the_one_the_file_declares() {
        let declared = declared().expect("the compositor's file declares a screen");

        assert_eq!(asked(HANDHELD), declared, "the device's own panel, asked rather than read");
    }

    #[test]
    fn a_laptops_panel_is_read_the_same_way_and_is_nothing_like_the_devices() {
        let screen = asked(LAPTOP);

        assert_eq!(screen.mode, size(1920, 1200));
        assert_eq!(screen.scale, 1.0);
        assert_eq!(screen.transform, SQUARE);
        assert_eq!(screen.pixels(), Ok(size(1920, 1200)), "nothing is turned");
    }

    #[test]
    fn the_panel_is_the_built_in_one_and_not_whichever_screen_hyprland_names_first() {
        let screen = asked(A_SCREEN_AND_THE_PANEL);

        assert_eq!(screen.mode, size(1920, 1200), "the monitor on the desk is not the panel");
    }

    #[test]
    fn a_panel_taller_than_it_is_wide_was_screwed_in_sideways() {
        assert_eq!(Mounted::of(size(1600, 2560)), Ok(Mounted::Sideways), "the handheld");
        assert_eq!(Mounted::of(size(1920, 1200)), Ok(Mounted::Upright), "a laptop");
        assert_eq!(Mounted::of(size(2560, 1600)), Ok(Mounted::Upright));
    }

    #[test]
    fn what_a_sideways_panel_wants_is_the_quarter_turn_the_device_is_set_up_with() {
        let declared = declared().expect("the compositor's file declares a screen");
        let Ok(mounted) = Mounted::of(declared.mode);
        let Ok(transform) = mounted.transform();

        assert_eq!(
            transform, declared.transform,
            "the turn in the compositor's file is the one the panel's own mode asks for, so a \
             machine nobody here owns needs no file to say it"
        );
    }

    #[test]
    fn a_compositor_with_no_monitors_says_so_rather_than_inventing_a_screen() {
        let Ok(shown) = shown(&monitors("[]"));

        assert_eq!(shown, None);
    }

    #[test]
    fn a_monitor_missing_half_of_what_a_screen_is_is_not_half_a_screen() {
        let Ok(shown) = shown(&monitors(r#"[{"name":"eDP-1","width":1920,"height":1200}]"#));

        assert_eq!(shown, None, "no refresh, no scale and no transform is not a reading");
    }

    #[test]
    fn a_canvas_is_the_scale_the_panel_has_to_wear_to_be_that_wide() {
        let declared = declared().expect("the compositor's file declares a screen");
        let Ok(scale) = DRAWN_AT.scale_on(&declared);

        assert_eq!(scale, declared.scale, "the device is set up at the canvas it was drawn at");
    }

    #[test]
    fn the_same_canvas_on_a_laptops_panel_is_a_different_density() {
        let laptop = Screen { mode: size(1920, 1200), refresh: 60, scale: 1.0, transform: SQUARE };
        let Ok(scale) = DRAWN_AT.scale_on(&laptop);

        assert_eq!(scale, 1.875);
        assert_eq!(laptop.logical_at(scale), Ok(size(1024, 640)), "the canvas, in whole pixels");
    }

    #[test]
    fn a_canvas_wider_than_the_panel_says_so_rather_than_scaling_below_one() {
        let small = Screen { mode: size(1280, 800), refresh: 60, scale: 1.0, transform: SQUARE };

        assert_eq!(Canvas(1280).fits(&small), Ok(Fits::OnThePanel));
        assert_eq!(Canvas(1600).fits(&small), Ok(Fits::WiderThanThePanel));
    }
}
