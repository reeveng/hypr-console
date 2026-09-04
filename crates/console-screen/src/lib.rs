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


use console_number::whole_u32;

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

/// The declaration itself, read at build time.
///
/// For the programs that run on the device, where there is no source tree to
/// read `CONFIG` out of. The mode, the refresh and the transform are facts
/// about a panel that is soldered to this machine, so a copy of them compiled
/// into a program cannot go stale in any way the file itself would not.
///
/// The density is the one number here anybody changes, and it is changed
/// against this rather than in it: `console_settings::size` asks the
/// compositor what it is standing at now and hands it back the rest of this.
///
/// It answers the same way `read` does, rather than being sure of itself. The
/// text is compiled in and `the_screen_this_device_has_is_read` proves it
/// parses, so a build where this fails is a build that failed its own tests --
/// but a program that turns that into a panic is a program that dies with no
/// more to say than a line number, and every caller of this is a program a
/// person is holding at the time.
pub fn declared() -> Result<Screen, String> {
    Screen::read(DECLARED)
}

const DECLARED: &str = include_str!("../../../files/home/@user@/.config/hypr/hyprland.lua");

/// Which way round the compositor is drawing the panel.
///
/// The device's panel is mounted portrait and the desktop is landscape, so
/// `Sideways` is the ordinary state here and `Upright` is the odd one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turned {
    /// A quarter or three quarters, so width and height swap.
    Sideways,
    /// However the panel is built, with nothing swapped.
    Upright,
}

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
    pub fn turned(&self) -> Turned {
        match self.transform % 2 == 1 {
            true => Turned::Sideways,
            false => Turned::Upright,
        }
    }

    /// The size of a picture of it, which is the mode after the turn.
    pub fn pixels(&self) -> (u32, u32) {
        match self.turned() {
            Turned::Sideways => (self.mode.1, self.mode.0),
            Turned::Upright => self.mode,
        }
    }

    /// Where a finger has to go down to land on that point of the picture.
    ///
    /// The panel is mounted portrait and the picture is turned into landscape,
    /// so a point on the screen and the point of glass over it are not the
    /// same numbers -- the touchscreen reports in the panel's own frame, and
    /// the compositor turns what it says by the same amount it turns what it
    /// draws. Anything that wants to press a place on the screen has to undo
    /// that, and this is the one place it is undone: `hyprland.lua` says the
    /// turn once, and everything that has to know reads it from here rather
    /// than working it out again and getting it the other way round.
    ///
    /// Taken and given in the panel's own resolution, which is what a
    /// touchscreen for this machine reports in.
    ///
    /// The quarter turn is the one this machine is mounted at and the one that
    /// was measured on it: a finger put down at a known place on the glass,
    /// and the application it opened read back off the compositor. The other
    /// three are the same family of turns and follow from it.
    pub fn poked(&self, at: (u32, u32)) -> (u32, u32) {
        let (wide, tall) = self.logical();
        let (across, down) = (
            f64::from(at.0) / f64::from(wide.max(1)),
            f64::from(at.1) / f64::from(tall.max(1)),
        );

        let (x, y) = match self.transform % 4 {
            1 => (down, 1.0 - across),
            2 => (1.0 - across, 1.0 - down),
            3 => (1.0 - down, across),
            _ => (across, down),
        };

        (
            whole_u32(x * f64::from(self.mode.0)),
            whole_u32(y * f64::from(self.mode.1)),
        )
    }

    /// The size the desktop is laid out in, which is what a window sees.
    pub fn logical(&self) -> (u32, u32) {
        self.logical_at(self.scale)
    }

    /// The same, at a density this screen is not standing at.
    ///
    /// For the one setting that changes the density: what the desktop would be
    /// laid out in on the other rungs of the ladder, which is what the bar's
    /// stylesheet has to be told. See `console_settings::size`.
    pub fn logical_at(&self, scale: f64) -> (u32, u32) {
        let (wide, tall) = self.pixels();
        (
            whole_u32(f64::from(wide) / scale),
            whole_u32(f64::from(tall) / scale),
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
        let fits = (f64::from(room.0) / f64::from(wide))
            .min(f64::from(room.1) / f64::from(tall))
            .min(1.0);
        self.scale * fits
    }
}

