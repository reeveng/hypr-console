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
//! checker is an even grey; at the wrong one the border doubles or vanishes and
//! the checker moires, and both are visible in a photograph rather than by
//! argument.
//!
//! It asks for nothing by default, which is how it is the same probe on any
//! machine: a size of nought by nought anchored to all four edges is a layer
//! surface the size of whatever screen it landed on, and the numbers it prints
//! come back from the compositor rather than from this file. A size is only
//! worth naming when what is being pressed is a particular surface's shape.

use std::process::ExitCode;
use std::time::Duration;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_draw_surface::standing::{Anchor, Gone, Keyboard, Margin, Room, Under, Wanted};
use console_draw_surface::{Scale, Surface};

const NAMESPACE: &str = "console-draw-surface-spike";

const GROUND: [u8; 4] = [0x30, 0x28, 0x24, 0xff];

const EDGE: [u8; 4] = [0xf0, 0xf0, 0xf0, 0xff];

const CHECK: [u8; 4] = [0x90, 0x90, 0x90, 0xff];

const TURNS: u32 = 40;

enum Ink {
    Edge,
    Check,
    Ground,
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let wanted = match asked(&argv) {
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
            Ok(Gone::Yes) => return ExitCode::SUCCESS,
            Ok(Gone::No) | Err(_) => {}
        }

        left = left.saturating_sub(1);
    }

    ExitCode::SUCCESS
}

#[derive(Debug, PartialEq, Eq)]
pub enum Unreadable {
    Size(String),
    Anchor(String),
    Margin(String),
    Without(&'static str),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unreadable::Size(said) => {
                write!(to, "a size is written 320x120, and this says {said}")
            }
            Unreadable::Anchor(said) => write!(
                to,
                "an anchor is top, top-right, bottom or whole, and this says {said}"
            ),
            Unreadable::Margin(said) => write!(to, "a margin is a whole number, and this says {said}"),
            Unreadable::Without(flag) => write!(to, "{flag} was given nothing to read"),
        }
    }
}

impl std::error::Error for Unreadable {}

fn asked(argv: &[String]) -> Result<Wanted, Unreadable> {
    let mut wanted = Wanted {
        namespace: NAMESPACE.to_string(),
        anchor: Anchor::Whole,
        size: Size { wide: 0, tall: 0 },
        margin: Margin::default(),
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::Nothing,
    };
    let mut rest = argv.iter();

    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--size" => {
                let said = rest.next().ok_or(Unreadable::Without("--size"))?;

                let size = measured(said)?;

                wanted.size = size;
            }
            "--anchor" => {
                let said = rest.next().ok_or(Unreadable::Without("--anchor"))?;

                let anchor = anchored(said)?;

                wanted.anchor = anchor;
            }
            "--margin" => {
                let said = rest.next().ok_or(Unreadable::Without("--margin"))?;

                let margin =
                    said.parse::<i32>().map_err(|_| Unreadable::Margin(said.clone()))?;

                wanted.margin =
                    Margin { top: margin, right: margin, bottom: margin, left: margin };
            }
            _ => {}
        }
    }

    Ok(wanted)
}

fn measured(said: &str) -> Result<Size<u32>, Unreadable> {
    let (wide, tall) = said.split_once('x').ok_or_else(|| Unreadable::Size(said.to_string()))?;
    let wide = wide.parse::<u32>().map_err(|_| Unreadable::Size(said.to_string()))?;
    let tall = tall.parse::<u32>().map_err(|_| Unreadable::Size(said.to_string()))?;

    Ok(Size { wide, tall })
}

fn anchored(said: &str) -> Result<Anchor, Unreadable> {
    match said {
        "top" => Ok(Anchor::Top),
        "top-right" => Ok(Anchor::TopRight),
        "bottom" => Ok(Anchor::Bottom),
        "whole" => Ok(Anchor::Whole),
        _ => Err(Unreadable::Anchor(said.to_string())),
    }
}

fn driving() -> Result<String, Never> {
    Ok(match console_screen::here() {
        Ok(Some(screen)) => {
            let Ok(logical) = screen.logical();

            format!(
                "the panel this machine is driving is {} by {} at {}, so {} by {} points",
                screen.mode.wide, screen.mode.tall, screen.scale, logical.wide, logical.tall
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
                logical.wide, logical.tall, many, device.wide, device.tall
            )
        }
        None => "the compositor has not said how large this is".to_string(),
    })
}

fn paint(pixels: &mut [u8], device: Size<u32>, _scale: Scale) -> Result<(), Never> {
    let Ok(wide) = fitted::<u32, usize>(device.wide);
    let Ok(tall) = fitted::<u32, usize>(device.tall);
    let stride = wide.saturating_mul(4);

    match stride == 0 {
        true => return Ok(()),
        false => {},
    }

    for (down, row) in pixels.chunks_exact_mut(stride).enumerate() {
        for (across, pixel) in row.chunks_exact_mut(4).enumerate() {
            let Ok(ink) = ink(At { across, down, wide, tall });

            pixel.copy_from_slice(match ink {
                Ink::Edge => &EDGE,
                Ink::Check => &CHECK,
                Ink::Ground => &GROUND,
            });
        }
    }

    Ok(())
}

struct At {
    across: usize,
    down: usize,
    wide: usize,
    tall: usize,
}

fn ink(at: At) -> Result<Ink, Never> {
    let At { across, down, wide, tall } = at;
    let edge = across == 0
        || down == 0
        || across.saturating_add(1) == wide
        || down.saturating_add(1) == tall;

    Ok(match edge {
        true => Ink::Edge,
        false => match across.saturating_add(down).checked_rem(2) {
            Some(0) => Ink::Check,
            Some(_) | None => Ink::Ground,
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

        assert_eq!(wanted.size, Size { wide: 0, tall: 0 });
        assert_eq!(wanted.anchor, Anchor::Whole);
    }

    #[test]
    fn a_size_is_read_off_the_command_line_rather_than_written_here() {
        let argv = ["--size".to_string(), "320x44".to_string()];
        let wanted = match asked(&argv) {
            Ok(wanted) => wanted,
            Err(why) => panic!("a size on the command line should read: {why}"),
        };

        assert_eq!(wanted.size, Size { wide: 320, tall: 44 });
    }

    #[test]
    fn a_size_nobody_can_read_says_so_rather_than_standing_somewhere_surprising() {
        let argv = ["--size".to_string(), "enormous".to_string()];

        assert_eq!(asked(&argv), Err(Unreadable::Size("enormous".to_string())));
    }

    #[test]
    fn a_flag_with_nothing_after_it_is_not_a_default() {
        let argv = ["--anchor".to_string()];

        assert_eq!(asked(&argv), Err(Unreadable::Without("--anchor")));
    }
}
