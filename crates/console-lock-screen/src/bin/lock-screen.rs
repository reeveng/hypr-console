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
//!
//! The bar over the ring is drawn on the same surface, because nothing else
//! comes up over a held lock: the clock, the sound and the battery, read when
//! the lock is taken and again as the minute turns, and the row that puts the
//! machine to sleep or stops it. What a press on it asks for is run here, as
//! the person whose session is locked.

use std::process::ExitCode;

use console_core_color::palette::{self, Wearing, WearingError};
use console_core_geometry::Size;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_draw_painting::{Rendered, painter};
use console_draw_surface::{Closed, KeyboardEvent, Lock, Surface, SurfaceError, Unlocked};
use console_lock_screen::{Wanted, respond, desired};
use console_login_greeter::bar;
use console_login_greeter::greeting::{GreeterEffect, GreeterEvent, GreeterState, Greeter, Status};
use console_login_greeter::picture::{Worn, over_a_session};
use console_login_greeter::session::{heard, press};
use console_login_window::stored_pattern::{self, Hash, PatternStoreError};
use console_core_state_machine::{Machine, Transition};
use console_program_contract::{Effect, Event, Exit};
use console_status_bar::clock::{self, Standing};
use console_status_bar::showing;

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

fn worn() -> Result<(Wearing, showing::Wearing), WearingError> {
    let spent = palette::spent()?;
    let ring = Wearing::out_of(&spent)?;
    let bar = showing::Wearing::out_of(&spent)?;

    Ok((ring, bar))
}

fn lock(hash: &Hash) -> Result<(), LockScreenError> {
    let worn = worn().map_err(LockScreenError::Palette);
    let (ring, bar) = worn?;
    let worn = Worn { ring: &ring, bar: &bar };
    let connected = Surface::connect();
    let mut surface = connected.map_err(LockScreenError::Surface)?;
    let locking = surface.lock(WHO);
    let lock = locking.map_err(LockScreenError::Surface)?;

    match lock {
        Lock::Denied => return console_readiness::ready().map_err(LockScreenError::Ready),
        Lock::Waiting | Lock::Acquired => {}
    }

    let Ok(Transition { state, effects: _ }) = Greeter::initial_transition(&Status::Waiting, None);
    let Ok(standing) = clock::current();
    let Ok(read_out) = bar::read_all();
    let Ok((state, _welcomed)) = answered(Answering { state, welcomed: Welcomed::No }, read_out, hash);
    let nothing_drawn = showing::Rendered { shapes: Vec::new(), room: Size { width: 0, height: 0 }, touching: Vec::new() };
    let locked = LockBehavior {
        surface,
        state,
        standing,
        rendered: Rendered::default(),
        bar: nothing_drawn,
        notified: Notified::No,
        welcomed: Welcomed::No,
    };

    let ended = iterate(locked, |locked| {
        Ok(match round(locked, worn, hash) {
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
    state: GreeterState,
    standing: Standing,
    rendered: Rendered,
    bar: showing::Rendered,
    notified: Notified,
    welcomed: Welcomed,
}

fn round(locked: LockBehavior, worn: Worn<'_>, hash: &Hash) -> Result<Step<LockBehavior, ()>, LockScreenError> {
    let LockBehavior { mut surface, state, standing, mut rendered, bar, mut notified, welcomed } = locked;
    let Ok(bar) = draw(&mut surface, Drawing { state: &state, worn, rendered: &mut rendered, bar });
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

    let Ok(patience) = clock::until_the_minute_turns();
    let waited = surface.wait(&[], Some(patience));

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
        let Ok(heard) = heard(event, logical, &bar);

        heard
    });
    let Ok((standing, read_out)) = bar::again(standing);
    let Ok((state, welcomed)) = answered(Answering { state, welcomed }, pressed.chain(touched).chain(read_out).collect(), hash);

    Ok(Step::Again(LockBehavior { surface, state, standing, rendered, bar, notified, welcomed }))
}

struct Answering {
    state: GreeterState,
    welcomed: Welcomed,
}

fn answered(answering: Answering, events: Vec<GreeterEvent>, hash: &Hash) -> Result<(GreeterState, Welcomed), Never> {
    let unanswered = (answering.state.clone(), answering.welcomed);
    let ended = iterate((answering, events), |(answering, events)| answering_round(answering, events, hash));

    Ok(match ended {
        Ok(ended) => ended,
        Err(Endless) => unanswered,
    })
}

type AnsweringRound = Step<(Answering, Vec<GreeterEvent>), (GreeterState, Welcomed)>;

fn answering_round(answering: Answering, events: Vec<GreeterEvent>, hash: &Hash) -> Result<AnsweringRound, Never> {
    let (Answering { state, welcomed }, replies) =
        events.into_iter().fold((answering, Vec::new()), |(Answering { state, mut welcomed }, mut replies), event| {
            let Ok(Transition { state, effects }) = Greeter::transition(state, Event::Custom(event));

            for effect in effects {
                match effect {
                    Effect::Custom(GreeterEffect::Send(said)) => {
                        let Ok(reply) = respond(&said, hash);

                        replies.extend(reply.map(GreeterEvent::Received));
                    }
                    Effect::Custom(GreeterEffect::Bar(asked)) => {
                        let Ok(heard) = bar::carried(asked);

                        replies.extend(heard);
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

            (Answering { state, welcomed }, replies)
        });

    Ok(match replies.is_empty() {
        true => Step::Halt((state, welcomed)),
        false => Step::Again((Answering { state, welcomed }, replies)),
    })
}

struct Drawing<'a> {
    state: &'a GreeterState,
    worn: Worn<'a>,
    rendered: &'a mut Rendered,
    bar: showing::Rendered,
}

fn draw(surface: &mut Surface, drawing: Drawing<'_>) -> Result<showing::Rendered, Never> {
    let Drawing { state, worn, rendered, bar } = drawing;
    let Ok(logical) = surface.logical();
    let logical = match logical {
        Some(logical) => logical,
        None => return Ok(bar),
    };
    let Ok((shapes, bar)) = over_a_session(state, worn, logical, UNLOCK);

    let Ok(wanted) = rendered.wanted(shapes);

    match wanted {
        Some(shapes) => {
            let Ok(painting) = painter(logical, shapes, "lock-screen");
            let _ = surface.draw(painting);
        }
        None => {},
    }

    Ok(bar)
}
