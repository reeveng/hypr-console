//! The screen a locked session is drawn behind.
//!
//!     lock-screen
//!     lock-screen idle
//!
//! What is decided is `console_lock_screen` and the greeter it drives; this is
//! the lock the compositor holds and the surface the ring is drawn on. It is
//! `ext-session-lock-v1` rather than a surface over everything, because a
//! surface over everything is still a desktop underneath: a bound key reaches
//! it, a notification comes up beside it, and a program that dies takes the
//! lock away with it. Held this way the compositor shows nothing else until
//! the lock is let go, and a lock screen that falls leaves the session locked.
//!
//! It is started by `console-lock.service`, and systemd is told the start is
//! done once the compositor says the session is locked. With no pattern chosen, or
//! the lock turned off in the settings, that is said at once and nothing is
//! locked. If the saved pattern cannot be read, the compositor still locks
//! and the screen offers any other configured method.
//!
//! The bar over the ring is drawn on the same surface, because nothing else
//! comes up over a held lock: the clock, the sound and the battery, read when
//! the lock is taken and again as the minute turns, and the row that puts the
//! machine to sleep or stops it. What a press on it asks for is run here, as
//! the person whose session is locked.

use std::process::ExitCode;

use console_core_color::palette::{self, Wearing, WearingError};
use console_core_geometry::Point;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_shapes::{Edge, Panel, Round, Shape};
use console_core_size::Size;
use console_core_state_machine::{Machine, Transition};
use console_draw_painting::{Rendered, painter};
use console_draw_surface::{Closed, Lock, Surface, SurfaceError, Unlocked};
use console_lock_screen::{LockScreen, Method, PATTERN_UNAVAILABLE, Wanted, desired, respond};
use console_login_greeter::bar;
use console_login_greeter::greeting::{
  Greeter, GreeterEffect, GreeterEvent, GreeterState, Status,
};
use console_login_greeter::picture::{Worn, over_a_session};
use console_login_greeter::session::{heard, press};
use console_login_window::stored_pattern::{self, Hash, PatternAttempts};
use console_program_contract::{Effect, Event, Exit};
use console_status_bar::clock::{self, Standing};
use console_status_bar::showing;

const WHO: &str = "lock-screen";

#[path = "lock-screen/idle_daemon/mod.rs"]
mod idle_daemon;
#[path = "lock-screen/unlock.rs"]
mod unlock;

use unlock::{Confirmation, InputDisposition, Unlock};

const UNLOCK: &str = "Unlock";

enum LockScreenError {
  Homeless,
  Denied,
  Palette(WearingError),
  Surface(SurfaceError),
  Ready(std::io::Error),
}

impl std::fmt::Display for LockScreenError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LockScreenError::Homeless => write!(to, "there is no home to find a pattern in"),
      LockScreenError::Denied => write!(to, "the compositor did not hold the session lock"),
      LockScreenError::Palette(why) => write!(to, "{why}"),
      LockScreenError::Surface(why) => write!(to, "no lock to hold: {why}"),
      LockScreenError::Ready(why) => {
        write!(to, "cannot tell systemd the session is locked: {why}")
      }
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
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let options = arguments.split_first();
  let command = options.map(|(mode, rest)| (mode.to_str(), rest));
  let result = match command {
    None => {
      let Ok(status) = show_lock();
      return status;
    }
    Some((Some("idle"), [])) => idle_daemon::run(),
    Some(_) => {
      eprintln!("Usage: {WHO} [idle]");
      return ExitCode::FAILURE;
    }
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("{WHO}: {fault}");
      ExitCode::FAILURE
    }
  }
}

#[must_use]
fn show_lock() -> core::result::Result<ExitCode, Never> {
  let Ok(ran) = ran();

  Ok(match ran {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("{WHO}: {why}");

      ExitCode::FAILURE
    }
  })
}

