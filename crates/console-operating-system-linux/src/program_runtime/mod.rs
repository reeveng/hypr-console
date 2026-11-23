//! Linux desktop integration for the platform-independent execution loop.
//!
//! The execution loop lives in `console-program-runtime`, shared with simulation.
//! Both use the same initialization, queued replies, timer subscriptions and
//! stopping rules; only the platform supplies time, arrivals and effect results.
//! GTK panels drive the same program contract through their own toolkit loop.
//!
//! Linux waits on one channel until an arrival or the earliest timer deadline.
//! The event pool and interpreter's listener feed that channel. An interpreter
//! that listens keeps the program alive until it sends `Tell::end`; otherwise,
//! a program with no remaining subscriptions receives `Event::Stopping` and ends.
//! A listener ending also delivers `Stopping`, so cleanup runs before exit.
//! The platform clock samples both wall and monotonic time. When the event
//! pool reconnects, `Interpreter::connected` can request a state refresh.
//!
//! Timer intervals must have a future deadline. Duplicate subscriptions are
//! idempotent, unsubscribing removes the deadline, and a due timer asks
//! `Interpreter::tick` for pending events before delivering `Event::Tick`.
//! A one-shot subscription expires before those events reach the machine.
//! Repeating timers skip missed intervals rather than replaying a burst.
//!
//! `Effect::Stop` ends the effect batch immediately. Captured commands still
//! run synchronously on Linux; their replies are queued for a later turn.
//! The simulation can script delayed replies without running a command or
//! spending wall-clock time, and refuses incomplete scripts at completion.
//!
//! A fork answers through the same channel as other arrivals. Each child is
//! held by its job key and a pidfd until its reply is received. Shutdown
//! closes the reply channel, cancels the remaining children, and joins their
//! workers so a full channel cannot strand a child or a worker.
//!
//! **Everything that wakes the loop waits for room.** The pool's carrier, an
//! interpreter's listener and a fork's answer reach the loop through one
//! channel, and it holds [`ARRIVING`] of them. A thread with something to say
//! when it is full waits until the loop has taken one, and that is the right
//! place for the wait: each of them has a buffer of its own behind it -- the
//! socket the pool writes to, the kernel's queue for an input device, the pipe
//! a fork prints into -- and the pool already ends a connection whose outbox is
//! full rather than let one slow reader hold up every other. What decides the
//! size is the largest burst that arrives at once, which is the pool's replay
//! of every event group a program has subscribed to when it connects. The one
//! thing that may not tell the loop anything is the loop: a [`Tell`] is for a
//! thread the interpreter started, because one told from the loop's own thread
//! into a full channel is a loop waiting for itself.

pub mod commands;
mod jobs;

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::{Instant, SystemTime};

use commands::{Linux as CommandPlatform, Timing};
use console_core_atomic_writes::Unwritten;
use console_core_external_programs::Program as ExternalProgram;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_state_machine::Machine;
use console_program_contract::{
  Answer, Choice, Command, Effect, Elapsed, Event, Exit, ExitStatus, FileWrite, Task,
  Notification, Prompt, Subscription, Timer, WriteAnswer, WriteFailure, WriteStage,
};
use console_program_lifetime::threads;
use console_program_runtime::commands::{self as execution_commands, Launch, Streams};
use console_program_runtime::{Invocation, Limit, Platform, Wake, provide};

use crate::events::subscription::{self, Received, Subscriptions};
use jobs::Tasks;

pub struct Clock {
  began: Instant,
}

pub struct Time {
  pub wall: SystemTime,
  pub monotonic: Instant,
  pub elapsed: Elapsed,
}

impl Clock {
  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
      explicit039_no_reading_the_clock,
      reason = "the platform samples both clocks for interpreters; decisions take the sample as an input"
    )
  )]
  #[cfg_attr(
    all(dylint_lib = "explicit077_no_ambient_randomness", not(test)),
    expect(
      explicit077_no_ambient_randomness,
      reason = "the Linux platform samples wall time once and hands the result to the interpreter as an input"
    )
  )]
  pub fn sample(&self) -> Result<Time, Never> {
    let monotonic = Instant::now();

    Ok(Time {
      wall: SystemTime::now(),
      monotonic,
      elapsed: monotonic.saturating_duration_since(self.began),
    })
  }
}

pub trait Interpreter {
  type Event;
  type Effect;

  fn interpret(&mut self, effect: &Self::Effect, clock: &Clock) -> Vec<Event<Self::Event>>;

  fn tick(&mut self, _timer: &Timer, _since: Elapsed) -> Result<Vec<Event<Self::Event>>, Never> {
    Ok(Vec::new())
  }

  fn connected(&mut self) -> Result<Option<Self::Event>, Never> {
    Ok(None)
  }

