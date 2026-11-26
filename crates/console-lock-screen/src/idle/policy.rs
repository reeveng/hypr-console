//! Seat, compositor and logind observations drive one idle policy.

use std::collections::BTreeSet;

use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};

use super::{Action, Profile};
use crate::LockScreen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timer {
  pub action: Action,
  pub profile: Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
  On,
  Dimmed,
  Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
  Unlocked,
  Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sleep {
  Awake,
  AwaitingLock,
  Preparing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inhibition {
  Clear,
  Held,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inhibitor {
  Logind,
  ScreenSaver,
}

pub struct Input {
  pub power: Profile,
  pub lock_before_sleep: LockScreen,
  pub logind_idle: Inhibition,
}

pub enum Request {
  Idled(Timer),
  Activity,
  Power(Profile),
  Inhibit(Inhibitor, Inhibition),
  LockRequested,
  Session(Session),
  PreparingForSleep,
  ResumedFromSleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
  Display(Display),
  StartLock,
  Suspend,
  HoldSleepDelay,
  ReleaseSleepDelay,
  ResetTimers,
  ReportSession(Session),
  ReportSleep(Sleep),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Display(_) | Self::StartLock | Self::Suspend => &[Service::Process],
      Self::HoldSleepDelay | Self::ReleaseSleepDelay => &[Service::Session],
      Self::ResetTimers => &[Service::Session],
      Self::ReportSession(_) | Self::ReportSleep(_) => &[Service::Session],
    })
  }
}

enum SuspendRequest {
  Ready,
  Sent,
}

pub struct State {
  power: Profile,
  lock_before_sleep: LockScreen,
  session: Session,
  display: Display,
  sleep: Sleep,
  logind_idle: Inhibition,
  screen_saver: Inhibition,
  due: BTreeSet<Timer>,
  suspend_request: SuspendRequest,
}

impl State {
  pub fn session(&self) -> Result<Session, Never> {
    Ok(self.session)
  }

  fn inhibited(&self) -> Result<Inhibition, Never> {
    Ok(match (self.logind_idle, self.screen_saver) {
      (Inhibition::Clear, Inhibition::Clear) => Inhibition::Clear,
      (Inhibition::Held, _) | (_, Inhibition::Held) => Inhibition::Held,
    })
  }

  fn display(&mut self, wanted: Display, effects: &mut Queue<Effect>) -> Result<(), Never> {
    match (self.display != wanted).then_some(()) {
      Some(()) => {
        self.display = wanted;
        effects.offer(Effect::Display(wanted))
      }
      None => Ok(()),
    }
  }

  fn lock(&self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    match self.session {
      Session::Unlocked => effects.offer(Effect::StartLock),
      Session::Locked => Ok(()),
    }
  }

  fn timing(&self, action: Action) -> Result<Timer, Never> {
    Ok(Timer {
      action,
      profile: self.power,
    })
  }

  fn display_due(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    let Ok(dim) = self.timing(Action::Dim);
    let Ok(blank) = self.timing(Action::Blank);
    let wanted = match (self.due.contains(&dim), self.due.contains(&blank)) {
      (_, true) => Display::Off,
      (true, false) => Display::Dimmed,
      (false, false) => Display::On,
    };
    self.display(wanted, effects)
  }

  fn lock_due(&self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    let Ok(timer) = self.timing(Action::Lock);

    match self.due.contains(&timer).then_some(()) {
      Some(()) => self.lock(effects),
      None => Ok(()),
    }
  }

  fn suspend_due(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    let Ok(timer) = self.timing(Action::Suspend);

    match (&self.suspend_request, self.due.contains(&timer)) {
      (SuspendRequest::Ready, true) => {
        self.suspend_request = SuspendRequest::Sent;
        effects.offer(Effect::Suspend)
      }
      (SuspendRequest::Ready, false) | (SuspendRequest::Sent, _) => Ok(()),
    }
  }

  fn timeout(&mut self, timer: Timer, effects: &mut Queue<Effect>) -> Result<(), Never> {
    let Ok(inhibition) = self.inhibited();

    match (inhibition, self.sleep) {
      (Inhibition::Held, _) | (_, Sleep::AwaitingLock | Sleep::Preparing) => Ok(()),
      (Inhibition::Clear, Sleep::Awake) => {
        let _inserted = self.due.insert(timer);

        match (timer.profile == self.power).then_some(timer.action) {
          Some(Action::Dim | Action::Blank) => self.display_due(effects),
          Some(Action::Lock) => self.lock_due(effects),
          Some(Action::Suspend) => self.suspend_due(effects),
          None => Ok(()),
        }
      }
    }
  }

  fn power(&mut self, profile: Profile, effects: &mut Queue<Effect>) -> Result<(), Never> {
    let changed = self.power != profile;
    self.power = profile;
    let Ok(inhibition) = self.inhibited();

    match (changed, inhibition, self.sleep) {
      (true, Inhibition::Clear, Sleep::Awake) => {
        let Ok(()) = self.display_due(effects);
        let Ok(()) = self.lock_due(effects);
        self.suspend_due(effects)
      }
      (false, _, _)
      | (_, Inhibition::Held, _)
      | (_, _, Sleep::AwaitingLock | Sleep::Preparing) => Ok(()),
    }
  }