fn ran() -> Result<Result<(), LockScreenError>, Never> {
  let Ok(chosen) = console_lock_screen::current();

  match { ::core::primitive::bool::then_some(chosen == LockScreen::Off, ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(console_readiness::ready().map_err(LockScreenError::Ready));
    }
    ::core::option::Option::None => {}
  }

  let Ok(home) = console_core_places::home();
  let home_3 = match home {
    Some(home_2) => home_2,
    None => return Ok(Err(LockScreenError::Homeless)),
  };
  let stored = match stored_pattern::stored(&home_3) {
    Ok(stored) => stored,
    Err(why) => {
      eprintln!("{WHO}: {why}");

      return Ok(lock(None));
    }
  };
  let Ok(wanted) = desired(stored, chosen);

  Ok(match wanted {
    Wanted::Lock(hash) => lock(Some(&hash)),
    Wanted::Absent => console_readiness::ready().map_err(LockScreenError::Ready),
  })
}

fn worn() -> Result<(Wearing, showing::Wearing), WearingError> {
  let spent = palette::spent()?;
  let ring = Wearing::out_of(&spent)?;
  let bar = showing::Wearing::out_of(&spent)?;

  Ok((ring, bar))
}

fn lock(hash: Option<&Hash>) -> Result<(), LockScreenError> {
  let connected = Surface::connect();
  let mut surface = connected.map_err(LockScreenError::Surface)?;
  let locking = surface.lock(WHO);
  let lock = locking.map_err(LockScreenError::Surface)?;

  match lock {
    Lock::Denied => return Err(LockScreenError::Denied),
    Lock::Waiting | Lock::Acquired => {}
  }

  let (ring, bar) = worn().map_err(LockScreenError::Palette)?;
  let worn_2 = Worn {
    ring: &ring,
    bar: &bar,
  };
  let opening = match hash {
    Some(_) => Status::Waiting,
    None => Status::Message(PATTERN_UNAVAILABLE.to_owned()),
  };
  let Ok(Transition { state, effects: _ }) = Greeter::initial_transition(&opening, None);
  let Ok(standing) = clock::current();
  let Ok(preferred) = console_lock_screen::preferred();
  let Ok(read_out) = bar::read_all();
  let Ok(greeting) = answered(
    Answering {
      state,
      welcomed: Welcomed::No,
      attempts: PatternAttempts::default(),
    },
    read_out,
    hash,
  );
  let nothing_drawn = showing::Rendered {
    shapes: Vec::new(),
    room: Size {
      width: 0,
      height: 0,
    },
    touching: Vec::new(),
  };
  let locked = LockBehavior {
    surface,
    greeting,
    standing,
    rendered: Rendered::default(),
    bar: nothing_drawn,
    notified: Notified::No,
    unlock: ({
      let Ok(explicit_value) = Unlock::new(preferred);
      explicit_value
    }),
  };

  let ended = iterate(locked, |locked_2| {
    Ok(match round(locked_2, worn_2, hash) {
      Ok(Step::Again(locked_3)) => Step::Again(locked_3),
      Ok(Step::Halt(())) => Step::Halt(Ok(())),
      Err(fault) => Step::Halt(Err(fault)),
    })
  });

  match ended {
    Ok(ended_2) => ended_2,
    Err(Endless) => Ok(()),
  }
}

struct LockBehavior {
  surface: Surface,
  greeting: Answering,
  standing: Standing,
  rendered: Rendered,
  bar: showing::Rendered,
  notified: Notified,
  unlock: Unlock,
}

