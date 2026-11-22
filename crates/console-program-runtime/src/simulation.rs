//! Scripted arrivals and effect completions over the production execution loop.
//!
//! Each performed effect must match the next expectation. Its replies are
//! queued at explicit offsets from that effect, so an old request can finish
//! after a newer one without starting a thread or spending wall-clock time.
//! Arrivals at one instant keep the order they were scheduled in. A missing arrival, an
//! unexpected effect, an unfinished script, or an exhausted execution limit
//! is an error. Shutdown discards scheduled replies for jobs still in flight.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use console_core_never::Never;
use console_program_contract::{Effect, Elapsed, Event, Job};

use crate::{Invocation, Platform, Subscribed, Wake};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct After<E> {
  pub delay: Elapsed,
  pub event: Event<E>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected<E, F> {
  pub effect: Effect<F>,
  pub replies: Vec<After<E>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulationError {
  UnexpectedEffect(u64),
  MissingArrival,
  TimeOverflow,
  InThePast,
  Unfinished { effects: u64, arrivals: u64 },
}

#[derive(Debug)]
pub struct Simulation<E, F> {
  now: Elapsed,
  listening: Subscribed,
  expected: VecDeque<Expected<E, F>>,
  arrivals: Vec<(Elapsed, Wake<E>)>,
  performed: u64,
  pub events: Vec<Event<E>>,
  pub effects: Vec<Vec<Effect<F>>>,
}

enum ScriptStatus {
  Complete,
  Pending,
}

impl From<bool> for ScriptStatus {
  fn from(complete: bool) -> Self {
    match bool::then_some(complete, ()) {
      Some(()) => Self::Complete,
      None => Self::Pending,
    }
  }
}

enum EffectMatch {
  Expected,
  Unexpected,
}

impl From<bool> for EffectMatch {
  fn from(expected: bool) -> Self {
    match bool::then_some(expected, ()) {
      Some(()) => Self::Expected,
      None => Self::Unexpected,
    }
  }
}

impl<E, F> Simulation<E, F> {
  pub fn new(expected: Vec<Expected<E, F>>, listening: Subscribed) -> Result<Self, Never> {
    Ok(Simulation {
      now: Elapsed::ZERO,
      listening,
      expected: VecDeque::from(expected),
      arrivals: Vec::new(),
      performed: 0,
      events: Vec::new(),
      effects: Vec::new(),
    })
  }

  pub fn schedule(&mut self, at: Elapsed, wake: Wake<E>) -> Result<(), SimulationError> {
    match at.cmp(&self.now) {
      core::cmp::Ordering::Less => Err(SimulationError::InThePast),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
        self.arrivals.push((at, wake));

        Ok(())
      }
    }
  }

  pub fn finish(&self) -> Result<(), SimulationError> {
    match ScriptStatus::from(self.expected.is_empty() && self.arrivals.is_empty()) {
      ScriptStatus::Complete => Ok(()),
      ScriptStatus::Pending => {
        let effects = u64::try_from(self.expected.len())
          .map_err(|_too_wide| SimulationError::TimeOverflow)?;
        let arrivals = u64::try_from(self.arrivals.len())
          .map_err(|_too_wide| SimulationError::TimeOverflow)?;

        Err(SimulationError::Unfinished { effects, arrivals })
      }
    }
  }
}

impl<E: Clone, F: Clone + PartialEq> Platform for Simulation<E, F> {
  type Event = E;
  type Effect = F;
  type Error = SimulationError;

  fn elapsed(&self) -> Result<Elapsed, Never> {
    Ok(self.now)
  }

  fn listening(&self) -> Result<Subscribed, Never> {
    Ok(self.listening)
  }

  fn receive(&mut self, deadline: Option<Elapsed>) -> Result<Wake<E>, SimulationError> {
    let next = self.arrivals.iter().map(|(at, _wake)| *at).min();
    let reached_deadline = deadline.filter(|due| next.is_none_or(|at| *due <= at));

    match reached_deadline {
      Some(due) => {
        self.now = due;

        return Ok(Wake::Deadline);
      }
      None => {}
    }

    let at = next.ok_or(SimulationError::MissingArrival)?;
    let found = self
      .arrivals
      .extract_if(.., |(when, _wake)| *when == at)
      .next();
    let (_when, wake) = found.ok_or(SimulationError::MissingArrival)?;

    self.now = at;

    Ok(wake)
  }

  fn perform(
    &mut self,
    effect: &Effect<F>,
    invocation: Option<Invocation>,
  ) -> Result<Vec<Event<E>>, SimulationError> {
    let expected = match self.expected.pop_front() {
      Some(expected) => expected,
      None => return Err(SimulationError::UnexpectedEffect(self.performed)),
    };

    match EffectMatch::from(expected.effect == *effect) {
      EffectMatch::Expected => {}
      EffectMatch::Unexpected => return Err(SimulationError::UnexpectedEffect(self.performed)),
    }

    self.performed = self.performed.saturating_add(1);

    let mut immediate = Vec::new();

    for reply in expected.replies {
      match reply.delay {
        Elapsed::ZERO => immediate.push(reply.event),
        _scheduled => {
          let at = self
            .now
            .checked_add(reply.delay)
            .ok_or(SimulationError::TimeOverflow)?;

          let Ok(wake) = Wake::from_effect(reply.event, invocation);
          self.schedule(at, wake)?;
        }
      }
    }

    Ok(immediate)
  }

  fn event(&mut self, event: &Event<E>) -> Result<(), Never> {
    self.events.push(event.clone());

    Ok(())
  }

  fn effects(&mut self, effects: &[Effect<F>]) -> Result<(), Never> {
    self.effects.push(effects.to_vec());

    Ok(())
  }

  fn finish(&mut self, jobs: &BTreeMap<Job, Invocation>) -> Result<(), SimulationError> {
    self.arrivals.retain(|(_, wake)| match wake {
      Wake::ForkReply(invocation, _) => jobs.get(&invocation.job) != Some(invocation),
      Wake::Event(_) | Wake::Deadline | Wake::Disconnected | Wake::Stop(_) => true,
    });

    Simulation::finish(self)
  }
}

impl fmt::Display for SimulationError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SimulationError::UnexpectedEffect(index) => {
        write!(to, "effect {index} did not match the simulation script")
      }
      SimulationError::MissingArrival => {
        to.write_str("the program is waiting but the simulation has no arrival or deadline")
      }
      SimulationError::TimeOverflow => to.write_str("the simulation's time or count overflowed"),
      SimulationError::InThePast => {
        to.write_str("an arrival was scheduled before the simulation's current time")
      }
      SimulationError::Unfinished { effects, arrivals } => write!(
        to,
        "the script still has {effects} effects and {arrivals} arrivals"
      ),
    }
  }
}

impl std::error::Error for SimulationError {}