/// The rule the bar's stylesheet is handed, so the apply strip is as wide as
/// the screen at whatever density it is laid out at.
///
/// The strip's fill is a gradient with a hard stop in it, and a gradient's
/// percentages are percentages of the box it is painted in -- so the box has to
/// be the screen. `min-width` is the only number outside this crate that has to
/// be told the density, which is why the writing of it is here: the settings
/// write one when somebody changes the size, the staged desktop writes one
/// because it is a desktop, and a rule written twice is a bar that goes out of
/// step with the screen it is drawn on.
///
/// Written more specifically than the stylesheet's own rule rather than after
/// it. GTK takes `@import` only at the top of a file, so a file imported there
/// cannot win on order and has to win on specificity instead.
pub fn bar_css(screen: &Screen, scale: f64) -> String {
    let (wide, _) = screen.logical_at(scale);
    format!(
        "/* Written by console-scale. The apply strip is as wide as the screen,\n   \
         and how wide that is depends on the size the screen is set to. */\n\
         window#waybar #custom-updating {{\n  min-width: {wide}px;\n}}\n"
    )
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

    #[test]
    fn the_screen_this_device_has_is_read() {
        let screen = declared().expect("the compositor declares a screen");
        assert_eq!(screen.pixels(), (2560, 1600));
        assert_eq!(screen.turned(), Turned::Sideways, "the panel is mounted portrait and turned");
    }

    /// The one measurement this rests on, written down as a test.
    ///
    /// On the device, a finger put down at this point of the glass opened the
    /// application on the top-left square of the home screen -- which is where
    /// the desktop draws it, a little in from the corner. Everything that
    /// presses a place on this screen is that arithmetic and nothing else, so
    /// getting the turn the other way round would put every press a
    /// point-reflection away from where it was meant, which is a bug that
    /// looks like a machine ignoring you rather than one misreading you.
    #[test]
    fn a_finger_lands_where_the_picture_says_it_should() {
        let screen = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 1 };
        assert_eq!(screen.poked((204, 151)), (378, 2050));

        // The corners, which say which way round it is more plainly than any
        // point in the middle can.
        assert_eq!(screen.poked((0, 0)), (0, 2560), "the top left of the picture");
        assert_eq!(screen.poked((1024, 640)), (1600, 0), "the bottom right");
    }

    /// An unturned panel is the case with nothing to undo, and it is worth
    /// holding: an arithmetic that turned something anyway would be wrong on
    /// every machine that is not this one.
    #[test]
    fn a_screen_that_is_not_turned_leaves_a_finger_where_it_was() {
        let upright = Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform: 0 };
        let (wide, tall) = upright.logical();
        assert_eq!(upright.poked((0, 0)), (0, 0));
        assert_eq!(upright.poked((wide, tall)), (1600, 2560));
    }

    /// And each turn undoes the one before it, however many there are of them.
    #[test]
    fn every_turn_puts_the_corner_somewhere_of_its_own() {
        let corners: Vec<(u32, u32)> = (0..4)
            .map(|transform| {
                Screen { mode: (1600, 2560), refresh: 144, scale: 2.5, transform }.poked((0, 0))
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

    /// The strip is as wide as the screen at whatever density it is laid out
    /// at, and it has to outrank the stylesheet's own rule to say so.
    #[test]
    fn the_bar_is_told_how_wide_the_screen_became() {
        let screen = declared().expect("the compositor declares a screen");
        let said = bar_css(&screen, 2.0);
        assert!(said.contains("min-width: 1280px"), "{said}");
        assert!(said.contains("window#waybar #custom-updating"), "the rule cannot outrank");
        assert!(bar_css(&screen, screen.scale).contains("min-width: 1024px"));
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
