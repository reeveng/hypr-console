//! How big everything on the screen is.
//!
//!     console-scale                 which size the screen is standing at
//!     console-scale smaller|normal|bigger
//!                                   put it there, and remember it
//!     console-scale left|upright|right
//!                                   which way up it stands, and remember that
//!     console-scale apply           wear what was remembered
//!
//! The size and the way up are one eval and not two. A density here is a rung
//! divided into the width of the panel, and turning the panel changes which of
//! its two sides that is -- so a turn that did not carry the size with it would
//! leave the desktop at a density nobody chose, and the screen is described
//! whole or not at all.
//!
//! `apply` is the one the session runs. What a person chose is in a file of
//! their own and this puts it back on at every login, because a desktop that
//! forgot the size it was set to at every reboot would be a setting nobody could
//! rely on having made.
//!
//! The screen it is put on is the one the compositor says it is driving, and not
//! the one the tree declares. Those were the same thing for as long as there was
//! one machine: `declared()` reads a block written for a 1600x2560 panel turned
//! a quarter, and a laptop handed that through `eval` comes up rotated at two
//! and a half times the size, which is a session nobody can use rather than a
//! size nobody asked for. A machine with no compositor to ask is told so and
//! nothing is changed.
//!
//! **A rung is one screen's, in one shape.** Which of a panel's two sides is
//! its width is what a turn changes, so the same word is a different density
//! either way up and choosing a size after turning is saying what *this* way up
//! should be rather than correcting the other. The turn is read first, the turn
//! decides the shape, and the shape decides which rung is put on -- which is
//! also why turning back finds the size that was chosen there.
//!
//! A shape nobody has chosen a size in yet wears the other shape's word rather
//! than the density that is already on the screen. A rung is a canvas across,
//! so carrying the word over is what keeps everything the same size to an eye
//! through a turn, and carrying the density over is what makes a turn quietly
//! change the size as well -- which is the thing the turn was described whole
//! in one eval to avoid. The word is the answer somebody gave; the density is
//! arithmetic about a screen they have not seen yet.
//!
//! **An answer is one screen's.** A word typed here is about the panel the
//! compositor is driving -- the built-in one where there is one, which is the
//! screen somebody holding a handheld is looking at -- and it is kept under
//! that screen's own connector by `console_settings::screens`. `apply` is the
//! other half: it walks every screen the compositor has, puts each one back to
//! what was remembered for it, and leaves alone the ones nothing was ever
//! chosen for. A machine that kept one word for the whole of itself put the
//! handheld's quarter turn on the monitor somebody plugged into it, which is
//! not a setting behaving oddly -- it is one answer being asked of two
//! different questions.
//!
//! What `hyprctl` is told is `eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."* -- the same trap `docs/screen.md`
//! describes for `dispatch`, where the command every example on the internet
//! gives comes back with a complaint and the only symptom is a setting that
//! does nothing.

use std::process::ExitCode;

use console_core_atomic_writes::Held;
use console_compositor::Done;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_settings::screens::{Output, Unnamed};
use console_settings::size::{self, Size};
use console_settings::turning::{self, Turn};

const BAR: &str = "console-bar.service";

const HOME: &str = "console-home.service";

enum Wanted {
    Sized(Size),
    Turned(Turn),
}

fn kept(at: &std::path::Path) -> Result<String, Never> {
    let Ok(held) = console_core_atomic_writes::read(at);

    Ok(match held {
        Held::Said(said) => said,

        Held::Nothing => String::new(),

        Held::Unreadable(fault) => {
            eprintln!("console-scale: {}: {fault}", at.display());

            String::new()
        }
    })
}

fn main() -> ExitCode {
    let word = match std::env::args().nth(1) {
        Some(word) => word,
        None => String::new(),
    };

    let Ok(said) = console_core_places::home();

    let home = match said {
        Some(home) => home,
        None => {
            eprintln!("console-scale: no HOME, so there is nobody to remember for");

            return ExitCode::FAILURE;
        }
    };

    match word == "apply" {
        true => {
            let Ok(gone) = applied(&home);

            return gone;
        },
        false => {},
    }

    let wanted = match word.as_str() {
        "" => {
            let Ok(monitors) = asked();

            let monitors = match monitors {
                Some(monitors) => monitors,
                None => return ExitCode::FAILURE,
            };

            let Ok(standing) = size::standing(&monitors);
            let Ok(shown) = console_screen::shown(&monitors);

            match (standing, shown) {
                (Some(size), _) => {
                    let Ok(written) = size.written();

                    println!("{written}");
                }
                (None, Some(screen)) => println!("{}", screen.scale),
                (None, None) => {
                    eprintln!("console-scale: the compositor said nothing about a screen");
                    return ExitCode::FAILURE;
                }
            }

            return ExitCode::SUCCESS;
        }
        said => {
            let Ok(named) = Size::of(said);

            match named {
                Some(size) => Wanted::Sized(size),
                None => {
                    let Ok(turn) = Turn::of(said);

                    match turn {
                        Some(turn) => Wanted::Turned(turn),
                        None => {
                            eprintln!(
                                "usage: console-scale [smaller|normal|bigger|left|upright|right|apply]"
                            );

                            return ExitCode::from(2);
                        }
                    }
                }
            }
        }
    };

    let Ok(found) = panel();

    let (named, screen) = match found {
        Some(panel) => panel,
        None => return ExitCode::FAILURE,
    };

    let wanted_turn = match wanted {
        Wanted::Turned(turn) => Some(turn),
        Wanted::Sized(_a_rung) => None,
    };

    let kept = match remembers(&home, Output(&named), &screen, wanted_turn) {
        Ok(kept) => kept,
        Err(why) => {
            eprintln!("console-scale: {why}, so there is nowhere to remember this");

            return ExitCode::FAILURE;
        }
    };

    let (size, turn) = match wanted {
        Wanted::Sized(size) => (Some(size), kept.turn),
        Wanted::Turned(turn) => (kept.size, Some(turn)),
    };

    let (remembering, said) = match wanted {
        Wanted::Sized(size) => {
            let Ok(written) = size.written();

            (kept.size_at, written.to_string())
        }
        Wanted::Turned(turn) => {
            let Ok(written) = turn.written();

            (kept.turn_at, written.to_string())
        }
    };

    match remembering.parent() {
        Some(holding) => {
            match std::fs::create_dir_all(holding) {
                Ok(()) => {},
                Err(fault) => {
                    eprintln!(
                        "console-scale: {}: {fault}, so nothing was changed",
                        holding.display()
                    );

                    return ExitCode::FAILURE;
                }
            }

            match console_core_atomic_writes::whole(&remembering, format!("{said}\n").as_bytes()) {
                Ok(()) => {},
                Err(fault) => {
                    eprintln!(
                        "console-scale: {}: {fault}, so nothing was changed",
                        remembering.display()
                    );

                    return ExitCode::FAILURE;
                }
            }
        }
        None => {},
    }

    let Ok(refused) = refused(&named, &screen, size, turn);

    match refused {
        Some(why) => {
            eprintln!("console-scale: {why}");
            return ExitCode::FAILURE;
        }
        None => {},
    }

    let Ok(mut asking) = Program::Systemctl.command();
    let _ = asking.args(["--user", "restart", BAR, HOME]).status();
    ExitCode::SUCCESS
}