fn round(
  locked: LockBehavior,
  worn: Worn<'_>,
  hash: Option<&Hash>,
) -> Result<Step<LockBehavior, ()>, LockScreenError> {
  let LockBehavior {
    mut surface,
    mut greeting,
    standing,
    mut rendered,
    bar,
    mut notified,
    mut unlock,
  } = locked;
  let Ok(bar_2) = draw(
    &mut surface,
    Drawing {
      state: &greeting.state,
      worn,
      rendered: &mut rendered,
      bar,
      unlock: &unlock,
    },
  );
  let Ok(lock) = surface.locked();

  match (lock, notified) {
    (Lock::Acquired, Notified::No) => {
      let readied = console_readiness::ready();

      readied.map_err(LockScreenError::Ready)?;

      notified = Notified::Yes;
      let Ok(()) = unlock.acquired();
    }
    (Lock::Acquired, Notified::Yes) | (Lock::Waiting, Notified::Yes | Notified::No) => {}
    (Lock::Denied, Notified::Yes | Notified::No) => return Err(LockScreenError::Denied),
  }

  match (lock, greeting.welcomed) {
    (Lock::Acquired, Welcomed::Yes) => {
      let unlocking = surface.unlock();
      let unlocked = unlocking.map_err(LockScreenError::Surface)?;

      match unlocked {
        Unlocked::Yes => return Ok(Step::Halt(())),
        Unlocked::NotYet => {}
      }
    }
    (Lock::Acquired, Welcomed::No)
    | (Lock::Waiting | Lock::Denied, Welcomed::Yes | Welcomed::No) => {}
  }

  let Ok(patience) = clock::until_the_minute_turns();
  let waited = surface.wait(
    &({
      let Ok(explicit_value) = unlock.descriptors();
      explicit_value
    }),
    Some(patience),
  );

  waited.map_err(LockScreenError::Surface)?;

  let Ok(closed) = surface.closed();

  match closed {
    Closed::Yes => return Err(LockScreenError::Denied),
    Closed::No => {}
  }

  let Ok(keys) = surface.key_presses();
  let Ok(pointer) = surface.pointer_events();
  let Ok(logical) = surface.logical();
  let mut events = Vec::new();
  let Ok(held) = surface.locked();

  match {
    ::core::primitive::bool::then_some(notified == Notified::Yes && held == Lock::Acquired, ())
  } {
    ::core::option::Option::Some(()) => {
      let Ok(method) = unlock.method();
      let state = &mut greeting.state;

      for key in keys {
        let Ok(disposition) = match state.bar.focus {
          Some(_) => Ok(InputDisposition::ForwardToGreeter),
          None => {
            let Ok(focus) = state.pattern_focus();
            unlock.key(key, focus)
          }
        };

        match disposition {
          InputDisposition::ForwardToGreeter => {
            let Ok(pressed) = press(key.key);
            events.extend(pressed.map(GreeterEvent::Pressed));
          }
          InputDisposition::Handled => {}
        }
      }

      for event in pointer {
        let Ok(heard) = heard(event, logical, &bar_2);
        let Ok(disposition) = match heard {
          Some(GreeterEvent::Bar(_)) => Ok(InputDisposition::ForwardToGreeter),
          Some(
            GreeterEvent::Touched(_) | GreeterEvent::Pressed(_) | GreeterEvent::Received(_),
          )
          | None => unlock.pointer(event, logical),
        };

        match disposition {
          InputDisposition::ForwardToGreeter => {
            events.extend(heard);
          }
          InputDisposition::Handled => {}
        }
      }

      match {
        ::core::primitive::bool::then_some(
          method
            != ({
              let Ok(explicit_value) = unlock.method();
              explicit_value
            }),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          let Ok(pattern) = state.greeting.pattern.cleared();
          state.greeting.pattern = pattern;
          events.retain(|event| matches!(event, GreeterEvent::Bar(_)));
        }
        ::core::option::Option::None => {}
      }

      let Ok(confirmation) = unlock.verified();

      match confirmation {
        Confirmation::Accepted => {
          greeting.welcomed = Welcomed::Yes;
        }
        Confirmation::AwaitingInput => {}
      }
    }
    ::core::option::Option::None => {}
  }

  let Ok((standing_2, read_out)) = bar::again(standing);
  events.extend(read_out);
  let Ok(greeting_2) = answered(greeting, events, hash);

  Ok(Step::Again(LockBehavior {
    surface,
    greeting: greeting_2,
    standing: standing_2,
    rendered,
    bar: bar_2,
    notified,
    unlock,
  }))
}

#[derive(Clone)]
struct Answering {
  state: GreeterState,
  welcomed: Welcomed,
  attempts: PatternAttempts,
}

