//! The pattern the login window asks for, chosen or forgotten.
//!
//!     settings-login-pattern
//!     settings-login-pattern off
//!
//! What is decided is `console_settings::login`; this is the surface it is
//! drawn on and the file it ends in. It is drawn over the desktop the way a
//! panel is, so the pad arrives as the keys every panel already reads and a
//! finger as the touches every panel already hears, and the picture is the
//! greeter's own, so the dots are where they will be at login.

use std::path::Path;
use std::process::ExitCode;

use console_core_color::palette::{Wearing, WearingError};
use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_never::Never;
use console_draw_painting::{Rendered, painter};
use console_draw_surface::{Anchor, Closed, Keyboard, KeyboardEvent, Margin, Room, Surface, SurfaceError, Under, Wanted};
use console_login_greeter::picture::render;
use console_login_greeter::session::{press, touch};
use console_login_window::stored_pattern::{self, PatternStoreError};
use console_core_state_machine::{Machine, Transition};
use console_program_contract::{Arguments, Effect, Event, Exit};
use console_settings::login::{Drawing, LoginPattern, LoginPatternEffect, LoginPatternEvent};

const WHO: &str = "settings-login-pattern";

const OFF: &str = "off";

enum LoginPatternError {
    Arguments,
    Homeless,
    Palette(WearingError),
    Surface(SurfaceError),
    Store(PatternStoreError),
}

impl std::fmt::Display for LoginPatternError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginPatternError::Arguments => write!(to, "usage: {WHO} [{OFF}]"),
            LoginPatternError::Homeless => write!(to, "there is no home to keep a pattern in"),
            LoginPatternError::Palette(why) => write!(to, "{why}"),
            LoginPatternError::Surface(why) => write!(to, "no surface to draw on: {why}"),
            LoginPatternError::Store(why) => write!(to, "{why}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Finished,
}

fn main() -> ExitCode {
    let Ok(ran) = ran();

    match ran {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{WHO}: {why}");

            ExitCode::FAILURE
        }
    }
}

fn ran() -> Result<Result<(), LoginPatternError>, Never> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Ok(home) = console_core_places::home();
    let home = match home {
        Some(home) => home,
        None => return Ok(Err(LoginPatternError::Homeless)),
    };

    Ok(match words.as_slice() {
        [] => run(&home),
        [word] => match word.as_str() {
            OFF => stored_pattern::remove(&home).map_err(LoginPatternError::Store),
            _ => Err(LoginPatternError::Arguments),
        },
        _ => Err(LoginPatternError::Arguments),
    })
}

fn run(home: &Path) -> Result<(), LoginPatternError> {
    let worn = Wearing::worn().map_err(LoginPatternError::Palette);
    let wearing = worn?;
    let connected = Surface::connect();
    let mut surface = connected.map_err(LoginPatternError::Surface)?;
    let wanted = Wanted {
        namespace: WHO.to_string(),
        anchor: Anchor::Whole,
        size: Size { width: 0, height: 0 },
        margin: Margin::default(),
        keyboard: Keyboard::Takes,
        room: Room::Over,
        under: Under::None,
    };
    let shown = surface.show(&wanted);

    shown.map_err(LoginPatternError::Surface)?;

    let Ok(arguments) = Arguments::of(&[]);
    let Ok(Transition { state, effects: _ }) = LoginPattern::initial_transition(&arguments, None);
    let turned = console_core_iteration::iterate((surface, state, Rendered::default()), |(mut surface, mut drawing, mut rendered)| {
        Ok(match turn(&mut surface, &mut drawing, &mut rendered, (&wearing, home)) {
            Ok(Flow::Continue) => Step::Again((surface, drawing, rendered)),
            Ok(Flow::Finished) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match turned {
        Ok(turned) => turned,
        Err(_endless) => Ok(()),
    }
}

fn turn(
    surface: &mut Surface,
    drawing: &mut Drawing,
    rendered: &mut Rendered,
    (wearing, home): (&Wearing, &Path),
) -> Result<Flow, LoginPatternError> {
    let Ok(()) = draw(surface, drawing, wearing, rendered);
    let waited = surface.wait(&[], None);

    waited.map_err(LoginPatternError::Surface)?;

    let Ok(closed) = surface.closed();

    match closed {
        Closed::Yes => return Ok(Flow::Finished),
        Closed::No => {}
    }

    let Ok(keys) = surface.keyboard_events();
    let Ok(pointer) = surface.pointer_events();
    let Ok(logical) = surface.logical();
    let pressed = keys.into_iter().filter_map(|KeyboardEvent::Down { key }| {
        let Ok(press) = press(key);

        press.map(LoginPatternEvent::Pressed)
    });
    let touched = pointer.into_iter().filter_map(|event| {
        let Ok(touch) = touch(event, logical);

        touch.map(LoginPatternEvent::Touched)
    });
    let mut flow = Flow::Continue;

    for event in pressed.chain(touched) {
        flow = match flow {
            Flow::Finished => Flow::Finished,
            Flow::Continue => apply_event(drawing, event, home)?,
        };
    }

    Ok(flow)
}

fn apply_event(drawing: &mut Drawing, event: LoginPatternEvent, home: &Path) -> Result<Flow, LoginPatternError> {
    let Ok(Transition { state, effects }) = LoginPattern::transition(drawing.clone(), Event::Custom(event));

    *drawing = state;

    for effect in effects {
        match effect {
            Effect::Custom(LoginPatternEffect::Save(secret)) => {
                let Ok(letters) = secret.as_str();
                let hashed = stored_pattern::hashed(letters);
                let hash = hashed.map_err(LoginPatternError::Store)?;
                let stored = stored_pattern::store(home, &hash);

                stored.map_err(LoginPatternError::Store)?;
            }
            Effect::Stop(Exit::Success | Exit::Failure(_)) => return Ok(Flow::Finished),
            Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Spawn(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
            | Effect::Write(_)
            | Effect::Notify(_)
            | Effect::Print(_) => {}
        }
    }

    Ok(Flow::Continue)
}

fn draw(surface: &mut Surface, drawing: &Drawing, wearing: &Wearing, rendered: &mut Rendered) -> Result<(), Never> {
    let Ok(logical) = surface.logical();
    let logical = match logical {
        Some(logical) => logical,
        None => return Ok(()),
    };
    let Ok(button) = drawing.button();
    let Ok(shapes) = render(&drawing.greeting, wearing, logical, button);

    let Ok(wanted) = rendered.wanted(shapes);

    match wanted {
        Some(shapes) => {
            let Ok(()) = surface.resize(logical);
            let Ok(painting) = painter(logical, shapes, "settings-login-pattern");
            let _ = surface.draw(painting);
        }
        None => {},
    }

    Ok(())
}