struct Remembers {
    size: Option<Size>,
    turn: Option<Turn>,
    size_at: std::path::PathBuf,
    turn_at: std::path::PathBuf,
}

fn remembers(
    home: &std::path::Path,
    panel: Output<'_>,
    screen: &console_screen::Screen,
    wanted: Option<Turn>,
) -> Result<Remembers, Unnamed> {
    let turn_at = turning::at(home, panel)?;
    let Ok(written) = kept(&turn_at);
    let Ok(turn) = Turn::of(&written);

    let standing = match wanted.or(turn) {
        Some(turn) => {
            let Ok(turned) = turning::turned(screen, turn);

            turned
        }
        None => *screen,
    };
    let Ok(shape) = standing.shape();

    let size_at = size::at(home, panel, shape)?;
    let Ok(written) = kept(&size_at);
    let Ok(said) = Size::of(&written);

    let size = match said {
        Some(size) => Some(size),
        None => {
            let Ok(other) = shape.other();
            let at = size::at(home, panel, other)?;
            let Ok(written) = kept(&at);
            let Ok(said) = Size::of(&written);

            said
        }
    };

    Ok(Remembers { size, turn, size_at, turn_at })
}

fn applied(home: &std::path::Path) -> Result<ExitCode, Never> {
    let Ok(said) = asked();

    let said = match said {
        Some(said) => said,
        None => return Ok(ExitCode::SUCCESS),
    };
    let Ok(monitors) = console_compositor::monitors(&said);

    for monitor in &monitors {
        let Ok(driving) = console_screen::driving(monitor);

        let screen = match driving {
            Some(screen) => screen,
            None => continue,
        };

        let kept = match remembers(home, Output(&monitor.named), &screen, None) {
            Ok(kept) => kept,
            Err(why) => {
                eprintln!("console-scale: {why}, so nothing was remembered for it");

                continue;
            }
        };

        match (kept.size, kept.turn) {
            (None, None) => continue,
            (size, turn) => {
                let Ok(why) = refused(&monitor.named, &screen, size, turn);

                match why {
                    Some(why) => eprintln!("console-scale: {}: {why}", monitor.named),
                    None => {},
                }
            }
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn panel() -> Result<Option<(String, console_screen::Screen)>, Never> {
    let found = match console_screen::driving_here() {
        Ok(found) => found,
        Err(why) => {
            eprintln!("console-scale: {why}");

            return Ok(None);
        }
    };

    match found {
        Some(panel) => Ok(Some(panel)),
        None => {
            eprintln!("console-scale: the compositor is driving no screen to put a size on");

            Ok(None)
        }
    }
}

fn refused(
    named: &str,
    screen: &console_screen::Screen,
    size: Option<Size>,
    turn: Option<Turn>,
) -> Result<Option<String>, Never> {
    let standing = match turn {
        Some(turn) => {
            let Ok(turned) = turning::turned(screen, turn);

            turned
        }
        None => *screen,
    };
    let scale = match size {
        Some(size) => {
            let Ok(scale) = size.scale_on(&standing);

            scale
        }
        None => standing.scale,
    };
    let Ok(lua) = size::lua(Output(named), &standing, scale);
    let Ok(done) = console_compositor::told(console_compositor::Told::Eval, &lua);

    Ok(match done {
        Done::Taken => None,
        Done::Refused(why) => Some(format!("the compositor would not take it: {why}")),
    })
}

fn asked() -> Result<Option<serde_json::Value>, Never> {
    Ok(match console_compositor::asked(console_compositor::Asked::Monitors) {
        Ok(monitors) => Some(monitors),
        Err(why) => {
            eprintln!("console-scale: {why}");

            None
        }
    })
}