fn answered(
  answering: Answering,
  events: Vec<GreeterEvent>,
  hash: Option<&Hash>,
) -> Result<Answering, Never> {
  let unanswered = answering.clone();
  let ended = iterate((answering, events), |(answering_2, events_2)| {
    answering_round(answering_2, events_2, hash)
  });

  Ok(match ended {
    Ok(answered) => answered,
    Err(Endless) => unanswered,
  })
}

type AnsweringRound = Step<(Answering, Vec<GreeterEvent>), Answering>;

fn answering_round(
  answering: Answering,
  events: Vec<GreeterEvent>,
  hash: Option<&Hash>,
) -> Result<AnsweringRound, Never> {
  let (answering_2, replies) = events.into_iter().fold(
    (answering, Vec::new()),
    |(
      Answering {
        state,
        mut welcomed,
        mut attempts,
      },
      mut replies,
    ),
     event| {
      let Ok(Transition {
        state: state_2,
        effects,
      }) = Greeter::transition(state, Event::Custom(event));

      for effect in effects {
        match effect {
          Effect::Custom(GreeterEffect::Send(said)) => {
            let Ok(reply) = respond(&said, hash, &mut attempts, std::time::Instant::now());

            replies.extend(reply.map(GreeterEvent::Received));
          }
          Effect::Custom(GreeterEffect::Bar(asked)) => {
            let Ok(heard) = bar::carried(asked);

            replies.extend(heard);
          }
          Effect::Stop(Exit::Success) => welcomed = Welcomed::Yes,
          Effect::Stop(Exit::Failure(_)) => {}
          Effect::Run(_)
          | Effect::Stream(_)
          | Effect::Fork(_)
          | Effect::Interrupt(_)
          | Effect::Prompt(_)
          | Effect::Spawn(_)
          | Effect::Subscribe(_)
          | Effect::Unsubscribe(_)
          | Effect::Write(_)
          | Effect::Notify(_)
          | Effect::Print(_) => {}
        }
      }

      (
        Answering {
          state: state_2,
          welcomed,
          attempts,
        },
        replies,
      )
    },
  );

  Ok(match replies.as_slice() {
    [] => Step::Halt(answering_2),
    [_first, ..] => Step::Again((answering_2, replies)),
  })
}

struct Drawing<'a> {
  state: &'a GreeterState,
  worn: Worn<'a>,
  rendered: &'a mut Rendered,
  bar: showing::Rendered,
  unlock: &'a Unlock,
}

fn draw(surface: &mut Surface, drawing: Drawing<'_>) -> Result<showing::Rendered, Never> {
  let Drawing {
    state,
    worn,
    rendered,
    bar,
    unlock,
  } = drawing;
  let Ok(logical) = surface.logical();
  let logical_3 = match logical {
    Some(logical_2) => logical_2,
    None => return Ok(bar),
  };
  let (mut shapes, bar_2) = match unlock.method() {
    Ok(Method::Pattern) => {
      let Ok((shapes, bar_3)) = over_a_session(state, worn, logical_3, UNLOCK);
      (shapes, bar_3)
    }
    Ok(Method::Fingerprint | Method::Password) => {
      let Ok(bar_4) = state.bar.pictured(worn.bar, logical_3);
      let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 0, y: 0 },
        size: logical_3,
        round: Round(0),
        fill: worn.ring.ground,
        edge: Edge::None,
      })];
      (shapes, bar_4)
    }
  };
  let Ok(()) = unlock.paint(logical_3, worn.ring, &mut shapes);

  match {
    ::core::primitive::bool::then_some(
      ({
        let Ok(explicit_value) = unlock.method();
        explicit_value
      }) != Method::Pattern,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      shapes.extend(bar_2.shapes.iter().cloned());
    }
    ::core::option::Option::None => {}
  }

  let Ok(wanted) = rendered.wanted(shapes);

  match wanted {
    Some(shapes_2) => {
      let Ok(painting) = painter(logical_3, shapes_2, "lock-screen");

      match surface.draw(painting) {
        Ok(()) => {}
        Err(fault) => {
          let Ok(()) = rendered.forget();

          eprintln!("lock-screen: {fault}");
        }
      }
    }
    None => {}
  }

  Ok(bar_2)
}
