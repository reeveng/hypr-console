//! The decisions between a control request, a decoded frame and the next wake.

use std::io;
use std::os::unix::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_paper::{Motion, Request};

use crate::picture::Prepared;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
  Still,
  Moving(Duration),
}

pub enum Activation {
  Unchanged,
  Changed(Timing),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(u64);

pub struct Decoded {
  pub ticket: Ticket,
  pub result: Result<Prepared, LoadFault>,
}

#[derive(Debug)]
pub enum LoadFault {
  Decode(io::Error),
  Replaced,
  WorkerStopped,
}

impl std::fmt::Display for LoadFault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Decode(fault) => write!(to, "{fault}"),
      Self::Replaced => write!(to, "wallpaper load was superseded by a newer request"),
      Self::WorkerStopped => write!(to, "wallpaper decoder stopped"),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
  Waiting,
  Dirty,
  Ready,
}

pub enum Event {
  Asked(SocketAddr, Request),
  Decoded(Decoded),
  Activated(SocketAddr, Activation, Instant),
  Woke(Instant, Display),
  Advanced(Timing, Instant),
}

pub enum Effect {
  Decode(Ticket, PathBuf, Motion),
  Install(SocketAddr, Prepared),
  Cancel,
  Clear([u8; 3]),
  ReadStatus(SocketAddr),
  Reply(SocketAddr, Result<(), LoadFault>),
  Present,
  Advance,
  Stop,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Decode(..) => &[Service::FileSystem],
      Self::Clear(_) | Self::Present | Self::ReadStatus(_) => &[Service::Display],
      Self::Install(..) | Self::Advance => &[Service::Clock],
      Self::Reply(..) | Self::Cancel | Self::Stop => &[],
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playback {
  Still,
  Due(Instant),
}

impl Playback {
  fn with_timing(timing: Timing, now: Instant) -> Result<Self, Never> {
    Ok(match timing {
      Timing::Still => Self::Still,
      Timing::Moving(delay) => match now.checked_add(delay) {
        Some(next) => Self::Due(next),
        None => Self::Still,
      },
    })
  }

  pub fn wait(self, display: Display, now: Instant) -> Result<Option<Duration>, Never> {
    Ok(match (self, display) {
      (_, Display::Dirty) => Some(Duration::ZERO),
      (Self::Due(due), Display::Ready) => Some(due.saturating_duration_since(now)),
      (Self::Still, _) | (Self::Due(_), Display::Waiting) => None,
    })
  }
}

pub struct Paper;

struct Pending {
  peer: SocketAddr,
  ticket: Ticket,
}

pub struct State {
  playback: Playback,
  last: Ticket,
  pending: Option<Pending>,
}

impl State {
  pub fn wait(&self, display: Display, now: Instant) -> Result<Option<Duration>, Never> {
    self.playback.wait(display, now)
  }

  fn cancel(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    match self.pending.take() {
      Some(pending) => {
        let Ok(()) = effects.offer_all([
          Effect::Cancel,
          Effect::Reply(pending.peer, Err(LoadFault::Replaced)),
        ]);
      }
      None => {}
    }

    Ok(())
  }

  fn load(
    &mut self,
    peer: SocketAddr,
    path: PathBuf,
    motion: Motion,
    effects: &mut Queue<Effect>,
  ) -> Result<(), Never> {
    let next = Ticket(self.last.0.wrapping_add(1));
    self.last = next;

    match self.pending.replace(Pending { peer, ticket: next }) {
      Some(older) => {
        let Ok(()) = effects.offer(Effect::Reply(older.peer, Err(LoadFault::Replaced)));
      }
      None => {}
    }

    effects.offer(Effect::Decode(next, path, motion))
  }

  fn decoded(&mut self, decoded: Decoded, effects: &mut Queue<Effect>) -> Result<(), Never> {
    let requested = self.pending.as_ref().map(|pending| pending.ticket);

    match (requested == Some(decoded.ticket)).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    match (self.pending.take(), decoded.result) {
      (Some(pending), Ok(prepared)) => effects.offer(Effect::Install(pending.peer, prepared)),
      (Some(pending), Err(fault)) => effects.offer(Effect::Reply(pending.peer, Err(fault))),
      (None, _) => Ok(()),
    }
  }

  fn woke(
    &self,
    now: Instant,
    display: Display,
    effects: &mut Queue<Effect>,
  ) -> Result<(), Never> {
    match (self.playback, display) {
      (_, Display::Dirty) => effects.offer(Effect::Present),
      (Playback::Due(due), Display::Ready) => match now.cmp(&due) {
        std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => effects.offer(Effect::Advance),
        std::cmp::Ordering::Less => Ok(()),
      },
      (Playback::Still, Display::Ready | Display::Waiting)
      | (Playback::Due(_), Display::Waiting) => Ok(()),
    }
  }
}

impl Machine for Paper {
  type Input = ();
  type State = State;
  type Request = Event;
  type Effect = Effect;

  fn initialize(
    (): &(),
    _previous: Option<State>,
    _effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    Ok(State {
      playback: Playback::Still,
      last: Ticket(0),
      pending: None,
    })
  }

