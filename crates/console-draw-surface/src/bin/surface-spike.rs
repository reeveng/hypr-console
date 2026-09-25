//! A rectangle on the screen, at whatever scale the compositor says.
//!
//!     surface-spike
//!     surface-spike --size 320x120 --anchor top-right --margin 12
//!
//! Not a program on the device and not themed: what it is for is the one
//! question `console-draw-surface` was written to answer, which is whether a surface
//! of ours comes out crisp on a panel driven at a scale that is not a whole
//! number. So it draws a border one device pixel wide and a checker of single
//! device pixels inside it. At the right scale the border is a hairline and the
//! checker is an even gray; at the wrong one the border doubles or vanishes and
//! the checker moires, and both are visible in a photograph rather than by
//! argument.
//!
//! It asks for nothing by default, which is how it is the same probe on any
//! machine: a size of zero by zero anchored to all four edges is a layer
//! surface the size of whatever screen it landed on, and the numbers it prints
//! come back from the compositor rather than from this file. A size is only
//! worth naming when what is being pressed is a particular surface's shape.

use std::process::ExitCode;
use std::time::Duration;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_draw_surface::standing::{Anchor, Closed, Keyboard, Margin, Room, Under, Wanted};
use console_draw_surface::{Scale, Surface};

const NAMESPACE: &str = "console-draw-surface-spike";

const GROUND: [u8; 4] = [0x30, 0x28, 0x24, 0xff];

const EDGE: [u8; 4] = [0xf0, 0xf0, 0xf0, 0xff];

const CHECK: [u8; 4] = [0x90, 0x90, 0x90, 0xff];

const TURNS: u32 = 40;

enum HexColor {
    Edge,
    Check,
    Ground,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let wanted = match asked(&arguments) {
        Ok(wanted) => wanted,
        Err(why) => {
            eprintln!("surface-spike: {why}");

            return ExitCode::FAILURE;
        }
    };

    let mut surface = match Surface::connect() {
        Ok(surface) => surface,
        Err(why) => {
            eprintln!("surface-spike: {why}");

            return ExitCode::FAILURE;
        }
    };

    match surface.show(&wanted) {
        Ok(()) => {}
        Err(why) => {
            eprintln!("surface-spike: {why}");

            return ExitCode::FAILURE;
        }
    }

    let Ok(panel) = driving();

    println!("{panel}");

    let mut left = TURNS;

    while left > 0 {
        match surface.draw(paint) {
            Ok(()) => {}
            Err(why) => {
                eprintln!("surface-spike: {why}");

                return ExitCode::FAILURE;
            }
        }

        let Ok(said) = said(&surface);

        println!("{said}");

        match surface.wait(&[], Some(Duration::from_millis(250))) {
            Ok(()) => {}
            Err(why) => {
                eprintln!("surface-spike: {why}");

                return ExitCode::FAILURE;
            }
        }

        match surface.closed() {
            Ok(Closed::Yes) => return ExitCode::SUCCESS,
            Ok(Closed::No) | Err(_) => {}
        }

        left = left.saturating_sub(1);
    }

    ExitCode::SUCCESS
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Size(String),
    Anchor(String),
    Margin(String),
    Without(&'static str),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Size(said) => {
                write!(to, "a size is written 320x120, and this says {said}")
            }
            ParseError::Anchor(said) => write!(
                to,
                "an anchor is top, top-right, bottom or whole, and this says {said}"
            ),
            ParseError::Margin(said) => write!(to, "a margin is a whole number, and this says {said}"),
            ParseError::Without(flag) => write!(to, "{flag} was given nothing to read"),
        }
    }
}

impl std::error::Error for ParseError {}

fn asked(arguments: &[String]) -> Result<Wanted, ParseError> {
    let mut wanted = Wanted {
        namespace: NAMESPACE.to_string(),
        anchor: Anchor::Whole,
        size: Size { width: 0, height: 0 },
        margin: Margin::default(),
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::None,
    };
    let mut rest = arguments.iter();

    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--size" => {
                let said = rest.next().ok_or(ParseError::Without("--size"))?;

                let size = measured(said)?;

                wanted.size = size;
            }
            "--anchor" => {
                let said = rest.next().ok_or(ParseError::Without("--anchor"))?;

                let anchor = anchored(said)?;

                wanted.anchor = anchor;
            }
            "--margin" => {
                let said = rest.next().ok_or(ParseError::Without("--margin"))?;

                let margin =
                    said.parse::<i32>().map_err(|_| ParseError::Margin(said.clone()))?;

                wanted.margin =
                    Margin { top: margin, right: margin, bottom: margin, left: margin };
            }
            _ => {}
        }
    }

    Ok(wanted)
}

