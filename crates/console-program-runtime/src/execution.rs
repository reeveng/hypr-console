//! The production execution loop, with the world and its clock handed in.
//!
//! A platform supplies arrivals and carries effects. Linux blocks on a channel;
//! a simulation advances to its next scripted arrival or deadline. Both run
//! these same turns, timer subscriptions, queued replies and stopping rules.
//! A bounded run refuses to pass when its turns run out, and a clock moving
//! backwards is a fault rather than a timer silently moved into the future.
//! The clock supplies monotonic time relative to the start of the run.
//!
//! The loop keeps each fork's invocation live until its matching reply arrives. This also
//! keeps a program alive while it awaits its last job. The platform runs and
//! interrupts the job. `Platform::start` opens the platform for one run.
//! `Platform::finish` runs after every outcome, including failed startup or
//! initialization. When cleanup fails, the error retains the run's outcome.
//! `Platform::perform` receives the invocation for each fork. Asynchronous
//! replies return it in `Wake::ForkReply`. The loop stamps immediate replies.
//! Providing a platform binds the program and its execution limit. Effects
//! begin when that program runs.
//! An executor receives each transition's state and request together and
//! returns the next state and ordered effects before the loop continues.

use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};
use std::fmt;

use console_core_iteration::Step;
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Transition};
use console_program_contract::{
  Answer, Effect, EffectKind, Elapsed, Event, EventGroup, Exit, Subscription, Task, Timer,
};

