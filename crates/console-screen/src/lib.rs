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
//! someone screwed in sideways, which is what the Legion Go's 1600x2560 eDP is
//! and what no laptop's is. A desktop that has to be told that in a file per
//! machine is a desktop no one can install on a machine no one here owns.
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

pub const CONFIGURATION: &str = "files/home/@user@/.config/console/hypr/hyprland.lua";

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

pub fn from_hyprland_config() -> Result<Screen, Undeclared> {
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
        let Ok(found) = value_between(lua, Named("hl.monitor"), Wrapped { open: '{', close: '}' });

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

        Ok(Screen { mode: Size { width: wide, height: tall }, refresh, scale, transform })
    }

    pub fn orientation(&self) -> Result<Turned, Never> {
        Ok(match self.transform & 1 == 1 {
            true => Turned::Sideways,
            false => Turned::Upright,
        })
    }

    pub fn pixels(&self) -> Result<Size<u32>, Never> {
        let Ok(turned) = self.orientation();

        Ok(match turned {
            Turned::Sideways => Size { width: self.mode.height, height: self.mode.width },
            Turned::Upright => self.mode,
        })
    }

    pub fn on_the_panel(&self, at: Point<u32>) -> Result<Point<u32>, Never> {
        let Ok(room) = self.logical();
        let (across, down) = (
            f64::from(at.x) / f64::from(room.width.max(1)),
            f64::from(at.y) / f64::from(room.height.max(1)),
        );

        let (x, y) = match self.transform & 3 {
            0 => (across, down),
            1 => (down, 1.0 - across),
            2 => (1.0 - across, 1.0 - down),
            _ => (1.0 - down, across),
        };

        let Ok(across) = whole_u32(x * f64::from(self.mode.width));
        let Ok(down) = whole_u32(y * f64::from(self.mode.height));

        Ok(Point { x: across, y: down })
    }

    pub fn logical(&self) -> Result<Size<u32>, Never> {
        self.logical_at(self.scale)
    }

    pub fn logical_at(&self, scale: f64) -> Result<Size<u32>, Never> {
        let Ok(pixels) = self.pixels();

        let Ok(wide) = whole_u32(f64::from(pixels.width) / scale);
        let Ok(tall) = whole_u32(f64::from(pixels.height) / scale);

        Ok(Size { width: wide, height: tall })
    }

    pub fn cut_to(&self, room: Size<u32>) -> Result<f64, Never> {
        let Ok(pixels) = self.pixels();

        let fits = (f64::from(room.width) / f64::from(pixels.width))
            .min(f64::from(room.height) / f64::from(pixels.height))
            .min(1.0);

        Ok(self.scale * fits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Wider,
    Taller,
}

impl Shape {
    pub fn other(self) -> Result<Shape, Never> {
        Ok(match self {
            Shape::Wider => Shape::Taller,
            Shape::Taller => Shape::Wider,
        })
    }
}

impl Screen {
    pub fn shape(&self) -> Result<Shape, Never> {
        let Ok(pixels) = self.pixels();

        Ok(match pixels.height > pixels.width {
            true => Shape::Taller,
            false => Shape::Wider,
        })
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

        Ok(f64::from(pixels.width) / f64::from(self.0.max(1)))
    }

    pub fn fits(self, screen: &Screen) -> Result<Fits, Never> {
        let Ok(pixels) = screen.pixels();

        Ok(match self.0 > pixels.width {
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
        Ok(match mode.height > mode.width {
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

pub fn from_monitor(monitor: &Monitor) -> Result<Option<Screen>, Never> {
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

    Ok(Some(Screen { mode: Size { width: across, height: down }, refresh, scale, transform }))
}

pub fn shown(monitors: &[console_compositor::Monitor]) -> Result<Option<Screen>, Never> {
    let Ok(panel) = panel(monitors);

    match panel {
        Some(panel) => from_monitor(panel),
        None => Ok(None),
    }
}

pub fn here() -> Result<Option<Screen>, console_compositor::HyprctlError> {
    let found = driving_here()?;

    Ok(found.map(|(_what_it_is_called, screen)| screen))
}

pub fn driving_here() -> Result<Option<(String, Screen)>, console_compositor::HyprctlError> {
    let monitors = console_compositor::ask(console_compositor::Monitors)?;
    let Ok(found) = panel(&monitors);

    let found = match found {
        Some(found) => found,
        None => return Ok(None),
    };

    let Ok(screen) = from_monitor(found);

    Ok(screen.map(|screen| (found.named.clone(), screen)))
}

struct Wrapped {
    open: char,
    close: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Named<'a>(&'a str);

fn value_between(text: &str, name: Named<'_>, wrapped: Wrapped) -> Result<Option<String>, Never> {
    let Wrapped { open, close } = wrapped;
    let name = name.0;

    Ok(text
        .split_once(name)
        .and_then(|(_, after_name)| after_name.split_once(open))
        .and_then(|(_, inside)| inside.split_once(close))
        .map(|(between, _)| between.to_string()))
}

fn after(block: &str, name: Named<'_>) -> Result<Option<String>, Never> {
    let name = name.0;

    let after_name = match block.split_once(name) {
        Some((_, after_name)) => after_name,
        None => return Ok(None),
    };

    let rest = after_name.trim_start();

    let valued = match rest.strip_prefix('=') {
        Some(valued) => valued,
        None => return Ok(None),
    };

    let rest = valued.trim_start();
    let said: String = match rest.strip_prefix('"') {
        Some(quoted) => quoted.chars().take_while(|character| *character != '"').collect(),
        None => rest
            .chars()
            .take_while(|character| character.is_ascii_digit() || *character == '.')
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
    use std::error::Error;

    const HANDHELD: &str = r#"[{"name":"eDP-1","width":1600,"height":2560,"refreshRate":144.0,
        "scale":2.5,"transform":1}]"#;

    const LAPTOP: &str = r#"[{"name":"eDP-1","width":1920,"height":1200,"refreshRate":60.003,
        "scale":1.0,"transform":0}]"#;

    const A_SCREEN_AND_THE_PANEL: &str = r#"[{"name":"DP-3","width":3840,"height":2160,
        "refreshRate":60.0,"scale":1.5,"transform":0},{"name":"eDP-1","width":1920,"height":1200,
        "refreshRate":60.003,"scale":1.0,"transform":0}]"#;

    fn monitors(said: &str) -> Result<Vec<console_compositor::Monitor>, Box<dyn Error>> {
        let monitors = console_compositor::read(console_compositor::Monitors, said)?;

        Ok(monitors)
    }

    fn screen_from(said: &str) -> Result<Screen, Box<dyn Error>> {
        let monitors = monitors(said)?;
        let Ok(shown) = shown(&monitors);
        let screen = shown.ok_or("the compositor named a monitor and this read nothing off it")?;

        Ok(screen)
    }

    fn size((width, height): (u32, u32)) -> Result<Size<u32>, Never> {
        Ok(Size { width, height })
    }

    fn point((x, y): (u32, u32)) -> Result<Point<u32>, Never> {
        Ok(Point { x, y })
    }

    const HANDHELD_SCREEN: Screen = Screen { mode: Size { width: 1600, height: 2560 }, refresh: 144, scale: 2.5, transform: 1 };

    const LAPTOP_SCREEN: Screen = Screen { mode: Size { width: 1920, height: 1200 }, refresh: 60, scale: 1.0, transform: SQUARE };

    #[test]
    fn the_screen_this_device_has_is_read() -> Result<(), Box<dyn Error>> {
        let screen = from_hyprland_config()?;
        let Ok(turned) = size((2560, 1600));

        assert_eq!(screen.pixels(), Ok(turned));
        assert_eq!(
            screen.orientation(),
            Ok(Turned::Sideways),
            "the panel is mounted portrait and turned"
        );

        Ok(())
    }

    #[test]
    fn the_seed_reads_a_finger_through_the_quarter_it_draws_the_seed_screen_at() -> Result<(), Box<dyn Error>> {
        let screen = from_hyprland_config()?;
        let said = format!("transform = {}", screen.transform);
        let (_, from_touch) = DECLARED.split_once("touchdevice").ok_or("the seed says how a touch is read")?;

        assert!(
            from_touch.contains(&said),
            "the seed draws at {} and reads a finger at something else",
            screen.transform
        );

        let (before_dofile, _) = DECLARED.split_once("pcall(dofile").ok_or("the seed reads the machine's own block")?;

        assert!(
            before_dofile.contains("touchdevice"),
            "the machine's own block is read first, so the seed's quarter outlives it"
        );

        Ok(())
    }

    #[test]
    fn a_finger_lands_where_the_picture_says_it_should() {
        let Ok(finger) = point((204, 151));
        let Ok(landed) = point((378, 2050));
        let Ok(top_left) = point((0, 0));
        let Ok(bottom_left) = point((0, 2560));
        let Ok(bottom_right) = point((1024, 640));
        let Ok(top_right) = point((1600, 0));

        assert_eq!(HANDHELD_SCREEN.on_the_panel(finger), Ok(landed));
        assert_eq!(HANDHELD_SCREEN.on_the_panel(top_left), Ok(bottom_left), "the top left of the picture");
        assert_eq!(HANDHELD_SCREEN.on_the_panel(bottom_right), Ok(top_right), "the bottom right");
    }

    #[test]
    fn a_screen_that_is_not_turned_leaves_a_finger_where_it_was() {
        let upright = Screen { transform: 0, ..HANDHELD_SCREEN };
        let Ok(room) = upright.logical();
        let Ok(top_left) = point((0, 0));
        let Ok(far_corner) = point((room.width, room.height));
        let Ok(on_the_panel) = point((1600, 2560));

        assert_eq!(upright.on_the_panel(top_left), Ok(top_left));
        assert_eq!(upright.on_the_panel(far_corner), Ok(on_the_panel));
    }

    #[test]
    fn every_turn_puts_the_corner_somewhere_of_its_own() {
        let Ok(top_left) = point((0, 0));
        let corners: Vec<Point<u32>> = (0..4)
            .map(|transform| {
                let Ok(corner) = Screen { transform, ..HANDHELD_SCREEN }.on_the_panel(top_left);

                corner
            })
            .collect();

        for (at, corner) in corners.iter().enumerate() {
            for other in corners.iter().skip(at.saturating_add(1)) {
                assert_ne!(corner, other, "two turns put the top left corner in one place");
            }
        }
    }

    #[test]
    fn the_shape_a_screen_stands_in_is_the_picture_and_not_the_mounting() {
        let portrait = HANDHELD_SCREEN;
        let laptop = LAPTOP_SCREEN;

        assert_eq!(portrait.shape(), Ok(Shape::Wider), "a sideways panel at its quarter is wide");
        assert_eq!(Screen { transform: 3, ..portrait }.shape(), Ok(Shape::Wider), "the other quarter");
        assert_eq!(Screen { transform: 0, ..portrait }.shape(), Ok(Shape::Taller));
        assert_eq!(Screen { transform: 2, ..portrait }.shape(), Ok(Shape::Taller), "upside down");
        assert_eq!(laptop.shape(), Ok(Shape::Wider));
        assert_eq!(Screen { transform: 1, ..laptop }.shape(), Ok(Shape::Taller));
    }

    #[test]
    fn a_picture_of_a_turned_screen_is_the_mode_the_other_way_round() {
        let Ok(turned) = size((2560, 1600));
        let Ok(upright) = size((1600, 2560));

        assert_eq!(HANDHELD_SCREEN.pixels(), Ok(turned));
        assert_eq!(Screen { transform: 0, ..HANDHELD_SCREEN }.pixels(), Ok(upright));
    }

    #[test]
    fn the_desktop_is_laid_out_at_the_density_it_was_told() {
        let Ok(canvas) = size((1024, 640));

        assert_eq!(HANDHELD_SCREEN.logical(), Ok(canvas));
    }

    #[test]
    fn cutting_to_a_screen_it_already_fits_on_gives_up_nothing() {
        let Ok(bigger) = size((3840, 2160));

        assert_eq!(HANDHELD_SCREEN.cut_to(bigger), Ok(2.5));
    }

    #[test]
    fn cutting_to_a_smaller_screen_gives_up_only_the_density() {
        let Ok(smaller) = size((1280, 1600));

        assert_eq!(HANDHELD_SCREEN.cut_to(smaller), Ok(1.25));
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
    fn the_screen_the_compositor_says_it_is_driving_is_the_one_the_file_declares() -> Result<(), Box<dyn Error>> {
        let declared = from_hyprland_config()?;
        let asked = screen_from(HANDHELD)?;

        assert_eq!(asked, declared, "the device's own panel, asked rather than read");

        Ok(())
    }

    #[test]
    fn a_laptops_panel_is_read_the_same_way_and_is_nothing_like_the_devices() -> Result<(), Box<dyn Error>> {
        let screen = screen_from(LAPTOP)?;
        let Ok(mode) = size((1920, 1200));

        assert_eq!(screen.mode, mode);
        assert_eq!(screen.scale, 1.0);
        assert_eq!(screen.transform, SQUARE);
        assert_eq!(screen.pixels(), Ok(mode), "nothing is turned");

        Ok(())
    }

    #[test]
    fn the_panel_is_the_built_in_one_and_not_whichever_screen_hyprland_names_first() -> Result<(), Box<dyn Error>> {
        let screen = screen_from(A_SCREEN_AND_THE_PANEL)?;
        let Ok(panel) = size((1920, 1200));

        assert_eq!(screen.mode, panel, "the monitor on the desk is not the panel");

        Ok(())
    }

    #[test]
    fn a_panel_taller_than_it_is_wide_was_screwed_in_sideways() {
        let Ok(handheld) = size((1600, 2560));
        let Ok(laptop) = size((1920, 1200));
        let Ok(wide) = size((2560, 1600));

        assert_eq!(Mounted::of(handheld), Ok(Mounted::Sideways), "the handheld");
        assert_eq!(Mounted::of(laptop), Ok(Mounted::Upright), "a laptop");
        assert_eq!(Mounted::of(wide), Ok(Mounted::Upright));
    }

    #[test]
    fn what_a_sideways_panel_wants_is_the_quarter_turn_the_device_is_set_up_with() -> Result<(), Box<dyn Error>> {
        let declared = from_hyprland_config()?;
        let Ok(mounted) = Mounted::of(declared.mode);
        let Ok(transform) = mounted.transform();

        assert_eq!(
            transform, declared.transform,
            "the turn in the compositor's file is the one the panel's own mode asks for, so a \
             machine no one here owns needs no file to say it"
        );

        Ok(())
    }

    #[test]
    fn a_compositor_with_no_monitors_says_so_rather_than_inventing_a_screen() -> Result<(), Box<dyn Error>> {
        let monitors = monitors("[]")?;
        let Ok(shown) = shown(&monitors);

        assert_eq!(shown, None);

        Ok(())
    }

    #[test]
    fn a_monitor_missing_half_of_what_a_screen_is_is_not_half_a_screen() -> Result<(), Box<dyn Error>> {
        let monitors = monitors(r#"[{"name":"eDP-1","width":1920,"height":1200}]"#)?;
        let Ok(shown) = shown(&monitors);

        assert_eq!(shown, None, "no refresh, no scale and no transform is not a reading");

        Ok(())
    }

    #[test]
    fn a_canvas_is_the_scale_the_panel_has_to_wear_to_be_that_wide() -> Result<(), Box<dyn Error>> {
        let declared = from_hyprland_config()?;
        let Ok(scale) = DRAWN_AT.scale_on(&declared);

        assert_eq!(scale, declared.scale, "the device is set up at the canvas it was drawn at");

        Ok(())
    }

    #[test]
    fn the_same_canvas_on_a_laptops_panel_is_a_different_density() {
        let Ok(scale) = DRAWN_AT.scale_on(&LAPTOP_SCREEN);
        let Ok(canvas) = size((1024, 640));

        assert_eq!(scale, 1.875);
        assert_eq!(LAPTOP_SCREEN.logical_at(scale), Ok(canvas), "the canvas, in whole pixels");
    }

    #[test]
    fn a_canvas_wider_than_the_panel_says_so_rather_than_scaling_below_one() {
        let Ok(mode) = size((1280, 800));
        let small = Screen { mode, ..LAPTOP_SCREEN };

        assert_eq!(Canvas(1280).fits(&small), Ok(Fits::OnThePanel));
        assert_eq!(Canvas(1600).fits(&small), Ok(Fits::WiderThanThePanel));
    }
}