fn measured(said: &str) -> Result<Size<u32>, ParseError> {
    let (wide, tall) = said.split_once('x').ok_or_else(|| ParseError::Size(said.to_string()))?;
    let wide = wide.parse::<u32>().map_err(|_| ParseError::Size(said.to_string()))?;
    let tall = tall.parse::<u32>().map_err(|_| ParseError::Size(said.to_string()))?;

    Ok(Size { width: wide, height: tall })
}

fn anchored(said: &str) -> Result<Anchor, ParseError> {
    match said {
        "top" => Ok(Anchor::Top),
        "top-right" => Ok(Anchor::TopRight),
        "bottom" => Ok(Anchor::Bottom),
        "whole" => Ok(Anchor::Whole),
        _ => Err(ParseError::Anchor(said.to_string())),
    }
}

fn driving() -> Result<String, Never> {
    Ok(match console_screen::here() {
        Ok(Some(screen)) => {
            let Ok(logical) = screen.logical();

            format!(
                "the panel this machine is driving is {} by {} at {}, so {} by {} points",
                screen.mode.width, screen.mode.height, screen.scale, logical.width, logical.height
            )
        }
        Ok(None) => "this machine names no panel it is driving".to_string(),
        Err(why) => format!("nothing could be asked about the panel: {why}"),
    })
}

fn said(surface: &Surface) -> Result<String, Never> {
    let Ok(logical) = surface.logical();
    let Ok(scale) = surface.scale();
    let Ok(many) = scale.hundred_twentieths();

    Ok(match logical {
        Some(logical) => {
            let Ok(device) = scale.device(logical);

            format!(
                "{} by {} points at {}/120, so a buffer {} by {}",
                logical.width, logical.height, many, device.width, device.height
            )
        }
        None => "the compositor has not said how large this is".to_string(),
    })
}

fn paint(pixels: &mut [u8], device: Size<u32>, _scale: Scale) -> Result<(), Never> {
    let Size { width: wide, height: tall } = device;
    let Ok(stride) = index(wide.saturating_mul(4));

    match stride == 0 {
        true => return Ok(()),
        false => {},
    }

    for (down, row) in pixels.chunks_exact_mut(stride).enumerate() {
        let Ok(down) = fitted::<_, u32>(down);

        for (across, pixel) in row.chunks_exact_mut(4).enumerate() {
            let Ok(across) = fitted::<_, u32>(across);
            let Ok(ink) = ink(At { x: across, y: down, width: wide, height: tall });

            pixel.copy_from_slice(match ink {
                HexColor::Edge => &EDGE,
                HexColor::Check => &CHECK,
                HexColor::Ground => &GROUND,
            });
        }
    }

    Ok(())
}

struct At {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn ink(at: At) -> Result<HexColor, Never> {
    let At { x: across, y: down, width: wide, height: tall } = at;
    let edge = across == 0
        || down == 0
        || across.saturating_add(1) == wide
        || down.saturating_add(1) == tall;

    Ok(match edge {
        true => HexColor::Edge,
        false => match across.saturating_add(down).checked_rem(2) {
            Some(0) => HexColor::Check,
            Some(_) | None => HexColor::Ground,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_asked_for_is_whatever_screen_it_lands_on() {
        let wanted = match asked(&[]) {
            Ok(wanted) => wanted,
            Err(why) => panic!("nothing asked for should read: {why}"),
        };

        assert_eq!(wanted.size, Size { width: 0, height: 0 });
        assert_eq!(wanted.anchor, Anchor::Whole);
    }

    #[test]
    fn a_size_is_read_off_the_command_line_rather_than_written_here() {
        let arguments = ["--size".to_string(), "320x44".to_string()];
        let wanted = match asked(&arguments) {
            Ok(wanted) => wanted,
            Err(why) => panic!("a size on the command line should read: {why}"),
        };

        assert_eq!(wanted.size, Size { width: 320, height: 44 });
    }

    #[test]
    fn a_size_no_one_can_read_says_so_rather_than_standing_somewhere_surprising() {
        let arguments = ["--size".to_string(), "enormous".to_string()];

        assert_eq!(asked(&arguments), Err(ParseError::Size("enormous".to_string())));
    }

    #[test]
    fn a_flag_with_nothing_after_it_is_not_a_default() {
        let arguments = ["--anchor".to_string()];

        assert_eq!(asked(&arguments), Err(ParseError::Without("--anchor")));
    }
}
