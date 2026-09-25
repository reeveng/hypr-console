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
use console_core_geometry::{Point, Size};
use console_core_shapes::Shape;
use console_core_never::Never;
use console_draw_painting::{self as painting, Frame};
use console_draw_surface::{Anchor, Closed, Keyboard, KeyboardEvent, Keysym, Margin, PointerEvent, Room, Surface, SurfaceError, Under, Wanted};
use console_input_event_devices::presses::ButtonPress;
use console_login_greeter::picture::{in_the_room, labelled};
use console_login_pattern::Touch;
use console_login_window::stored_pattern::{self, PatternStoreError};
use console_program_contract::{Arguments, Effect, Event, Exit, Initial, Program, Update};
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
        [] => chosen(&home),
        [word] => match word.as_str() {
            OFF => stored_pattern::remove(&home).map_err(LoginPatternError::Store),
            _ => Err(LoginPatternError::Arguments),
        },
        _ => Err(LoginPatternError::Arguments),
    })
}

fn chosen(home: &Path) -> Result<(), LoginPatternError> {
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
    let Initial { state, subscriptions: _ } = LoginPattern::init(&arguments);
    let mut drawing = state;
    let mut rendered: Option<Vec<Shape>> = None;

    loop {
        let Ok(()) = drawn(&mut surface, &drawing, &wearing, &mut rendered);
        let waited = surface.wait(&[], None);

        waited.map_err(LoginPatternError::Surface)?;

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return Ok(()),
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
        let events: Vec<LoginPatternEvent> = pressed.chain(touched).collect();

        for event in events {
            let turned = turned(&mut drawing, event, home);
            let flow = turned?;

            match flow {
                Flow::Continue => {}
                Flow::Finished => return Ok(()),
            }
        }
    }
}

fn touch(event: PointerEvent, logical: Option<Size<u32>>) -> Result<Option<Touch>, Never> {
    let room = |at: (f64, f64)| {
        logical.map(|canvas| {
            let Ok(room) = in_the_room(canvas, Point { x: at.0, y: at.1 });

            room
        })
    };

    Ok(match event {
        PointerEvent::Down { at } => room(at).map(Touch::Down),
        PointerEvent::Moved { at } => room(at).map(Touch::Moved),
        PointerEvent::Up => Some(Touch::Up),
        PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } | PointerEvent::Left => None,
    })
}

fn turned(drawing: &mut Drawing, event: LoginPatternEvent, home: &Path) -> Result<Flow, LoginPatternError> {
    let Update { state, effects } = LoginPattern::update(drawing, &Event::Custom(event));

    *drawing = state;

    for effect in effects {
        match effect {
            Effect::Custom(LoginPatternEffect::Save(secret)) => {
                let Ok(letters) = secret.spelled();
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

fn press(key: Keysym) -> Result<Option<ButtonPress>, Never> {
    Ok(match key {
        Keysym::Up => Some(ButtonPress::Up),
        Keysym::Down => Some(ButtonPress::Down),
        Keysym::Left => Some(ButtonPress::Left),
        Keysym::Right => Some(ButtonPress::Right),
        Keysym::Return | Keysym::KP_Enter | Keysym::space => Some(ButtonPress::Choose),
        Keysym::Escape | Keysym::BackSpace => Some(ButtonPress::Back),
        _ => None,
    })
}

fn drawn(surface: &mut Surface, drawing: &Drawing, wearing: &Wearing, rendered: &mut Option<Vec<Shape>>) -> Result<(), Never> {
    let Ok(logical) = surface.logical();
    let logical = match logical {
        Some(logical) => logical,
        None => return Ok(()),
    };
    let Ok(button) = drawing.button();
    let Ok(shapes) = labelled(&drawing.greeting, wearing, logical, button);

    match rendered.as_ref() == Some(&shapes) {
        true => return Ok(()),
        false => {}
    }

    let Ok(()) = surface.resize(logical);
    let _ = surface.draw(|pixels, device, _scale| {
        let frame = Frame { device, points: logical };
        let _ = painting::onto(pixels, frame, &shapes);

        Ok(())
    });

    *rendered = Some(shapes);

    Ok(())
}