  fn handle(mut state: State, event: Event, effects: &mut Queue<Effect>) -> Result<State, Never> {
    let Ok(()) = match event {
      Event::Asked(peer, Request::Show(path, motion)) => state.load(peer, path, motion, effects),
      Event::Asked(peer, Request::Clear(color)) => {
        let Ok(()) = state.cancel(effects);
        state.playback = Playback::Still;
        effects.offer_all([
          Effect::Clear(color),
          Effect::Present,
          Effect::Reply(peer, Ok(())),
        ])
      }
      Event::Asked(peer, Request::Query) => effects.offer(Effect::ReadStatus(peer)),
      Event::Asked(peer, Request::Quit) => {
        let Ok(()) = state.cancel(effects);
        effects.offer_all([Effect::Reply(peer, Ok(())), Effect::Stop])
      }
      Event::Decoded(decoded) => state.decoded(decoded, effects),
      Event::Activated(peer, Activation::Unchanged, _) => {
        effects.offer(Effect::Reply(peer, Ok(())))
      }
      Event::Activated(peer, Activation::Changed(timing), now) => {
        let Ok(scheduled) = Playback::with_timing(timing, now);
        state.playback = scheduled;
        effects.offer_all([Effect::Present, Effect::Reply(peer, Ok(()))])
      }
      Event::Advanced(timing, now) => {
        let Ok(scheduled) = Playback::with_timing(timing, now);
        state.playback = scheduled;
        effects.offer(Effect::Present)
      }
      Event::Woke(now, display) => state.woke(now, display, effects),
    };

    Ok(state)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_now() -> Result<Instant, Never> {
    Ok(Instant::now())
  }

  #[test]
  fn a_blocked_output_does_not_keep_the_decoder_awake() {
    let Ok(now) = test_now();
    let playback = Playback::Due(now);
    let state = State {
      playback,
      last: Ticket(0),
      pending: None,
    };
    assert_eq!(playback.wait(Display::Waiting, now), Ok(None));
    let Ok(blocked) = Paper::transition(state, Event::Woke(now, Display::Waiting));
    assert!(blocked.effects.is_empty());
    let Ok(moving) = Paper::transition(blocked.state, Event::Woke(now, Display::Ready));
    assert!(matches!(moving.effects.as_slice(), [Effect::Advance]));
  }

  #[test]
  fn a_failed_load_preserves_the_existing_animation_deadline() -> io::Result<()> {
    let Ok(now) = test_now();
    let peer = SocketAddr::from_pathname("/wallpaper-client")?;
    let state = State {
      playback: Playback::Due(now),
      last: Ticket(1),
      pending: Some(Pending {
        ticket: Ticket(1),
        peer,
      }),
    };
    let Ok(step) = Paper::transition(
      state,
      Event::Decoded(Decoded {
        ticket: Ticket(1),
        result: Err(LoadFault::Decode(io::Error::from(
          io::ErrorKind::InvalidData,
        ))),
      }),
    );
    assert_eq!(step.state.playback, Playback::Due(now));
    assert!(step.state.pending.is_none());
    assert!(matches!(
      step.effects.as_slice(),
      [Effect::Reply(_, Err(LoadFault::Decode(_)))]
    ));

    Ok(())
  }

  #[test]
  fn the_old_animation_keeps_playing_and_a_displaced_result_is_ignored() -> io::Result<()> {
    let Ok(now) = test_now();
    let peer = SocketAddr::from_pathname("/first-wallpaper-client")?;
    let replacement = SocketAddr::from_pathname("/second-wallpaper-client")?;
    let state = State {
      playback: Playback::Due(now),
      last: Ticket(0),
      pending: None,
    };
    let Ok(first) = Paper::transition(
      state,
      Event::Asked(
        peer,
        Request::Show(PathBuf::from("/first.webp"), Motion::Play),
      ),
    );
    assert!(matches!(
      first.effects.as_slice(),
      [Effect::Decode(Ticket(1), _, Motion::Play)]
    ));
    let Ok(older_frame) = Paper::transition(first.state, Event::Woke(now, Display::Ready));
    assert!(matches!(older_frame.effects.as_slice(), [Effect::Advance]));
    let Ok(newer) = Paper::transition(
      older_frame.state,
      Event::Asked(
        replacement,
        Request::Show(PathBuf::from("/second.webp"), Motion::Play),
      ),
    );
    assert!(matches!(
      newer.effects.as_slice(),
      [
        Effect::Reply(_, Err(LoadFault::Replaced)),
        Effect::Decode(Ticket(2), _, Motion::Play)
      ]
    ));
    let Ok(stale) = Paper::transition(
      newer.state,
      Event::Decoded(Decoded {
        ticket: Ticket(1),
        result: Ok(Prepared::Reuse(Motion::Play)),
      }),
    );
    assert!(stale.effects.is_empty());
    let Ok(current) = Paper::transition(
      stale.state,
      Event::Decoded(Decoded {
        ticket: Ticket(2),
        result: Ok(Prepared::Reuse(Motion::Play)),
      }),
    );
    assert!(matches!(
      current.effects.as_slice(),
      [Effect::Install(_, Prepared::Reuse(Motion::Play))]
    ));

    Ok(())
  }
}