  fn reset(&mut self) -> Result<(), Never> {
    self.due.clear();
    self.suspend_request = SuspendRequest::Ready;
    Ok(())
  }

  fn inhibit(
    &mut self,
    source: Inhibitor,
    now: Inhibition,
    effects: &mut Queue<Effect>,
  ) -> Result<(), Never> {
    let Ok(before) = self.inhibited();

    match source {
      Inhibitor::Logind => self.logind_idle = now,
      Inhibitor::ScreenSaver => self.screen_saver = now,
    }

    let Ok(after) = self.inhibited();

    match (before, after) {
      (Inhibition::Clear, Inhibition::Held) => {
        let Ok(()) = self.reset();
        self.display(Display::On, effects)
      }
      (Inhibition::Held, Inhibition::Clear) => effects.offer(Effect::ResetTimers),
      (Inhibition::Clear, Inhibition::Clear) | (Inhibition::Held, Inhibition::Held) => Ok(()),
    }
  }

  fn session_changed(&mut self, now: Session, effects: &mut Queue<Effect>) -> Result<(), Never> {
    match (self.session != now).then_some(()) {
      Some(()) => {
        self.session = now;
        let Ok(()) = effects.offer(Effect::ReportSession(now));
      }
      None => {}
    }

    match (self.sleep, now) {
      (Sleep::AwaitingLock, Session::Locked) => {
        self.sleep = Sleep::Preparing;
        effects.offer(Effect::ReleaseSleepDelay)
      }
      (Sleep::Awake | Sleep::Preparing, _) | (Sleep::AwaitingLock, Session::Unlocked) => Ok(()),
    }
  }

  fn preparing(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    match self.sleep {
      Sleep::Awake => {
        let Ok(()) = effects.offer(Effect::ReportSleep(Sleep::Preparing));

        match (self.lock_before_sleep, self.session) {
          (LockScreen::On, Session::Unlocked) => {
            self.sleep = Sleep::AwaitingLock;
            self.lock(effects)
          }
          (LockScreen::On, Session::Locked) => {
            self.sleep = Sleep::Preparing;
            effects.offer(Effect::ReleaseSleepDelay)
          }
          (LockScreen::Off, _) => {
            self.sleep = Sleep::Preparing;
            Ok(())
          }
        }
      }
      Sleep::AwaitingLock | Sleep::Preparing => Ok(()),
    }
  }

  fn resumed(&mut self, effects: &mut Queue<Effect>) -> Result<(), Never> {
    self.sleep = Sleep::Awake;
    let Ok(()) = self.reset();
    let Ok(()) = effects.offer(Effect::ReportSleep(Sleep::Awake));
    let Ok(()) = effects.offer(Effect::ResetTimers);
    let Ok(()) = self.display(Display::On, effects);

    match self.lock_before_sleep {
      LockScreen::On => effects.offer(Effect::HoldSleepDelay),
      LockScreen::Off => Ok(()),
    }
  }
}

pub struct Idle;

impl Machine for Idle {
  type Input = Input;
  type State = State;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    input: &Input,
    _previous: Option<State>,
    effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    match input.lock_before_sleep {
      LockScreen::On => {
        let Ok(()) = effects.offer(Effect::HoldSleepDelay);
      }
      LockScreen::Off => {}
    }

