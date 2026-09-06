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


use console_never::Never;
use console_number_conversion::whole_u32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub mode: (u32, u32),
    pub refresh: u32,
    pub scale: f64,
    pub transform: u32,
}

pub const CONFIG: &str = "files/home/@user@/.config/hypr/hyprland.lua";

pub fn declared() -> Result<Screen, String> {
    Screen::read(DECLARED)
}

const DECLARED: &str = include_str!("../../../files/home/@user@/.config/hypr/hyprland.lua");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turned {
    Sideways,
    Upright,
}

impl Screen {
    pub fn read(lua: &str) -> Result<Self, String> {
        let Ok(found) = between(lua, "hl.monitor", '{', '}');

        let block = found.ok_or("the compositor's file declares no screen")?;

        let Ok(said) = after(&block, "mode");

        let mode = said.ok_or("the screen says nothing about its mode")?;
        let (wide, rest) = mode.split_once('x').ok_or("that mode is not WIDTHxHEIGHT")?;
        let (tall, refresh) = rest.split_once('@').ok_or("that mode names no refresh")?;
        let wide = number(wide)?;
        let tall = number(tall)?;
        let refresh = number(refresh)?;
        let Ok(found) = after(&block, "scale");

        let said = found.ok_or("the screen says nothing about its scale")?;
        let scale = said.parse().map_err(|_| "that scale is not a number".to_string())?;

        let Ok(found) = after(&block, "transform");

        let turn = found.ok_or("the screen says nothing about its transform")?;
        let transform = number(&turn)?;

        Ok(Screen { mode: (wide, tall), refresh, scale, transform })
    }

    pub fn turned(&self) -> Result<Turned, Never> {
        Ok(match self.transform & 1 == 1 {
            true => Turned::Sideways,
            false => Turned::Upright,
        })
    }

    pub fn pixels(&self) -> Result<(u32, u32), Never> {
        let Ok(turned) = self.turned();

        Ok(match turned {
            Turned::Sideways => (self.mode.1, self.mode.0),
            Turned::Upright => self.mode,
        })
    }

    pub fn on_the_panel(&self, at: (u32, u32)) -> Result<(u32, u32), Never> {
        let Ok((wide, tall)) = self.logical();
        let (across, down) = (
            f64::from(at.0) / f64::from(wide.max(1)),
            f64::from(at.1) / f64::from(tall.max(1)),
        );

        let (x, y) = match self.transform & 3 {
            0 => (across, down),
            1 => (down, 1.0 - across),
            2 => (1.0 - across, 1.0 - down),
            _ => (1.0 - down, across),
        };

        let Ok(across) = whole_u32(x * f64::from(self.mode.0));
        let Ok(down) = whole_u32(y * f64::from(self.mode.1));

        Ok((across, down))
    }

    pub fn logical(&self) -> Result<(u32, u32), Never> {
        self.logical_at(self.scale)
    }

    pub fn logical_at(&self, scale: f64) -> Result<(u32, u32), Never> {
        let Ok((wide, tall)) = self.pixels();

        let Ok(across) = whole_u32(f64::from(wide) / scale);
        let Ok(down) = whole_u32(f64::from(tall) / scale);

        Ok((across, down))
    }

    pub fn cut_to(&self, room: (u32, u32)) -> Result<f64, Never> {
        let Ok((wide, tall)) = self.pixels();

        let fits = (f64::from(room.0) / f64::from(wide))
            .min(f64::from(room.1) / f64::from(tall))
            .min(1.0);

        Ok(self.scale * fits)
    }
}

pub fn bar_css(screen: &Screen, scale: f64) -> Result<String, Never> {
    let Ok((wide, _)) = screen.logical_at(scale);

    Ok(format!(
        "/* Written by console-scale. The apply strip is as wide as the screen,\n   \
         and how wide that is depends on the size the screen is set to. */\n\
         window#waybar #custom-updating {{\n  min-width: {wide}px;\n}}\n"
    ))
}

fn between(
    text: &str,
    name: &str,
    open: char,
    close: char,
) -> Result<Option<String>, Never> {
    let Some(found) = text.find(name) else {
        return Ok(None);
    };

    let at = found.saturating_add(name.len());

    let Some(after_name) = text.get(at..) else {
        return Ok(None);
    };

    let Some(opened) = after_name.find(open) else {
        return Ok(None);
    };

    let start = at.saturating_add(opened).saturating_add(open.len_utf8());

    let Some(inside) = text.get(start..) else {
        return Ok(None);
    };

    let Some(closed) = inside.find(close) else {
        return Ok(None);
    };

    let end = start.saturating_add(closed);

    Ok(text.get(start..end).map(str::to_string))
}

fn after(block: &str, name: &str) -> Result<Option<String>, Never> {
    let Some(found) = block.find(name) else {
        return Ok(None);
    };

    let at = found.saturating_add(name.len());

    let Some(after_name) = block.get(at..) else {
        return Ok(None);
    };

    let rest = after_name.trim_start();

    let Some(valued) = rest.strip_prefix('=') else {
        return Ok(None);
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

fn number(said: &str) -> Result<u32, String> {
    said.trim().parse().map_err(|_| format!("{said:?} is not a whole number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_this_device_has_is_read() {
        let screen = declared().expect("the compositor declares a screen");
        assert_eq!(screen.pixels(), Ok((2560, 1600)));
        assert_eq!(
            screen.turned(),
            Ok(Turned::Sideways),
            "the panel is mounted portrait and turned"
        );
    }

    #[test]
    fn a_finger_lands_where_the_picture_says_it_should() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.on_the_panel((204, 151)), Ok((378, 2050)));

        assert_eq!(screen.on_the_panel((0, 0)), Ok((0, 2560)), "the top left of the picture");
        assert_eq!(screen.on_the_panel((1024, 640)), Ok((1600, 0)), "the bottom right");
    }

    #[test]
    fn a_screen_that_is_not_turned_leaves_a_finger_where_it_was() {
        let upright = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 0 };
        let Ok((wide, tall)) = upright.logical();

        assert_eq!(upright.on_the_panel((0, 0)), Ok((0, 0)));
        assert_eq!(upright.on_the_panel((wide, tall)), Ok((1600, 2560)));
    }

    #[test]
    fn every_turn_puts_the_corner_somewhere_of_its_own() {
        let corners: Vec<(u32, u32)> = (0..4)
            .map(|transform| {
                let screen =
                    Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform };

                let Ok(corner) = screen.on_the_panel((0, 0));

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
        let portrait = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(portrait.pixels(), Ok((2560, 1600)));

        let upright = Screen { transform: 0, ..portrait };

        assert_eq!(upright.pixels(), Ok((1600, 2560)));
    }

    #[test]
    fn the_desktop_is_laid_out_at_the_density_it_was_told() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.logical(), Ok((1024, 640)));
    }

    #[test]
    fn cutting_to_a_screen_it_already_fits_on_gives_up_nothing() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.cut_to((3840, 2160)), Ok(2.5));
    }

    #[test]
    fn cutting_to_a_smaller_screen_gives_up_only_the_density() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.cut_to((1280, 1600)), Ok(1.25));
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
