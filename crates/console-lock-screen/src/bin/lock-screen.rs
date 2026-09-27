//! The screen a locked session is drawn behind.
//!
//!     lock-screen
//!
//! What is decided is `console_lock_screen` and the greeter it drives; this is
//! the lock the compositor holds and the surface the ring is drawn on. It is
//! `ext-session-lock-v1` rather than a surface over everything, because a
//! surface over everything is still a desktop underneath: a bound key reaches
//! it, a notification comes up beside it, and a program that dies takes the
//! lock away with it. Held this way the compositor shows nothing else until
//! the lock is let go, and a lock screen that falls leaves the session locked.
//!
//! It is started by `console-lock.service`, which is what the idle daemon asks
//! for before the machine sleeps, and systemd is told the start is done once
//! the compositor says the session is locked -- so the sleep waiting on it is
//! not taken with the desktop still on the glass. With no pattern chosen, or
//! the lock turned off in the settings, that is said at once and nothing is
//! locked. A pattern that cannot be read is said and not locked with either:
//! the unit would otherwise fail before it was ready and be started again
//! every second, and the settings already show the same fault on the row
//! where it can be mended.

use std::process::ExitCode;

use console_core_color::palette::{Wearing, WearingError};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_draw_painting::{Rendered, painter};
use console_draw_surface::{Closed, KeyboardEvent, Lock, Surface, SurfaceError, Unlocked};
use console_lock_screen::{Wanted, respond, desired};
use console_login_greeter::greeting::{Greeter, GreeterEffect, GreeterEvent, Greeting};
use console_login_greeter::picture::render;
use console_login_greeter::session::{press, touch};
use console_login_window::stored_pattern::{self, Hash, PatternStoreError};
use console_program_contract::{Arguments, Effect, Event, Exit, Initial, Program, Update};

const WHO: &str = "lock-screen";

const UNLOCK: &str = "Unlock";

enum LockScreenError {
    Homeless,
    Store(PatternStoreError),
    Palette(WearingError),
    Surface(SurfaceError),
    Ready(std::io::Error),
}

