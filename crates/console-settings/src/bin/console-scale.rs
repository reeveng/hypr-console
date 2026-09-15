//! How big everything on the screen is.
//!
//!     console-scale                 which size the screen is standing at
//!     console-scale smaller|normal|bigger
//!                                   put it there, and remember it
//!     console-scale apply           wear what was remembered
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
//! What `hyprctl` is told is `eval`, and that is not a preference. A
//! Lua-configured compositor answers `hyprctl keyword` with *"keyword can't
//! work with non-legacy parsers. Use eval."* -- the same trap `docs/screen.md`
//! describes for `dispatch`, where the command every example on the internet
//! gives comes back with a complaint and the only symptom is a setting that
//! does nothing.

use std::path::Path;
use std::process::ExitCode;

use console_core_atomic_writes::Held;
use console_compositor::Done;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_settings::size::{self, Size};

const BAR: &str = "console-bar.service";

const HOME: &str = "console-home.service";

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

    let Ok(at) = size::at(&home);
    let Ok(held) = console_core_atomic_writes::read(&at);

    let written = match held {
        Held::Said(said) => said,

        Held::Nothing => String::new(),

        Held::Unreadable(fault) => {
            eprintln!("console-scale: {}: {fault}", at.display());
            String::new()
        }
    };

    let Ok(remembered) = Size::of(&written);

    match word == "apply" {
        true => {
            let Ok(gone) = applied(&home, remembered);

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
                Some(size) => size,
                None => {
                    eprintln!("usage: console-scale [smaller|normal|bigger|apply]");
                    return ExitCode::from(2);
                }
            }
        }
    };

    let Ok(found) = panel();

    let screen = match found {
        Some(screen) => screen,
        None => return ExitCode::FAILURE,
    };

    match at.parent() {
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

            let Ok(written) = wanted.written();

            match console_core_atomic_writes::whole(&at, format!("{written}\n").as_bytes()) {
                Ok(()) => {},
                Err(fault) => {
                    eprintln!("console-scale: {}: {fault}, so nothing was changed", at.display());

                    return ExitCode::FAILURE;
                }
            }
        }
        None => {},
    }

    let Ok(()) = write_the_bar(&home, &screen, wanted);
    let Ok(refused) = refused(&screen, wanted);

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

fn applied(home: &Path, written: Option<Size>) -> Result<ExitCode, Never> {
    let Ok(found) = panel();

    let screen = match found {
        Some(screen) => screen,
        None => return Ok(ExitCode::SUCCESS),
    };

    let wearing = match written {
        Some(size) => size,
        None => Size::Normal,
    };

    let Ok(()) = write_the_bar(home, &screen, wearing);

    let why = match written {
        Some(size) => {
            let Ok(refused) = refused(&screen, size);

            refused
        }
        None => None,
    };

    match why {
        Some(why) => eprintln!("console-scale: {why}"),
        None => {},
    }

    Ok(ExitCode::SUCCESS)
}

fn panel() -> Result<Option<console_screen::Screen>, Never> {
    let found = match console_screen::here() {
        Ok(found) => found,
        Err(why) => {
            eprintln!("console-scale: {why}");

            return Ok(None);
        }
    };

    match found {
        Some(screen) => Ok(Some(screen)),
        None => {
            eprintln!("console-scale: the compositor is driving no screen to put a size on");

            Ok(None)
        }
    }
}

fn refused(screen: &console_screen::Screen, size: Size) -> Result<Option<String>, Never> {
    let Ok(scale) = size.scale_on(screen);
    let Ok(lua) = size::lua(screen, scale);
    let Ok(done) = console_compositor::told(console_compositor::Told::Eval, &lua);

    Ok(match done {
        Done::Taken => None,
        Done::Refused(why) => Some(format!("the compositor would not take it: {why}")),
    })
}

fn write_the_bar(home: &Path, screen: &console_screen::Screen, size: Size) -> Result<(), Never> {
    let Ok(at) = size::bar_at(home);

    let holding = match at.parent() {
        Some(holding) => holding,
        None => return Ok(()),
    };

    match std::fs::create_dir_all(holding) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-scale: {}: {fault}", holding.display());

            return Ok(());
        }
    }

    let Ok(scale) = size.scale_on(screen);
    let Ok(css) = console_screen::bar_css(screen, scale);

    match console_core_atomic_writes::whole(&at, css.as_bytes()) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-scale: {}: {fault}", at.display()),
    }

    Ok(())
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