  fn listen(&mut self, _tell: Tell<Self::Event>) -> Result<Subscribed, Never> {
    Ok(Subscribed::No)
  }
}

pub use console_program_runtime::Subscribed;

pub const ARRIVING: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
  Yes,
  Gone,
}

enum Arrived<E> {
  Pool(Received),
  Custom(E),
  Reply(Invocation, Answer),
  Ended(Exit),
}

pub struct Tell<E>(SyncSender<Arrived<E>>);

impl<E> Tell<E> {
  pub fn tell(&self, event: E) -> Result<Delivery, Never> {
    Ok(match self.0.send(Arrived::Custom(event)) {
      Ok(()) => Delivery::Yes,
      Err(_the_loop_has_ended) => Delivery::Gone,
    })
  }

  pub fn end(&self, how: Exit) -> Result<Delivery, Never> {
    Ok(match self.0.send(Arrived::Ended(how)) {
      Ok(()) => Delivery::Yes,
      Err(_the_loop_has_ended) => Delivery::Gone,
    })
  }
}

pub struct Pure;

impl Interpreter for Pure {
  type Event = Never;
  type Effect = Never;

  fn interpret(&mut self, effect: &Never, _clock: &Clock) -> Vec<Event<Never>> {
    match *effect {}
  }
}

struct Linux<'a, C: Interpreter> {
  called: &'a str,
  interpreter: &'a mut C,
  subscriptions: Subscriptions,
  arrived: Option<Receiver<Arrived<C::Event>>>,
  forking: SyncSender<Arrived<C::Event>>,
  jobs: Tasks,
  listening: Subscribed,
  clock: Clock,
}

pub fn run<M, C>(called: &str, input: &M::Input, interpreter: &mut C) -> Result<ExitCode, Never>
where
  M: Machine<Request = Event<C::Event>, Effect = Effect<C::Effect>>,
  C: Interpreter<Event: Send + 'static>,
{
  let Ok(subscriber) = subscription::connect(&[]);
  let Ok((subscriptions, pool)) = subscriber.split();
  let Ok(room) = index(ARRIVING);
  let (telling, arrived) = sync_channel::<Arrived<C::Event>>(room);
  let carrying = telling.clone();
  let forking = telling.clone();

  let Ok(()) = threads::let_go(std::thread::spawn(move || {
    let Ok(()) = forward_pool(pool, carrying);
  }));

  let Ok(listening) = interpreter.listen(Tell(telling));

  #[cfg_attr(
    dylint_lib = "explicit039_no_reading_the_clock",
    allow(
      explicit039_no_reading_the_clock,
      reason = "the Linux platform owns the monotonic clock handed to the common execution loop"
    )
  )]
  let began = Instant::now();
  let mut platform = Linux {
    called,
    interpreter,
    subscriptions,
    arrived: Some(arrived),
    forking,
    jobs: Tasks::default(),
    listening,
    clock: Clock { began },
  };

  let Ok(program) = provide::<M, _>(input, &mut platform, Limit::Unbounded);

  match program.run() {
    Ok(completed) => ended(called, completed.exit),
    Err(error) => {
      eprintln!("{called}: {error}");

      Ok(ExitCode::FAILURE)
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit055_no_unbounded_loop",
  expect(
    explicit055_no_unbounded_loop,
    reason = "the pool carrier lives for a single subscription and exits when either its subscription or the owned event loop closes"
  )
)]
fn forward_pool<E>(
  pool: Receiver<Received>,
  carrying: SyncSender<Arrived<E>>,
) -> Result<(), Never> {
  for received in pool.iter() {
    match carrying.send(Arrived::Pool(received)) {
      Ok(()) => {}
      Err(_the_loop_has_ended) => return Ok(()),
    }
  }

  Ok(())
}

impl<C: Interpreter<Event: Send + 'static>> Platform for Linux<'_, C> {
  type Event = C::Event;
  type Effect = C::Effect;
  type Error = Never;

  fn elapsed(&self) -> Result<Elapsed, Never> {
    Ok(self.clock.began.elapsed())
  }

  fn listening(&self) -> Result<Subscribed, Never> {
    Ok(self.listening)
  }