    Ok(State {
      power: input.power,
      lock_before_sleep: input.lock_before_sleep,
      session: Session::Unlocked,
      display: Display::On,
      sleep: Sleep::Awake,
      logind_idle: input.logind_idle,
      screen_saver: Inhibition::Clear,
      due: BTreeSet::new(),
      suspend_request: SuspendRequest::Ready,
    })
  }

  fn handle(
    mut state: State,
    request: Request,
    effects: &mut Queue<Effect>,
  ) -> Result<State, Never> {
    let Ok(()) = match request {
      Request::Idled(timer) => state.timeout(timer, effects),
      Request::Activity => {
        let Ok(()) = state.reset();
        state.display(Display::On, effects)
      }
      Request::Power(profile) => state.power(profile, effects),
      Request::Inhibit(source, now) => state.inhibit(source, now, effects),
      Request::LockRequested => state.lock(effects),
      Request::Session(now) => state.session_changed(now, effects),
      Request::PreparingForSleep => state.preparing(effects),
      Request::ResumedFromSleep => state.resumed(effects),
    };

    Ok(state)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn effects(
    lock_before_sleep: LockScreen,
    requests: impl IntoIterator<Item = Request>,
  ) -> Result<Vec<Vec<Effect>>, Never> {
    let input = Input {
      power: Profile::PluggedIn,
      lock_before_sleep,
      logind_idle: Inhibition::Clear,
    };
    let Ok(initial) = Idle::initial_transition(&input, None);
    let mut state = initial.state;
    let mut seen = vec![initial.effects];

    for request in requests {
      let Ok(next) = Idle::transition(state, request);
      state = next.state;
      seen.push(next.effects);
    }

    Ok(seen)
  }

  #[test]
  fn a_blank_screen_wakes_once_even_when_dimming_is_disabled() {
    assert_eq!(
      effects(
        LockScreen::Off,
        [
          Request::Idled(Timer {
            action: Action::Blank,
            profile: Profile::PluggedIn
          }),
          Request::Idled(Timer {
            action: Action::Dim,
            profile: Profile::PluggedIn
          }),
          Request::Activity,
          Request::Activity,
        ],
      ),
      Ok(vec![
        vec![],
        vec![Effect::Display(Display::Off)],
        vec![],
        vec![Effect::Display(Display::On)],
        vec![],
      ]),
    );
  }

  #[test]
  fn suspend_waits_for_the_compositors_confirmation_and_rearms_on_resume() {
    assert_eq!(
      effects(
        LockScreen::On,
        [
          Request::PreparingForSleep,
          Request::PreparingForSleep,
          Request::Session(Session::Locked),
          Request::ResumedFromSleep,
          Request::PreparingForSleep,
        ],
      ),
      Ok(vec![
        vec![Effect::HoldSleepDelay],
        vec![Effect::ReportSleep(Sleep::Preparing), Effect::StartLock],
        vec![],
        vec![
          Effect::ReportSession(Session::Locked),
          Effect::ReleaseSleepDelay
        ],
        vec![
          Effect::ReportSleep(Sleep::Awake),
          Effect::ResetTimers,
          Effect::HoldSleepDelay
        ],
        vec![
          Effect::ReportSleep(Sleep::Preparing),
          Effect::ReleaseSleepDelay
        ],
      ]),
    );
  }

  #[test]
  fn the_new_power_profile_uses_inactivity_already_elapsed() {
    assert_eq!(
      effects(
        LockScreen::Off,
        [
          Request::Idled(Timer {
            action: Action::Lock,
            profile: Profile::OnBattery
          }),
          Request::Power(Profile::OnBattery),
          Request::Activity,
          Request::Power(Profile::PluggedIn),
          Request::Power(Profile::OnBattery),
        ],
      ),
      Ok(vec![
        vec![],
        vec![],
        vec![Effect::StartLock],
        vec![],
        vec![],
        vec![]
      ]),
    );
  }

  #[test]
  fn switching_power_applies_elapsed_display_and_suspend_timers_once() {
    let battery = |action| Timer {
      action,
      profile: Profile::OnBattery,
    };
    let plugged = |action| Timer {
      action,
      profile: Profile::PluggedIn,
    };

    assert_eq!(
      effects(
        LockScreen::Off,
        [
          Request::Idled(battery(Action::Blank)),
          Request::Idled(plugged(Action::Dim)),
          Request::Power(Profile::OnBattery),
          Request::Power(Profile::PluggedIn),
          Request::Idled(battery(Action::Suspend)),
          Request::Power(Profile::OnBattery),
          Request::Idled(plugged(Action::Suspend)),
          Request::Power(Profile::PluggedIn),
          Request::Activity,
          Request::Idled(plugged(Action::Suspend)),
        ],
      ),
      Ok(vec![
        vec![],
        vec![],
        vec![Effect::Display(Display::Dimmed)],
        vec![Effect::Display(Display::Off)],
        vec![Effect::Display(Display::Dimmed)],
        vec![],
        vec![Effect::Display(Display::Off), Effect::Suspend],
        vec![],
        vec![Effect::Display(Display::Dimmed)],
        vec![Effect::Display(Display::On)],
        vec![Effect::Suspend],
      ]),
    );
  }

  #[test]
  fn idle_actions_stay_paused_until_both_inhibitors_are_released() {
    assert_eq!(
      effects(
        LockScreen::Off,
        [
          Request::Idled(Timer {
            action: Action::Dim,
            profile: Profile::PluggedIn
          }),
          Request::Inhibit(Inhibitor::Logind, Inhibition::Held),
          Request::Inhibit(Inhibitor::ScreenSaver, Inhibition::Held),
          Request::Idled(Timer {
            action: Action::Blank,
            profile: Profile::PluggedIn
          }),
          Request::Inhibit(Inhibitor::Logind, Inhibition::Clear),
          Request::Idled(Timer {
            action: Action::Suspend,
            profile: Profile::PluggedIn
          }),
          Request::LockRequested,
          Request::Inhibit(Inhibitor::ScreenSaver, Inhibition::Clear),
          Request::Idled(Timer {
            action: Action::Suspend,
            profile: Profile::PluggedIn
          }),
        ],
      ),
      Ok(vec![
        vec![],
        vec![Effect::Display(Display::Dimmed)],
        vec![Effect::Display(Display::On)],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![Effect::StartLock],
        vec![Effect::ResetTimers],
        vec![Effect::Suspend],
      ]),
    );
  }
}
