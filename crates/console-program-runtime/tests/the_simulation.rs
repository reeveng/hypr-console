//! Timers, replies and shutdown driven through the production execution loop.

use std::cell::Cell;
use std::error::Error;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use console_core_bytes::Bytes;
use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service, Transition};
use console_program_contract::{
  Answer, Command, Effect, EffectKind, Event, Exit, ExitStatus, FileWrite, Fork, JobId, JobOwner,
  Subscription, Task, Timer, WriteAnswer, WriteFailure, WriteStage,
};
use console_program_lifetime::threads;
use console_program_runtime::simulation::{After, Expected, Simulation, SimulationError};
use console_program_runtime::{
  Clock, ExecutionError, Inline, Invocation, Limit, PlacementError, Platform, Seeded, Subscribed,
  TransitionExecutor, Wake, execute, execute_with, provide,
};

const EARLY: Timer = Timer {
  name: "early",
  interval: Duration::from_secs(1),
};
const LATE: Timer = Timer {
  name: "late",
  interval: Duration::from_secs(30),
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Work {
  Record(u32),
}

impl EffectDependencies for Work {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Record(_) => &[],
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Request {
  Effects(Vec<Effect<Work>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Repeat {
  Once,
  Forever,
  Until(Duration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rules {
  initialized: Vec<Effect<Work>>,
  opened: Vec<Effect<Work>>,
  stopping: Vec<Effect<Work>>,
  repeat: Repeat,
}

impl Default for Rules {
  fn default() -> Self {
    Rules {
      initialized: Vec::new(),
      opened: Vec::new(),
      stopping: Vec::new(),
      repeat: Repeat::Once,
    }
  }
}

#[derive(Debug, PartialEq, Eq)]
struct State {
  rules: Rules,
  answers: Vec<Answer>,
  writes: Vec<WriteAnswer>,
  stopping: u32,
}

struct Program;

impl Machine for Program {
  type Input = Rules;
  type State = State;
  type Request = Event<Request>;
  type Effect = Effect<Work>;

  fn initialize(
    input: &Rules,
    _previous: Option<State>,
    effects: &mut Queue<Effect<Work>>,
  ) -> Result<State, Never> {
    let Ok(()) = effects.offer_all(input.initialized.clone());

    Ok(State {
      rules: input.clone(),
      answers: Vec::new(),
      writes: Vec::new(),
      stopping: 0,
    })
  }

  fn handle(
    mut state: State,
    event: Event<Request>,
    effects: &mut Queue<Effect<Work>>,
  ) -> Result<State, Never> {
    match event {
      Event::Opened => {
        let Ok(()) = effects.offer_all(state.rules.opened.clone());
      }
      Event::Tick(timer, elapsed) => match state.rules.repeat {
        Repeat::Once => {
          let Ok(()) = effects.offer(Effect::Unsubscribe(Subscription::Timer(timer)));
        }
        Repeat::Until(end) => match elapsed.cmp(&end) {
          core::cmp::Ordering::Less => {
            let Ok(()) = effects.offer(Effect::Subscribe(Subscription::Once(timer)));
          }
          core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {}
        },
        Repeat::Forever => {}
      },
      Event::Custom(Request::Effects(requested)) => {
        let Ok(()) = effects.offer_all(requested);
      }
      Event::Replied(answer) => state.answers.push(answer),
      Event::Written(answer) => state.writes.push(answer),
      Event::Stopping => {
        state.stopping = state.stopping.saturating_add(1);

        let Ok(()) = effects.offer_all(state.rules.stopping.clone());
      }
      Event::Chosen(_) | Event::Modified(_) => {}
    }

    Ok(state)
  }
}

#[test]
fn timers_run_in_deadline_order_and_thirty_seconds_cost_no_wall_clock_wait()
-> Result<(), Box<dyn Error>> {
  let rules = Rules {
    initialized: vec![
      Effect::Subscribe(Subscription::Timer(LATE)),
      Effect::Subscribe(Subscription::Timer(EARLY)),
    ],
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::No);
  let Ok(program) = provide::<Program, _>(&rules, &mut world, Limit::Turns(20));
  let result = program.run()?;

  world.finish()?;

  assert_eq!(
    world.events,
    vec![
      Event::Opened,
      Event::Tick(EARLY, Duration::from_secs(1)),
      Event::Tick(LATE, Duration::from_secs(30)),
      Event::Stopping
    ]
  );
  assert_eq!(world.elapsed(), Ok(Duration::from_secs(30)));
  assert_eq!(result.exit, Exit::Success);

  Ok(())
}

#[test]
fn subscribing_twice_does_not_create_two_ticks() -> Result<(), Box<dyn Error>> {
  let rules = Rules {
    initialized: vec![
      Effect::Subscribe(Subscription::Timer(EARLY)),
      Effect::Subscribe(Subscription::Timer(EARLY)),
    ],
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::No);
  let _completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(20))?;

  assert_eq!(
    world.events,
    vec![
      Event::Opened,
      Event::Tick(EARLY, Duration::from_secs(1)),
      Event::Stopping
    ]
  );

  Ok(())
}

#[test]
fn a_one_shot_can_be_scheduled_again_from_its_tick_and_expires_after_the_last_tick()
-> Result<(), Box<dyn Error>> {
  let once = Effect::Subscribe(Subscription::Once(EARLY));
  let rules = Rules {
    initialized: vec![
      once.clone(),
      once,
      Effect::Subscribe(Subscription::Timer(EARLY)),
    ],
    repeat: Repeat::Until(Duration::from_secs(2)),
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::No);
  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(20))?;

  assert_eq!(
    world.events,
    [
      Event::Opened,
      Event::Tick(EARLY, Duration::from_secs(1)),
      Event::Tick(EARLY, Duration::from_secs(2)),
      Event::Stopping,
    ]
  );
  assert_eq!(completed.exit, Exit::Success);

  Ok(())
}

#[test]
fn an_unsubscribed_timer_never_fires() -> Result<(), Box<dyn Error>> {
  let rules = Rules {
    initialized: vec![Effect::Subscribe(Subscription::Once(EARLY))],
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::Yes);

  world.schedule(
    Duration::from_millis(100),
    Wake::Event(Event::Custom(Request::Effects(vec![Effect::Unsubscribe(
      Subscription::Once(EARLY),
    )]))),
  )?;
  world.schedule(Duration::from_secs(2), Wake::Disconnected)?;

  let _completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(20))?;

  world.finish()?;

  assert!(
    world
      .events
      .iter()
      .all(|event| !matches!(event, Event::Tick(_, _)))
  );
  assert_eq!(world.elapsed(), Ok(Duration::from_secs(2)));

  Ok(())
}

#[test]
fn a_failed_effect_delivers_its_reply_at_the_scripted_time() -> Result<(), Box<dyn Error>> {
  let Ok(command) = Command::external(ExternalProgram::Curl, &["a search"]);
  let answer = Answer {
    job: None,
    command: command.clone(),
    output: String::new(),
    error_output: "offline".to_string(),
    status: ExitStatus::Failure(Some(1)),
  };
  let rules = Rules {
    opened: vec![Effect::Run(command.clone())],
    ..Rules::default()
  };
  let expected = Expected {
    effect: Effect::Run(command),
    replies: vec![After {
      delay: Duration::from_secs(5),
      event: Event::Replied(answer.clone()),
    }],
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(vec![expected], Subscribed::Yes);

  world.schedule(Duration::from_secs(6), Wake::Disconnected)?;

  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(20))?;

  world.finish()?;

  assert_eq!(completed.state.answers, vec![answer]);
  assert_eq!(completed.state.stopping, 1);
  assert_eq!(world.elapsed(), Ok(Duration::from_secs(6)));

  Ok(())
}

#[test]
fn a_write_failure_is_delivered_to_the_machine() -> Result<(), Box<dyn Error>> {
  let write = FileWrite {
    path: <::std::path::PathBuf as ::core::convert::From<&::core::primitive::str>>::from(
      "report",
    ),
    contents: Bytes::from("ready".to_string()),
  };
  let failed = WriteAnswer {
    path: write.path.clone(),
    result: Err(WriteFailure {
      stage: WriteStage::Fill,
      detail: "disk full".to_string(),
    }),
  };
  let effect = Effect::Write(write);
  let rules = Rules {
    opened: vec![effect.clone()],
    ..Rules::default()
  };
  let expected = Expected {
    effect,
    replies: vec![After {
      delay: Duration::ZERO,
      event: Event::Written(failed.clone()),
    }],
  };
  let mut world = Simulation::<Request, Work>::new(vec![expected], Subscribed::No)?;
  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(20))?;

  assert_eq!(completed.state.writes, [failed]);
  assert_eq!(completed.state.stopping, 1);

  Ok(())
}

#[test]
fn a_stop_is_a_terminal_effect_and_nothing_after_it_runs() -> Result<(), Box<dyn Error>> {
  let rules = Rules {
    opened: vec![
      Effect::Custom(Work::Record(1)),
      Effect::Stop(Exit::Success),
      Effect::Custom(Work::Record(2)),
    ],
    ..Rules::default()
  };
  let expected = Expected {
    effect: Effect::Custom(Work::Record(1)),
    replies: Vec::new(),
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(vec![expected], Subscribed::No);
  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(10))?;

  world.finish()?;

  assert_eq!(completed.exit, Exit::Success);
  assert_eq!(world.events, vec![Event::Opened]);

  Ok(())
}

#[test]
fn a_source_stopping_runs_cleanup_and_preserves_its_failure() -> Result<(), Box<dyn Error>> {
  let rules = Rules {
    stopping: vec![Effect::Custom(Work::Record(7)), Effect::Stop(Exit::Success)],
    ..Rules::default()
  };
  let expected = Expected {
    effect: Effect::Custom(Work::Record(7)),
    replies: Vec::new(),
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(vec![expected], Subscribed::Yes);
  let failure = Exit::Failure("input device disconnected".to_string());

  world.schedule(Duration::ZERO, Wake::Stop(failure.clone()))?;

  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(10))?;

  world.finish()?;

  assert_eq!(completed.state.stopping, 1);
  assert_eq!(completed.exit, failure);
  assert_eq!(world.events, vec![Event::Opened, Event::Stopping]);

  Ok(())
}

#[test]
fn arrivals_at_the_same_time_keep_their_producers_order() -> Result<(), Box<dyn Error>> {
  let expected = vec![
    Expected {
      effect: Effect::Custom(Work::Record(1)),
      replies: Vec::new(),
    },
    Expected {
      effect: Effect::Custom(Work::Record(2)),
      replies: Vec::new(),
    },
  ];
  let Ok(mut world) = Simulation::<Request, Work>::new(expected, Subscribed::Yes);

  for value in [1, 2] {
    world.schedule(
      Duration::from_secs(1),
      Wake::Event(Event::Custom(Request::Effects(vec![Effect::Custom(
        Work::Record(value),
      )]))),
    )?;
  }

  world.schedule(Duration::from_secs(1), Wake::Disconnected)?;

  let completed = execute::<Program, _>(&Rules::default(), &mut world, Limit::Turns(10))?;

  world.finish()?;

  assert_eq!(completed.state.stopping, 1);

  Ok(())
}

#[test]
fn missing_arrivals_fail_instead_of_hanging() {
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::Yes);
  let result = execute::<Program, _>(&Rules::default(), &mut world, Limit::Turns(10));

  assert!(matches!(
    result,
    Err(ExecutionError::Receive(SimulationError::MissingArrival))
  ));
}

#[test]
fn unexpected_effects_fail_instead_of_being_stubbed_as_success() {
  let rules = Rules {
    opened: vec![Effect::Custom(Work::Record(1))],
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::No);
  let result = execute::<Program, _>(&rules, &mut world, Limit::Turns(10));

  assert!(matches!(
    result,
    Err(ExecutionError::Perform {
      effect: EffectKind::Custom,
      error: SimulationError::UnexpectedEffect(0),
    })
  ));
}

#[test]
fn unfinished_scripts_cannot_be_reported_as_complete() {
  let expected = Expected {
    effect: Effect::Custom(Work::Record(1)),
    replies: Vec::new(),
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(vec![expected], Subscribed::No);
  let result = execute::<Program, _>(&Rules::default(), &mut world, Limit::Turns(10));

  assert_eq!(
    result.map(|_completed| ()),
    Err(ExecutionError::Cleanup {
      error: SimulationError::Unfinished {
        effects: 1,
        arrivals: 0
      },
      outcome: Ok(Exit::Success),
    })
  );
}

#[test]
fn cleanup_preserves_a_runtime_failure_and_a_program_exit() {
  let remaining = Expected {
    effect: Effect::Custom(Work::Record(1)),
    replies: Vec::new(),
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(vec![remaining.clone()], Subscribed::No);

  assert_eq!(
    execute::<Program, _>(&Rules::default(), &mut world, Limit::Turns(0)).map(|_| ()),
    Err(ExecutionError::Cleanup {
      error: SimulationError::Unfinished {
        effects: 1,
        arrivals: 0
      },
      outcome: Err(Box::new(ExecutionError::TurnLimit(0))),
    })
  );

  let exit = Exit::Failure("the program refused to continue".to_string());
  let rules = Rules {
    initialized: vec![Effect::Stop(exit.clone())],
    ..Rules::default()
  };
  let Ok(mut world_2) = Simulation::<Request, Work>::new(vec![remaining], Subscribed::No);

  assert_eq!(
    execute::<Program, _>(&rules, &mut world_2, Limit::Turns(10)).map(|_| ()),
    Err(ExecutionError::Cleanup {
      error: SimulationError::Unfinished {
        effects: 1,
        arrivals: 0
      },
      outcome: Ok(exit),
    })
  );
}

#[test]
fn a_program_that_never_stops_exhausts_its_turn_limit() {
  let rules = Rules {
    initialized: vec![Effect::Subscribe(Subscription::Timer(EARLY))],
    repeat: Repeat::Forever,
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::No);
  let result = execute::<Program, _>(&rules, &mut world, Limit::Turns(12));

  assert!(matches!(result, Err(ExecutionError::TurnLimit(12))));
}

#[test]
fn a_zero_interval_timer_fails_instead_of_spinning() {
  let zero = Timer {
    name: "zero",
    interval: Duration::ZERO,
  };
  let rules = Rules {
    initialized: vec![Effect::Subscribe(Subscription::Timer(zero))],
    ..Rules::default()
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(Vec::new(), Subscribed::No);
  let result = execute::<Program, _>(&rules, &mut world, Limit::Turns(12));

  assert_eq!(
    result.map(|_completed| ()),
    Err(ExecutionError::InvalidTimer(zero))
  );
}

#[test]
fn initialization_replies_are_delivered_before_opening() -> Result<(), Box<dyn Error>> {
  let Ok(command) = Command::external(ExternalProgram::Curl, &["a search"]);
  let answer = Answer {
    job: None,
    command: command.clone(),
    output: "ready".to_string(),
    error_output: String::new(),
    status: ExitStatus::Success,
  };
  let rules = Rules {
    initialized: vec![Effect::Run(command.clone())],
    opened: vec![Effect::Stop(Exit::Success)],
    ..Rules::default()
  };
  let expected = Expected {
    effect: Effect::Run(command),
    replies: vec![After {
      delay: Duration::ZERO,
      event: Event::Replied(answer.clone()),
    }],
  };
  let Ok(mut world) = Simulation::<Request, Work>::new(vec![expected], Subscribed::No);
  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(10))?;

  assert_eq!(completed.state.answers, vec![answer.clone()]);
  assert_eq!(world.events, vec![Event::Replied(answer), Event::Opened]);

  Ok(())
}

struct Worker<'a>(&'a mut Vec<Event<Request>>);

fn run_on_worker(
  work: impl FnOnce() -> Result<Transition<State, Effect<Work>>, Never> + Send + 'static,
) -> io::Result<Transition<State, Effect<Work>>> {
  let (reply, finished) = mpsc::sync_channel(1);
  let worker = thread::Builder::new().spawn(move || {
    let Ok(transition) = work();
    let _receiver_may_have_left = reply.try_send(transition);
  })?;
  let Ok(()) = threads::let_go(worker);

  finished
    .recv_timeout(Duration::from_secs(10))
    .map_err(io::Error::other)
}

impl TransitionExecutor<Program> for Worker<'_> {
  type Error = io::Error;

  fn initialize(&mut self, rules: &Rules) -> Result<Transition<State, Effect<Work>>, io::Error> {
    let rules_2 = rules.clone();

    run_on_worker(move || Program::initial_transition(&rules_2, None))
  }

  fn transition(
    &mut self,
    state: State,
    request: Event<Request>,
  ) -> Result<Transition<State, Effect<Work>>, io::Error> {
    self.0.push(request.clone());

    run_on_worker(move || Program::transition(state, request))
  }
}

#[test]
fn seeded_placement_preserves_state_and_the_ordered_effect_loop() -> Result<(), Box<dyn Error>> {
  let write = FileWrite {
    path: <::std::path::PathBuf as ::core::convert::From<&::core::primitive::str>>::from(
      "report",
    ),
    contents: Bytes::from("ready".to_string()),
  };
  let written = WriteAnswer {
    path: write.path.clone(),
    result: Ok(()),
  };
  let record = Effect::Write(write);
  let rules = Rules {
    opened: vec![record.clone(), Effect::Subscribe(Subscription::Once(EARLY))],
    repeat: Repeat::Until(Duration::from_secs(2)),
    ..Rules::default()
  };
  let script = vec![Expected {
    effect: record,
    replies: vec![After {
      delay: Duration::ZERO,
      event: Event::Written(written.clone()),
    }],
  }];
  let Ok(mut inline) = Simulation::new(script.clone(), Subscribed::No);
  let Ok(mut switching) = Simulation::new(script.clone(), Subscribed::No);
  let Ok(mut replay) = Simulation::new(script, Subscribed::No);
  let mut on_worker = Vec::new();
  let mut replay_worker = Vec::new();
  let Ok(mut schedule) = Seeded::new(0, Inline, Worker(&mut on_worker));
  let Ok(mut repeated) = Seeded::new(0, Inline, Worker(&mut replay_worker));
  let original = execute::<Program, _>(&rules, &mut inline, Limit::Turns(20))?;
  let moved =
    execute_with::<Program, _, _>(&rules, &mut switching, Limit::Turns(20), &mut schedule)?;
  let replayed =
    execute_with::<Program, _, _>(&rules, &mut replay, Limit::Turns(20), &mut repeated)?;

  assert_eq!(
    on_worker,
    [
      Event::Written(written.clone()),
      Event::Tick(EARLY, Duration::from_secs(2))
    ],
  );
  assert_eq!(original, moved);
  assert_eq!(moved.state.writes, [written]);
  assert_eq!(inline.events, switching.events);
  assert_eq!(inline.effects, switching.effects);
  assert_eq!(moved, replayed);
  assert_eq!(switching.events, replay.events);
  assert_eq!(switching.effects, replay.effects);
  assert_eq!(on_worker, replay_worker);

  Ok(())
}

struct Failing;

impl TransitionExecutor<Program> for Failing {
  type Error = io::Error;

  fn initialize(&mut self, _rules: &Rules) -> Result<Transition<State, Effect<Work>>, io::Error> {
    Err(io::Error::other("the executor could not initialize"))
  }

  fn transition(
    &mut self,
    _state: State,
    _request: Event<Request>,
  ) -> Result<Transition<State, Effect<Work>>, io::Error> {
    Err(io::Error::other("the executor could not transition"))
  }
}

#[test]
fn a_transition_failure_still_cleans_up_outstanding_jobs() -> Result<(), Box<dyn Error>> {
  let Ok(command) = Command::external(ExternalProgram::Curl, &[]);
  let job = Task {
    owner: JobOwner("test"),
    id: JobId(1),
  };
  let effect = Effect::Fork(Fork {
    job,
    command: command.clone(),
  });
  let rules = Rules {
    initialized: vec![effect.clone()],
    ..Rules::default()
  };
  let script = vec![Expected {
    effect,
    replies: vec![After {
      delay: Duration::from_secs(1),
      event: Event::Replied(Answer {
        job: Some(job),
        command,
        output: String::new(),
        error_output: String::new(),
        status: ExitStatus::Success,
      }),
    }],
  }];
  let Ok(mut world) = Simulation::<Request, Work>::new(script, Subscribed::No);
  let Ok(mut schedule) = Seeded::new(0, Failing, Inline);
  let failed = execute_with::<Program, _, _>(&rules, &mut world, Limit::Turns(5), &mut schedule);

  assert!(matches!(
    failed,
    Err(ExecutionError::Transition(PlacementError::First(_)))
  ));
  assert_eq!(world.events, [Event::Opened]);
  world.finish()?;

  Ok(())
}

#[test]
fn a_duplicate_reply_cannot_settle_a_new_fork_with_the_same_job_key() -> Result<(), Box<dyn Error>>
{
  let Ok(command) = Command::external(ExternalProgram::True, &[]);
  let job = Task {
    owner: JobOwner("test"),
    id: JobId(1),
  };
  let effect = Effect::Fork(Fork {
    job,
    command: command.clone(),
  });
  let original = Answer {
    job: Some(job),
    command,
    output: "original".to_string(),
    error_output: String::new(),
    status: ExitStatus::Success,
  };
  let current = Answer {
    output: "current".to_string(),
    ..original.clone()
  };
  let rules = Rules {
    initialized: vec![effect.clone()],
    ..Rules::default()
  };
  let script = vec![
    Expected {
      effect: effect.clone(),
      replies: vec![
        After {
          delay: Duration::from_secs(1),
          event: Event::Replied(original.clone()),
        },
        After {
          delay: Duration::from_secs(3),
          event: Event::Replied(original.clone()),
        },
      ],
    },
    Expected {
      effect: effect.clone(),
      replies: vec![After {
        delay: Duration::from_secs(2),
        event: Event::Replied(current.clone()),
      }],
    },
  ];
  let Ok(mut world) = Simulation::<Request, Work>::new(script, Subscribed::Yes);
  world.schedule(
    Duration::from_secs(2),
    Wake::Event(Event::Custom(Request::Effects(vec![effect]))),
  )?;
  world.schedule(
    Duration::from_secs(3),
    Wake::Event(Event::Replied(current.clone())),
  )?;
  world.schedule(Duration::from_secs(5), Wake::Disconnected)?;

  let completed = execute::<Program, _>(&rules, &mut world, Limit::Turns(12))?;
  assert_eq!(completed.state.answers, [original, current]);
  world.finish()?;

  Ok(())
}

struct Backwards(Cell<u32>);

impl Clock for Backwards {
  fn elapsed(&self) -> Result<Duration, Never> {
    let reading = self.0.get();

    self.0.set(reading.saturating_add(1));

    Ok(match reading {
      0 => Duration::from_secs(1),
      1.. => Duration::ZERO,
    })
  }
}

impl Platform for Backwards {
  type Event = Request;
  type Effect = Work;
  type Error = Never;

  fn listening(&self) -> Result<Subscribed, Never> {
    Ok(Subscribed::No)
  }

  fn receive(&mut self, _deadline: Option<Duration>) -> Result<Wake<Request>, Never> {
    Ok(Wake::Disconnected)
  }

  fn perform(
    &mut self,
    _effect: &Effect<Work>,
    _invocation: Option<Invocation>,
  ) -> Result<Vec<Event<Request>>, Never> {
    Ok(Vec::new())
  }
}

#[test]
fn a_clock_moving_backwards_cannot_silently_reschedule_a_timer() {
  let rules = Rules {
    initialized: vec![Effect::Subscribe(Subscription::Timer(EARLY))],
    ..Rules::default()
  };
  let mut world = Backwards(Cell::new(0));
  let result = execute::<Program, _>(&rules, &mut world, Limit::Turns(10));

  assert_eq!(
    result.map(|_completed| ()),
    Err(ExecutionError::ClockBackwards {
      previous: Duration::from_secs(1),
      current: Duration::ZERO
    })
  );
}