  fn receive(&mut self, deadline: Option<Elapsed>) -> Result<Wake<C::Event>, Never> {
    let result = console_core_iteration::iterate(self, |platform| {
      let Ok(now) = platform.elapsed();

      let arrived = {
        let optional_arrived = platform.arrived.as_ref();

        match optional_arrived {
          Some(matched_arrived) => matched_arrived,
          ::core::option::Option::None => return Ok(Step::Halt(Wake::Disconnected)),
        }
      };

      #[cfg_attr(
        dylint_lib = "explicit057_no_unbounded_wait",
        expect(
          explicit057_no_unbounded_wait,
          reason = "the Linux event loop is the owning waiter. With no timer due it may sleep idle until the next event, while scheduled timers always pass a deadline"
        )
      )]
      let arrival = match deadline {
        Some(due) => match arrived.recv_timeout(due.saturating_sub(now)) {
          Ok(arrived_2) => arrived_2,
          Err(RecvTimeoutError::Timeout) => return Ok(Step::Halt(Wake::Deadline)),
          Err(RecvTimeoutError::Disconnected) => {
            return Ok(Step::Halt(Wake::Disconnected));
          }
        },
        None => match arrived.recv() {
          Ok(arrived_3) => arrived_3,
          Err(_disconnected) => return Ok(Step::Halt(Wake::Disconnected)),
        },
      };

      Ok(match arrival {
        Arrived::Pool(Received::Connected) => {
          let Ok(refresh) = platform.interpreter.connected();

          match refresh {
            Some(event) => Step::Halt(Wake::Event(Event::Custom(event))),
            None => Step::Again(platform),
          }
        }
        Arrived::Pool(Received::Event(change)) => {
          Step::Halt(Wake::Event(Event::Modified(change)))
        }
        Arrived::Custom(event) => Step::Halt(Wake::Event(Event::Custom(event))),
        Arrived::Reply(invocation, answer) => {
          let Ok(said) = platform.jobs.landed(invocation, answer);

          Step::Halt(Wake::ForkReply(invocation, said))
        }
        Arrived::Ended(exit) => Step::Halt(Wake::Stop(exit)),
      })
    });

    Ok(match result {
      Ok(wake) => wake,
      Err(_endless) => Wake::Disconnected,
    })
  }

  fn perform(
    &mut self,
    effect: &Effect<C::Effect>,
    invocation: Option<Invocation>,
  ) -> Result<Vec<Event<C::Event>>, Never> {
    let events = match effect {
      Effect::Run(command) => {
        let Ok(answer) = run_command(self.called, command, Streams::CAPTURE);

        vec![Event::Replied(answer)]
      }
      Effect::Stream(command) => {
        let Ok(answer) = run_command(self.called, command, Streams::INHERIT);

        vec![Event::Replied(answer)]
      }
      Effect::Fork(fork) => match invocation {
        Some(issued) => match self.jobs.start(self.called, fork, issued, &self.forking) {
          Ok(()) => Vec::new(),
          Err(answer) => vec![Event::Replied(answer)],
        },
        None => Vec::new(),
      },
      Effect::Interrupt(job) => {
        let Ok(()) = self.jobs.interrupt(job);

        Vec::new()
      }
      Effect::Prompt(prompt) => {
        let Ok(choice) = chose(prompt);

        vec![Event::Chosen(choice)]
      }
      Effect::Spawn(command) => {
        let mut platform = CommandPlatform {
          timing: Timing::Inherited,
        };

        match execution_commands::spawn(
          &mut platform,
          command,
          Streams {
            stdout: execution_commands::Stream::Discard,
            stderr: execution_commands::Stream::Inherit,
          },
          Launch::Application,
        ) {
          Ok(()) => {}
          Err(fault) => eprintln!("{}: nothing started: {fault}", self.called),
        }

        Vec::new()
      }
      Effect::Subscribe(Subscription::EventGroup(group)) => {
        let Ok(()) = self.subscriptions.subscribe(group);

        Vec::new()
      }
      Effect::Unsubscribe(Subscription::EventGroup(group)) => {
        let Ok(()) = self.subscriptions.unsubscribe(group);

        Vec::new()
      }
      Effect::Write(write) => {
        let Ok(answer) = wrote(self.called, write);

        vec![Event::Written(answer)]
      }
      Effect::Notify(notification) => {
        let Ok(()) = notify(self.called, notification);

        Vec::new()
      }
      #[cfg_attr(
        dylint_lib = "explicit041_no_unsaid_printing",
        allow(
          explicit041_no_unsaid_printing,
          reason = "the Linux platform is the edge that carries Effect::Print out"
        )
      )]
      Effect::Print(line) => {
        println!("{line}");

        Vec::new()
      }
      Effect::Custom(custom) => self.interpreter.interpret(custom, &self.clock),
      Effect::Subscribe(Subscription::Timer(_) | Subscription::Once(_))
      | Effect::Unsubscribe(Subscription::Timer(_) | Subscription::Once(_))
      | Effect::Stop(_) => Vec::new(),
    };
    Ok(events)
  }

  fn tick(&mut self, timer: &Timer, elapsed: Elapsed) -> Result<Vec<Event<C::Event>>, Never> {
    self.interpreter.tick(timer, elapsed)
  }

  fn finish(&mut self, _jobs: &BTreeMap<Task, Invocation>) -> Result<(), Never> {
    drop(self.arrived.take());
    self.jobs.finish()
  }
}