use crate::jobs::{StartError, Tasks};
use crate::{Invocation, ReplyStatus, Subscribed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wake<E> {
  Event(Event<E>),
  ForkReply(Invocation, Answer),
  Deadline,
  Disconnected,
  Stop(Exit),
}

impl<E> Wake<E> {
  pub(crate) fn from_effect(
    event: Event<E>,
    invocation: Option<Invocation>,
  ) -> Result<Self, Never> {
    Ok(match (event, invocation) {
      (Event::Replied(answer), Some(fork)) => {
        match ReplyStatus::from(answer.job == Some(fork.job)) {
          ReplyStatus::Accepted => Wake::ForkReply(fork, answer),
          ReplyStatus::Stale => Wake::Event(Event::Replied(answer)),
        }
      }
      (event_2, _) => Wake::Event(event_2),
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
  Unbounded,
  Turns(u64),
}

pub trait Clock {
  fn elapsed(&self) -> Result<Elapsed, Never>;
}

pub trait Platform: Clock {
  type Event;
  type Effect: EffectDependencies;
  type Error;

  fn start(&mut self) -> Result<(), Self::Error> {
    Ok(())
  }

  fn listening(&self) -> Result<Subscribed, Never>;

  fn receive(&mut self, deadline: Option<Elapsed>) -> Result<Wake<Self::Event>, Self::Error>;

  fn perform(
    &mut self,
    effect: &Effect<Self::Effect>,
    invocation: Option<Invocation>,
  ) -> Result<Vec<Event<Self::Event>>, Self::Error>;

  fn tick(
    &mut self,
    _timer: &Timer,
    _elapsed: Elapsed,
  ) -> Result<Vec<Event<Self::Event>>, Self::Error> {
    Ok(Vec::new())
  }

  fn event(&mut self, _event: &Event<Self::Event>) -> Result<(), Never> {
    Ok(())
  }

  fn effects(&mut self, _effects: &[Effect<Self::Effect>]) -> Result<(), Never> {
    Ok(())
  }

  fn finish(&mut self, _jobs: &BTreeMap<Task, Invocation>) -> Result<(), Self::Error> {
    Ok(())
  }
}

pub trait TransitionExecutor<M: Machine> {
  type Error;

  fn initialize(
    &mut self,
    input: &M::Input,
  ) -> Result<Transition<M::State, M::Effect>, Self::Error>;

  fn transition(
    &mut self,
    state: M::State,
    request: M::Request,
  ) -> Result<Transition<M::State, M::Effect>, Self::Error>;
}

pub struct Inline;

impl<M: Machine> TransitionExecutor<M> for Inline {
  type Error = Never;

  fn initialize(&mut self, input: &M::Input) -> Result<Transition<M::State, M::Effect>, Never> {
    M::initial_transition(input, None)
  }

  fn transition(
    &mut self,
    state: M::State,
    request: M::Request,
  ) -> Result<Transition<M::State, M::Effect>, Never> {
    M::transition(state, request)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed<S> {
  pub state: S,
  pub exit: Exit,
  pub turns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError<E, T = Never> {
  Start(E),
  Receive(E),
  Perform {
    effect: EffectKind,
    error: E,
  },
  Tick {
    timer: Timer,
    error: E,
  },
  Cleanup {
    error: E,
    outcome: Result<Exit, Box<Self>>,
  },
  Transition(T),
  TurnLimit(u64),
  ClockBackwards {
    previous: Elapsed,
    current: Elapsed,
  },
  InvalidTimer(Timer),
  DuplicateJob(Task),
  JobInvocationsExhausted,
}

#[must_use = "call run to execute the program"]
pub struct Provided<'a, M: Machine, P: Platform> {
  input: &'a M::Input,
  platform: &'a mut P,
  limit: Limit,
}

pub fn provide<'a, M, P>(
  input: &'a M::Input,
  platform: &'a mut P,
  limit: Limit,
) -> Result<Provided<'a, M, P>, Never>
where
  M: Machine<Request = Event<P::Event>, Effect = Effect<P::Effect>>,
  P: Platform,
{
  Ok(Provided {
    input,
    platform,
    limit,
  })
}

impl<M, P> Provided<'_, M, P>
where
  M: Machine<Request = Event<P::Event>, Effect = Effect<P::Effect>>,
  P: Platform,
{
  pub fn run(self) -> Result<Completed<M::State>, ExecutionError<P::Error>> {
    execute::<M, P>(self.input, self.platform, self.limit)
  }
}

#[derive(Clone, Copy)]
enum Cadence {
  Once,
  Repeating,
}

#[derive(Clone, Copy)]
struct ScheduledTimer {
  due: Elapsed,
  cadence: Cadence,
}

struct Execution<'a, S, P: Platform> {
  state: S,
  platform: &'a mut P,
  queue: VecDeque<Wake<P::Event>>,
  timers: BTreeMap<(&'static str, Elapsed), ScheduledTimer>,
  subscriptions: BTreeSet<EventGroup>,
  jobs: &'a mut Tasks,
  previous: Elapsed,
  turns: u64,
  limit: Limit,
}

type Turn<'a, S, P, T> =
  Result<Step<Execution<'a, S, P>, Completed<S>>, ExecutionError<<P as Platform>::Error, T>>;

pub fn execute<M, P>(
  input: &M::Input,
  platform: &mut P,
  limit: Limit,
) -> Result<Completed<M::State>, ExecutionError<P::Error>>
where
  P: Platform,
  M: Machine<Request = Event<P::Event>, Effect = Effect<P::Effect>>,
{
  execute_with::<M, P, _>(input, platform, limit, &mut Inline)
}

pub fn execute_with<M, P, X>(
  input: &M::Input,
  platform: &mut P,
  limit: Limit,
  transitions: &mut X,
) -> Result<Completed<M::State>, ExecutionError<P::Error, X::Error>>
where
  P: Platform,
  M: Machine<Request = Event<P::Event>, Effect = Effect<P::Effect>>,
  X: TransitionExecutor<M>,
{
  let mut jobs = Tasks::default();
  let result = match platform.start() {
    Ok(()) => run_loop::<M, P, X>(input, platform, &mut jobs, limit, transitions),
    Err(error) => Err(ExecutionError::Start(error)),
  };

  match platform.finish(&jobs.live) {
    Ok(()) => result,
    Err(error) => Err(ExecutionError::Cleanup {
      error,
      outcome: result.map(|completed| completed.exit).map_err(Box::new),
    }),
  }
}

fn run_loop<M, P, X>(
  input: &M::Input,
  platform: &mut P,
  jobs: &mut Tasks,
  limit: Limit,
  transitions: &mut X,
) -> Result<Completed<M::State>, ExecutionError<P::Error, X::Error>>
where
  P: Platform,
  M: Machine<Request = Event<P::Event>, Effect = Effect<P::Effect>>,
  X: TransitionExecutor<M>,
{
  let initialized = transitions
    .initialize(input)
    .map_err(ExecutionError::Transition)?;
  let Ok(previous) = platform.elapsed();
  let mut execution = Execution {
    state: initialized.state,
    platform,
    queue: VecDeque::new(),
    timers: BTreeMap::new(),
    subscriptions: BTreeSet::new(),
    jobs,
    previous,
    turns: 0,
    limit,
  };
  let Ok(()) = execution.platform.effects(&initialized.effects);
  let ending = execution.carry(&initialized.effects)?;

  match ending {
    Some(exit) => {
      let Ok(completed) = execution.complete(exit);

      return Ok(completed);
    }
    None => {}
  }

  execution.queue.push_back(Wake::Event(Event::Opened));

  let stepped = console_core_iteration::iterate((execution, transitions), |(running, runner)| {
    Ok(match turn::<M, P, X>(running, runner) {
      Ok(Step::Again(next)) => Step::Again((next, runner)),
      Ok(Step::Halt(completed)) => Step::Halt(Ok(completed)),
      Err(error) => Step::Halt(Err(error)),
    })
  });

  match stepped {
    Ok(result) => result,
    Err(_unreachable_end) => Err(ExecutionError::TurnLimit(u64::MAX)),
  }
}

enum Idle {
  Disconnected,
  Waiting,
}

impl From<bool> for Idle {
  fn from(disconnected: bool) -> Self {
    match bool::then_some(disconnected, ()) {
      Some(()) => Self::Disconnected,
      None => Self::Waiting,
    }
  }
}

enum TurnState {
  Running,
  Stopping,
}

impl From<bool> for TurnState {
  fn from(stopping: bool) -> Self {
    match bool::then_some(stopping, ()) {
      Some(()) => Self::Stopping,
      None => Self::Running,
    }
  }
}

fn turn<'a, M, P, X>(
  mut execution: Execution<'a, M::State, P>,
  transitions: &mut X,
) -> Turn<'a, M::State, P, X::Error>
where
  P: Platform,
  M: Machine<Request = Event<P::Event>, Effect = Effect<P::Effect>>,
  X: TransitionExecutor<M>,
{
  match execution.limit {
    Limit::Turns(maximum) => match execution.turns.cmp(&maximum) {
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
        return Err(ExecutionError::TurnLimit(maximum));
      }
      core::cmp::Ordering::Less => {}
    },
    Limit::Unbounded => {}
  }

  execution.turns = execution.turns.saturating_add(1);

  let wake = match execution.queue.pop_front() {
    Some(wake) => wake,
    None => {
      let now = execution.now()?;
      let due = execution
        .timers
        .iter()
        .filter(|(_timer, scheduled)| scheduled.due <= now)
        .min_by_key(|(_timer, scheduled)| scheduled.due)
        .map(|((name, interval), scheduled)| {
          let timer = Timer {
            name,
            interval: *interval,
          };

          (timer, scheduled.cadence)
        });

      match due {
        Some((timer, cadence)) => {
          let Ok(()) = execution.fired(timer, cadence, now);

          let events = execution
            .platform
            .tick(&timer, now)
            .map_err(|error| ExecutionError::Tick { timer, error })?;

          execution.queue.extend(events.into_iter().map(Wake::Event));
          execution
            .queue
            .push_back(Wake::Event(Event::Tick(timer, now)));

          return Ok(Step::Again(execution));
        }
        None => {}
      }

      let Ok(listening) = execution.platform.listening();

      let idle = Idle::from(
        execution.timers.is_empty()
          && execution.subscriptions.is_empty()
          && execution.jobs.live.is_empty()
          && listening == Subscribed::No,
      );

      match idle {
        Idle::Disconnected => Wake::Disconnected,
        Idle::Waiting => {
          let deadline = execution
            .timers
            .values()
            .map(|scheduled| scheduled.due)
            .min();
          execution
            .platform
            .receive(deadline)
            .map_err(ExecutionError::Receive)?
        }
      }
    }
  };

  let (event, interrupted) = match wake {
    Wake::Event(Event::Replied(answer)) => match answer.job {
      Some(_) => return Ok(Step::Again(execution)),
      None => (Event::Replied(answer), None),
    },
    Wake::Event(event) => (event, None),
    Wake::ForkReply(invocation, answer) => {
      let Ok(status) = execution.jobs.complete_reply(invocation, answer.job);

      match status {
        ReplyStatus::Accepted => (Event::Replied(answer), None),
        ReplyStatus::Stale => return Ok(Step::Again(execution)),
      }
    }
    Wake::Deadline => return Ok(Step::Again(execution)),
    Wake::Disconnected => (Event::Stopping, None),
    Wake::Stop(exit) => (Event::Stopping, Some(exit)),
  };

  let turn_state = TurnState::from(matches!(event, Event::Stopping));
  let Ok(()) = execution.platform.event(&event);
  let Transition { state, effects } = transitions
    .transition(execution.state, event)
    .map_err(ExecutionError::Transition)?;
  let mut advanced = Execution { state, ..execution };

  let Ok(()) = advanced.platform.effects(&effects);
  let ending = advanced.carry(&effects)?;
  let exit = interrupted.or(ending);

  match (exit, turn_state) {
    (Some(exit_2), TurnState::Running | TurnState::Stopping) => {
      let Ok(completed) = advanced.complete(exit_2);

      Ok(Step::Halt(completed))
    }
    (None, TurnState::Stopping) => {
      let Ok(completed) = advanced.complete(Exit::Success);

      Ok(Step::Halt(completed))
    }
    (None, TurnState::Running) => Ok(Step::Again(advanced)),
  }
}

impl<S, P: Platform> Execution<'_, S, P> {
  fn complete(self, exit: Exit) -> Result<Completed<S>, Never> {
    Ok(Completed {
      state: self.state,
      exit,
      turns: self.turns,
    })
  }

  fn now<T>(&mut self) -> Result<Elapsed, ExecutionError<P::Error, T>> {
    let Ok(current) = self.platform.elapsed();

    match current.cmp(&self.previous) {
      core::cmp::Ordering::Less => Err(ExecutionError::ClockBackwards {
        previous: self.previous,
        current,
      }),
      core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
        self.previous = current;

        Ok(current)
      }
    }
  }

  fn carry<T>(
    &mut self,
    effects: &[Effect<P::Effect>],
  ) -> Result<Option<Exit>, ExecutionError<P::Error, T>> {
    for effect in effects {
      let invocation = match effect {
        Effect::Stop(exit) => return Ok(Some(exit.clone())),
        Effect::Subscribe(Subscription::Timer(timer)) => {
          self.schedule(*timer, Cadence::Repeating)?;
          continue;
        }
        Effect::Subscribe(Subscription::Once(timer)) => {
          self.schedule(*timer, Cadence::Once)?;
          continue;
        }
        Effect::Unsubscribe(Subscription::Timer(timer) | Subscription::Once(timer)) => {
          let _ = self.timers.remove(&(timer.name, timer.interval));
          continue;
        }
        Effect::Subscribe(Subscription::EventGroup(group)) => {
          let _ = self.subscriptions.insert(group.clone());

          None
        }
        Effect::Unsubscribe(Subscription::EventGroup(group)) => {
          let _ = self.subscriptions.remove(group);

          None
        }
        Effect::Fork(fork) => Some({
          let checked_value = self.jobs.start(fork.job).map_err(|fault| match fault {
            StartError::Active(job) => ExecutionError::DuplicateJob(job),
            StartError::Exhausted => ExecutionError::JobInvocationsExhausted,
          })?;
          checked_value
        }),
        Effect::Run(_)
        | Effect::Stream(_)
        | Effect::Interrupt(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Custom(_) => None,
      };

      let events = self.platform.perform(effect, invocation).map_err(|error| {
        let Ok(kind) = effect.kind();

        ExecutionError::Perform {
          effect: kind,
          error,
        }
      })?;

      for event in events {
        let Ok(wake) = Wake::from_effect(event, invocation);
        self.queue.push_back(wake);
      }
    }

    Ok(None)
  }

  fn schedule<T>(
    &mut self,
    timer: Timer,
    cadence: Cadence,
  ) -> Result<(), ExecutionError<P::Error, T>> {
    let now = self.now()?;

    match self.timers.entry((timer.name, timer.interval)) {
      Entry::Occupied(_) => {}
      Entry::Vacant(entry) => {
        let due = now
          .checked_add(timer.interval)
          .filter(|_due| !timer.interval.is_zero())
          .ok_or(ExecutionError::InvalidTimer(timer))?;

        let _ = entry.insert(ScheduledTimer { due, cadence });
      }
    }

    Ok(())
  }

  fn fired(&mut self, timer: Timer, cadence: Cadence, now: Elapsed) -> Result<(), Never> {
    let next = match cadence {
      Cadence::Once => None,
      Cadence::Repeating => now.checked_add(timer.interval),
    };

    match next {
      Some(due) => {
        let _ = self.timers.insert(
          (timer.name, timer.interval),
          ScheduledTimer { due, cadence },
        );
      }
      None => {
        let _ = self.timers.remove(&(timer.name, timer.interval));
      }
    }

    Ok(())
  }
}

impl<E: fmt::Display, T: fmt::Display> fmt::Display for ExecutionError<E, T> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ExecutionError::Start(error) => write!(to, "platform startup failed: {error}"),
      ExecutionError::Receive(error) => write!(to, "receiving an event failed: {error}"),
      ExecutionError::Perform { effect, error } => {
        write!(to, "performing {effect:?} failed: {error}")
      }
      ExecutionError::Tick { timer, error } => write!(to, "timer {} failed: {error}", timer.name),
      ExecutionError::Cleanup { error, outcome } => match outcome {
        Ok(Exit::Success) => write!(to, "program cleanup failed: {error}"),
        Ok(Exit::Failure(failure)) => {
          write!(to, "{failure}. Program cleanup also failed: {error}")
        }
        Err(failure) => write!(to, "{failure}. Program cleanup also failed: {error}"),
      },
      ExecutionError::Transition(error) => write!(to, "a machine transition failed: {error}"),
      ExecutionError::TurnLimit(limit) => {
        write!(to, "the execution exhausted its limit of {limit} turns")
      }
      ExecutionError::ClockBackwards { previous, current } => write!(
        to,
        "the clock moved backwards from {previous:?} to {current:?}"
      ),
      ExecutionError::InvalidTimer(timer) => {
        write!(to, "timer {} has no future deadline", timer.name)
      }
      ExecutionError::DuplicateJob(job) => {
        write!(
          to,
          "job {} for {} is already in flight",
          job.id.0, job.owner.0
        )
      }
      ExecutionError::JobInvocationsExhausted => {
        to.write_str("the execution exhausted its fork invocation IDs")
      }
    }
  }
}

impl<E: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error
  for ExecutionError<E, T>
{
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Start(error)
      | Self::Receive(error)
      | Self::Perform { error, .. }
      | Self::Tick { error, .. }
      | Self::Cleanup {
        error,
        outcome: Ok(_),
      } => Some(error),
      Self::Cleanup {
        outcome: Err(failure),
        ..
      } => Some(failure.as_ref()),
      Self::Transition(error) => Some(error),
      Self::TurnLimit(_)
      | Self::ClockBackwards { .. }
      | Self::InvalidTimer(_)
      | Self::DuplicateJob(_)
      | Self::JobInvocationsExhausted => None,
    }
  }
}
