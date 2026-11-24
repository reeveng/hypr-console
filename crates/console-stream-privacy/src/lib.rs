//! OBS observations drive browser context switches and temporary desktop holds.

use std::time::Duration;

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};
use console_program_contract::{Effect, Event, Subscription, Timer};

const POLL: Timer = Timer {
  name: "OBS",
  interval: Duration::from_secs(3),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
  Closed,
  Idle,
  Unreadable,
  Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Obs {
  pub streaming: Output,
  pub recording: Output,
}

impl Obs {
  pub const CLOSED: Self = Self {
    streaming: Output::Closed,
    recording: Output::Closed,
  };
  pub const UNREADABLE: Self = Self {
    streaming: Output::Unreadable,
    recording: Output::Unreadable,
  };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture {
  Idle,
  Active,
}

impl Capture {
  fn observe(self, obs: Obs, effects: &mut Queue<Effect<DesktopEffect>>) -> Result<Self, Never> {
    Ok(match (obs.streaming, obs.recording) {
      (Output::Active, _) | (_, Output::Active) => {
        match self {
          Self::Idle => {
            let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::SwitchBrowser));
          }
          Self::Active => {}
        }

        Self::Active
      }
      (Output::Unreadable, _) | (_, Output::Unreadable) => self,
      (Output::Closed | Output::Idle, Output::Closed | Output::Idle) => Self::Idle,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEffect {
  Observe,
  SwitchBrowser,
  HoldNotifications,
  HoldScreen,
  Release,
  Publish(Output),
}

impl EffectDependencies for DesktopEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Observe | Self::Publish(_) => &[Service::FileSystem],
      Self::SwitchBrowser => &[
        Service::Configuration,
        Service::FileSystem,
        Service::Process,
      ],
      Self::HoldNotifications => &[Service::Notifications],
      Self::HoldScreen => &[Service::Power],
      Self::Release => &[Service::Notifications, Service::Power],
    })
  }
}

pub struct StreamPrivacy;

impl Machine for StreamPrivacy {
  type Input = ();
  type State = Capture;
  type Request = Event<Obs>;
  type Effect = Effect<DesktopEffect>;

  fn initialize(
    _: &(),
    _: Option<Capture>,
    _: &mut Queue<Self::Effect>,
  ) -> Result<Capture, Never> {
    Ok(Capture::Idle)
  }

  fn handle(
    capture: Capture,
    event: Event<Obs>,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<Capture, Never> {
    match event {
      Event::Opened => {
        let Ok(()) = effects.offer(Effect::Subscribe(Subscription::Timer(POLL)));
        let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::Observe));
      }
      Event::Tick(POLL, _) => {
        let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::Observe));
      }
      Event::Custom(obs) => {
        match obs.streaming {
          Output::Active => {
            let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::HoldNotifications));
            let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::HoldScreen));
          }
          Output::Closed | Output::Idle => {
            let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::Release));
          }
          Output::Unreadable => {}
        }

        let Ok(next) = capture.observe(obs, effects);
        let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::Publish(obs.streaming)));
        return Ok(next);
      }
      Event::Stopping => {
        let Ok(()) = effects.offer(Effect::Unsubscribe(Subscription::Timer(POLL)));
        let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::Release));
        let Ok(()) = effects.offer(Effect::Custom(DesktopEffect::Publish(Output::Closed)));
        return Ok(Capture::Idle);
      }
      Event::Tick(_, _)
      | Event::Modified(_)
      | Event::Replied(_)
      | Event::Written(_)
      | Event::Chosen(_) => {}
    }

    Ok(capture)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use DesktopEffect::{HoldNotifications, HoldScreen, Publish, Release, SwitchBrowser};
  use Output::{Active, Closed, Idle, Unreadable};

  #[test]
  fn the_browser_switches_once_until_both_outputs_stop() {
    let mut capture = Capture::Idle;

    for (streaming, recording, next, expected) in [
      (
        Active,
        Idle,
        Capture::Active,
        vec![
          HoldNotifications,
          HoldScreen,
          SwitchBrowser,
          Publish(Active),
        ],
      ),
      (
        Active,
        Active,
        Capture::Active,
        vec![HoldNotifications, HoldScreen, Publish(Active)],
      ),
      (Idle, Active, Capture::Active, vec![Release, Publish(Idle)]),
      (
        Unreadable,
        Unreadable,
        Capture::Active,
        vec![Publish(Unreadable)],
      ),
      (Idle, Active, Capture::Active, vec![Release, Publish(Idle)]),
      (Idle, Idle, Capture::Idle, vec![Release, Publish(Idle)]),
      (
        Idle,
        Active,
        Capture::Active,
        vec![Release, SwitchBrowser, Publish(Idle)],
      ),
      (
        Closed,
        Closed,
        Capture::Idle,
        vec![Release, Publish(Closed)],
      ),
      (
        Active,
        Idle,
        Capture::Active,
        vec![
          HoldNotifications,
          HoldScreen,
          SwitchBrowser,
          Publish(Active),
        ],
      ),
    ] {
      let Ok(turn) = StreamPrivacy::transition(
        capture,
        Event::Custom(Obs {
          streaming,
          recording,
        }),
      );

      assert_eq!(turn.state, next);
      assert_eq!(
        turn.effects,
        expected.into_iter().map(Effect::Custom).collect::<Vec<_>>()
      );
      capture = turn.state;
    }
  }

  #[test]
  fn stopping_releases_the_stream_holds() {
    let Ok(stopping) = StreamPrivacy::transition(Capture::Active, Event::Stopping);

    assert_eq!(stopping.state, Capture::Idle);
    assert_eq!(
      stopping.effects,
      vec![
        Effect::Unsubscribe(Subscription::Timer(POLL)),
        Effect::Custom(Release),
        Effect::Custom(Publish(Closed)),
      ],
    );
  }
}