fn ended(called: &str, how: Exit) -> Result<ExitCode, Never> {
  Ok(match how {
    Exit::Success => ExitCode::SUCCESS,
    Exit::Failure(fault) => {
      eprintln!("{called}: {fault}");

      ExitCode::FAILURE
    }
  })
}

fn run_command(called: &str, command: &Command, streams: Streams) -> Result<Answer, Never> {
  let mut platform = CommandPlatform {
    timing: Timing::Inherited,
  };

  Ok(
    match execution_commands::run(&mut platform, command, streams) {
      Ok(answer) => answer,
      Err(fault) => {
        let Ok(name) = command.program.name();

        eprintln!("{called}: {name} did not run: {fault}");

        Answer {
          job: None,
          command: command.clone(),
          output: String::new(),
          error_output: fault.to_string(),
          status: ExitStatus::Failure(None),
        }
      }
    },
  )
}

#[cfg_attr(
  dylint_lib = "explicit041_no_unsaid_printing",
  allow(
    explicit041_no_unsaid_printing,
    reason = "a question asked of whoever is at the terminal, whose answer is read back off stdin on the same line -- there is no effect for the half of a conversation that is waiting for a person"
  )
)]
fn chose(prompt: &Prompt) -> Result<Choice, Never> {
  print!("{} ", prompt.message);

  match std::io::Write::flush(&mut std::io::stdout()) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("could not show the prompt: {fault}");

      return Ok(prompt.default);
    }
  }

  let mut said = String::new();
  let terminal = std::io::stdin();

  match terminal.read_line(&mut said) {
    Ok(0) => Ok(prompt.default),
    Err(_the_terminal_failed) => Ok(prompt.default),
    Ok(_) => parse_choice(said.trim(), prompt.default),
  }
}

fn parse_choice(said: &str, default: Choice) -> Result<Choice, Never> {
  Ok(match said.to_lowercase().as_str() {
    "y" | "yes" => Choice::Yes,
    "n" | "no" => Choice::No,
    _ => default,
  })
}

fn wrote(called: &str, write: &FileWrite) -> Result<WriteAnswer, Never> {
  let result =
    console_core_atomic_writes::whole(&write.path, write.contents.as_bytes()).map_err(|fault| {
      let stage = match &fault {
        Unwritten::Making(_, _) => WriteStage::Create,
        Unwritten::Filling(_, _) => WriteStage::Fill,
        Unwritten::Settling(_, _) => WriteStage::SyncFile,
        Unwritten::Moving(_, _) => WriteStage::Replace,
        Unwritten::Keeping(_, _) => WriteStage::PreserveMetadata,
        Unwritten::Naming(_, _) => WriteStage::SyncDirectory,
        Unwritten::Cleaning(_, _, _) => WriteStage::Cleanup,
      };
      eprintln!("{called}: {}: {fault}", write.path.display());

      WriteFailure {
        stage,
        detail: fault.to_string(),
      }
    });

  Ok(WriteAnswer {
    path: write.path.clone(),
    result,
  })
}

fn notify(called: &str, notification: &Notification) -> Result<(), Never> {
  let arguments: Vec<&str> = std::iter::once(notification.summary.as_str())
    .chain(notification.body.as_deref())
    .collect();
  let Ok(command) = Command::external(ExternalProgram::NotifySend, &arguments);
  let mut platform = CommandPlatform {
    timing: Timing::Background,
  };

  match execution_commands::run(&mut platform, &command, Streams::INHERIT) {
    Ok(_) => {}
    Err(fault) => eprintln!("{called}: nothing was said on the screen: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod write_tests {
  use super::*;

  #[test]
  fn a_write_reply_reports_the_target_and_the_stage_that_failed()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("linux-write-reply")?;
    let path = folder.join("report");
    let answer = wrote(
      "test",
      &FileWrite {
        path: path.clone(),
        contents: "saved".to_string(),
      },
    )?;
    assert_eq!(
      answer,
      WriteAnswer {
        path: path.clone(),
        result: Ok(())
      }
    );
    let contents = std::fs::read_to_string(&path)?;

    assert_eq!(contents, "saved");

    let missing = path.join("no-file-can-go-here");
    let failed = wrote(
      "test",
      &FileWrite {
        path: missing.clone(),
        contents: "unsaved".to_string(),
      },
    )?;
    assert_eq!(failed.path, missing);
    assert!(matches!(
      failed.result,
      Err(WriteFailure {
        stage: WriteStage::Create,
        ..
      })
    ));

    Ok(())
  }
}
