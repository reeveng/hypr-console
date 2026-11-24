//! The decisions between a control request, a decoded frame and the next wake.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_paper::{Motion, Request};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
  Still,
  Moving(Duration),
}

pub enum Load {
  Unchanged,
  Ready(Timing),
  Refused(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
  Waiting,
  Dirty,
  Ready,
}

pub enum Event {
  Asked(Request),
  Loaded(Load, Instant),
  Woke(Instant, Display),
  Advanced(Timing, Instant),
}

pub enum Effect {
  Load(PathBuf, Motion),
  Clear([u8; 3]),
  ReadStatus,
  Reply(Result<(), String>),
  Present,
  Advance,
  Stop,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Load(..) => &[Service::FileSystem, Service::Clock],
      Self::Clear(_) | Self::Present | Self::ReadStatus => &[Service::Display],
      Self::Advance => &[Service::Clock],
      Self::Reply(..) | Self::Stop => &[],
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

impl Machine for Paper {
  type Input = ();
  type State = Playback;
  type Request = Event;
  type Effect = Effect;

  fn initialize(
    (): &(),
    _previous: Option<Playback>,
    _effects: &mut Queue<Effect>,
  ) -> Result<Playback, Never> {
    Ok(Playback::Still)
  }

  fn handle(
    mut playback: Playback,
    event: Event,
    effects: &mut Queue<Effect>,
  ) -> Result<Playback, Never> {
    match event {
      Event::Asked(Request::Show(path, motion)) => {
        let Ok(()) = effects.offer(Effect::Load(path, motion));
      }
      Event::Asked(Request::Clear(color)) => {
        playback = Playback::Still;
        let Ok(()) =
          effects.offer_all([Effect::Clear(color), Effect::Present, Effect::Reply(Ok(()))]);
      }
      Event::Asked(Request::Query) => {
        let Ok(()) = effects.offer(Effect::ReadStatus);
      }
      Event::Asked(Request::Quit) => {
        let Ok(()) = effects.offer_all([Effect::Reply(Ok(())), Effect::Stop]);
      }
      Event::Loaded(Load::Unchanged, _) => {
        let Ok(()) = effects.offer(Effect::Reply(Ok(())));
      }
      Event::Loaded(Load::Ready(timing), now) => {
        let Ok(scheduled) = Playback::with_timing(timing, now);
        playback = scheduled;
        let Ok(()) = effects.offer_all([Effect::Present, Effect::Reply(Ok(()))]);
      }
      Event::Loaded(Load::Refused(fault), _) => {
        let Ok(()) = effects.offer(Effect::Reply(Err(fault)));
      }
      Event::Advanced(timing, now) => {
        let Ok(scheduled) = Playback::with_timing(timing, now);
        playback = scheduled;
        let Ok(()) = effects.offer(Effect::Present);
      }
      Event::Woke(now, display) => match (playback, display) {
        (_, Display::Dirty) => {
          let Ok(()) = effects.offer(Effect::Present);
        }
        (Playback::Due(due), Display::Ready) => match now.cmp(&due) {
          std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
            let Ok(()) = effects.offer(Effect::Advance);
          }
          std::cmp::Ordering::Less => {}
        },
        _ => {}
      },
    }

    Ok(playback)
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
    assert_eq!(playback.wait(Display::Waiting, now), Ok(None));
    let Ok(blocked) = Paper::transition(playback, Event::Woke(now, Display::Waiting));
    assert!(blocked.effects.is_empty());
    let Ok(moving) = Paper::transition(playback, Event::Woke(now, Display::Ready));
    assert!(matches!(moving.effects.as_slice(), [Effect::Advance]));
  }

  #[test]
  fn a_failed_load_preserves_the_existing_animation_deadline() {
    let Ok(now) = test_now();
    let Ok(step) = Paper::transition(
      Playback::Due(now),
      Event::Loaded(Load::Refused("damaged file".to_string()), now),
    );
    assert_eq!(step.state, Playback::Due(now));
    assert!(
      matches!(step.effects.as_slice(), [Effect::Reply(Err(message))] if message == "damaged file")
    );
  }
}