impl std::fmt::Display for LockScreenError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockScreenError::Homeless => write!(to, "there is no home to find a pattern in"),
            LockScreenError::Store(why) => write!(to, "{why}"),
            LockScreenError::Palette(why) => write!(to, "{why}"),
            LockScreenError::Surface(why) => write!(to, "no lock to hold: {why}"),
            LockScreenError::Ready(why) => write!(to, "cannot tell systemd the session is locked: {why}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notified {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Welcomed {
    Yes,
    No,
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

fn ran() -> Result<Result<(), LockScreenError>, Never> {
    let Ok(home) = console_core_places::home();
    let home = match home {
        Some(home) => home,
        None => return Ok(Err(LockScreenError::Homeless)),
    };
    let stored = match stored_pattern::stored(&home) {
        Ok(stored) => stored,
        Err(why) => {
            eprintln!("{WHO}: {}", LockScreenError::Store(why));

            return Ok(console_readiness::ready().map_err(LockScreenError::Ready));
        }
    };
    let Ok(chosen) = console_lock_screen::current();
    let Ok(wanted) = desired(stored, chosen);

    Ok(match wanted {
        Wanted::Lock(hash) => lock(&hash),
        Wanted::Absent => console_readiness::ready().map_err(LockScreenError::Ready),
    })
}

fn lock(hash: &Hash) -> Result<(), LockScreenError> {
    let worn = Wearing::worn().map_err(LockScreenError::Palette);
    let wearing = worn?;
    let connected = Surface::connect();
    let mut surface = connected.map_err(LockScreenError::Surface)?;
    let locking = surface.lock(WHO);
    let lock = locking.map_err(LockScreenError::Surface)?;

    match lock {
        Lock::Denied => return console_readiness::ready().map_err(LockScreenError::Ready),
        Lock::Waiting | Lock::Acquired => {}
    }

    let Ok(arguments) = Arguments::of(&[]);
    let Initial { state, subscriptions: _ } = Greeter::init(&arguments);
    let locked = LockBehavior { surface, greeting: state, rendered: Rendered::default(), notified: Notified::No, welcomed: Welcomed::No };

    let ended = iterate(locked, |locked| {
        Ok(match round(locked, &wearing, hash) {
            Ok(Step::Again(locked)) => Step::Again(locked),
            Ok(Step::Halt(())) => Step::Halt(Ok(())),
            Err(fault) => Step::Halt(Err(fault)),
        })
    });

    match ended {
        Ok(ended) => ended,
        Err(Endless) => Ok(()),
    }
}

struct LockBehavior {
    surface: Surface,
    greeting: Greeting,
    rendered: Rendered,
    notified: Notified,
    welcomed: Welcomed,
}

fn round(locked: LockBehavior, wearing: &Wearing, hash: &Hash) -> Result<Step<LockBehavior, ()>, LockScreenError> {
    let LockBehavior { mut surface, greeting, mut rendered, mut notified, welcomed } = locked;
    let Ok(()) = draw(&mut surface, &greeting, wearing, &mut rendered);
    let Ok(lock) = surface.locked();

    match (lock, notified) {
        (Lock::Acquired, Notified::No) => {
            let readied = console_readiness::ready();

            readied.map_err(LockScreenError::Ready)?;

            notified = Notified::Yes;
        }
        (Lock::Acquired, Notified::Yes) | (Lock::Waiting, Notified::Yes | Notified::No) => {}
        (Lock::Denied, Notified::Yes | Notified::No) => return Ok(Step::Halt(())),
    }

    match (lock, welcomed) {
        (Lock::Acquired, Welcomed::Yes) => {
            let unlocking = surface.unlock();
            let unlocked = unlocking.map_err(LockScreenError::Surface)?;

            match unlocked {
                Unlocked::Yes => return Ok(Step::Halt(())),
                Unlocked::NotYet => {}
            }
        }
        (Lock::Acquired, Welcomed::No) | (Lock::Waiting | Lock::Denied, Welcomed::Yes | Welcomed::No) => {}
    }

    let waited = surface.wait(&[], None);

    waited.map_err(LockScreenError::Surface)?;

    let Ok(closed) = surface.closed();

    match closed {
        Closed::Yes => return Ok(Step::Halt(())),
        Closed::No => {}
    }

    let Ok(keys) = surface.keyboard_events();
    let Ok(pointer) = surface.pointer_events();
    let Ok(logical) = surface.logical();
    let pressed = keys.into_iter().filter_map(|KeyboardEvent::Down { key }| {
        let Ok(press) = press(key);

        press.map(GreeterEvent::Pressed)
    });
    let touched = pointer.into_iter().filter_map(|event| {
        let Ok(touch) = touch(event, logical);

        touch.map(GreeterEvent::Touched)
    });
    let Ok((greeting, welcomed)) = answered(Answering { greeting, welcomed }, pressed.chain(touched).collect(), hash);

    Ok(Step::Again(LockBehavior { surface, greeting, rendered, notified, welcomed }))
}

struct Answering {
    greeting: Greeting,
    welcomed: Welcomed,
}

fn answered(answering: Answering, events: Vec<GreeterEvent>, hash: &Hash) -> Result<(Greeting, Welcomed), Never> {
    let unanswered = (answering.greeting.clone(), answering.welcomed);
    let ended = iterate((answering, events), |(answering, events)| answering_round(answering, events, hash));

    Ok(match ended {
        Ok(ended) => ended,
        Err(Endless) => unanswered,
    })
}

type AnsweringRound = Step<(Answering, Vec<GreeterEvent>), (Greeting, Welcomed)>;

fn answering_round(answering: Answering, events: Vec<GreeterEvent>, hash: &Hash) -> Result<AnsweringRound, Never> {
    let (Answering { greeting, welcomed }, replies) =
        events.into_iter().fold((answering, Vec::new()), |(Answering { greeting, mut welcomed }, mut replies), event| {
            let Update { state, effects } = Greeter::update(&greeting, &Event::Custom(event));

            for effect in effects {
                match effect {
                    Effect::Custom(GreeterEffect::Send(said)) => {
                        let Ok(reply) = respond(&said, hash);

                        replies.extend(reply.map(GreeterEvent::Received));
                    }
                    Effect::Stop(Exit::Success | Exit::Failure(_)) => welcomed = Welcomed::Yes,
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

            (Answering { greeting: state, welcomed }, replies)
        });

    Ok(match replies.is_empty() {
        true => Step::Halt((greeting, welcomed)),
        false => Step::Again((Answering { greeting, welcomed }, replies)),
    })
}

fn draw(surface: &mut Surface, greeting: &Greeting, wearing: &Wearing, rendered: &mut Rendered) -> Result<(), Never> {
    let Ok(logical) = surface.logical();
    let logical = match logical {
        Some(logical) => logical,
        None => return Ok(()),
    };
    let Ok(shapes) = render(greeting, wearing, logical, UNLOCK);

    let Ok(wanted) = rendered.wanted(shapes);

    match wanted {
        Some(shapes) => {
            let Ok(painting) = painter(logical, shapes, "lock-screen");
            let _ = surface.draw(painting);
        }
        None => {},
    }

    Ok(())
}
