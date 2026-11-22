use std::collections::{BTreeMap, btree_map::Entry};
use std::io;
use std::sync::mpsc::SyncSender;
use std::thread::{Builder, JoinHandle};

use console_core_never::Never;
use console_program_contract::{Answer, ExitStatus, Fork, Job};
use console_program_lifetime::{Insistence, Interruption};
use console_program_runtime::{Invocation, ReplyStatus};
use console_program_runtime::commands::{self, Streams};

use super::Arrived;
use super::commands::{Linux, Timing, finished};
use crate::program_lifetime::ForkCancellation;

#[derive(Default)]
pub(super) struct Jobs(BTreeMap<Job, Running>);

struct Running {
  invocation: Invocation,
  cancellation: ForkCancellation,
  interruption: Interruption,
  worker: JoinHandle<()>,
}

impl Jobs {
  pub(super) fn start<E: Send + 'static>(
    &mut self,
    called: &str,
    fork: &Fork,
    invocation: Invocation,
    replies: &SyncSender<Arrived<E>>,
  ) -> Result<(), Answer> {
    let slot = match self.0.entry(fork.job) {
      Entry::Vacant(slot) => slot,
      Entry::Occupied(_) => {
        let fault = io::Error::other(format!("job {:?} is already running", fork.job));
        let Ok(answer) = refused(called, fork, &fault);

        return Err(answer);
      }
    };
    let running = match start(fork, invocation, replies) {
      Ok(running) => running,
      Err(fault) => {
        let Ok(answer) = refused(called, fork, &fault);

        return Err(answer);
      }
    };
    slot.insert(running);

    Ok(())
  }

  pub(super) fn interrupt(&mut self, job: &Job) -> Result<(), Never> {
    match self.0.get_mut(job) {
      Some(running) => {
        let Ok((insistence, interruption)) = running.interruption.again();
        let Ok(_reached) = running.cancellation.interrupt(insistence);

        running.interruption = interruption;
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  pub(super) fn landed(
    &mut self,
    invocation: Invocation,
    answer: Answer,
  ) -> Result<Answer, Never> {
    let running = match self.0.entry(invocation.job) {
      Entry::Occupied(entry) => {
        let Ok(status) = entry.get().invocation.matches_reply(invocation, answer.job);

        match status {
          ReplyStatus::Accepted => {}
          ReplyStatus::Stale => return Ok(answer),
        }

        entry.remove()
      }
      Entry::Vacant(_) => return Ok(answer),
    };

    let Ok(()) = joined(running.worker);
    let Ok(status) = match running.interruption {
      Interruption::None => Ok(answer.status),
      Interruption::Requested | Interruption::Insisting => answer.status.after_interrupting(),
    };

    Ok(Answer { status, ..answer })
  }

  pub(super) fn finish(&mut self) -> Result<(), Never> {
    let running = std::mem::take(&mut self.0);

    for job in running.values() {
      let Ok(_reached) = job.cancellation.interrupt(Insistence::Insist);
    }

    for job in running.into_values() {
      let Ok(()) = joined(job.worker);
    }

    Ok(())
  }
}

fn refused(called: &str, fork: &Fork, fault: &io::Error) -> Result<Answer, Never> {
  let Ok(name) = fork.command.program.name();
  let detail = format!("{name} did not start: {fault}");

  console_core_logging::warn!("{called}: {detail}");

  Ok(Answer {
    job: Some(fork.job),
    command: fork.command.clone(),
    output: String::new(),
    error_output: detail,
    status: ExitStatus::Failure(None),
  })
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "a worker joins only after delivering its result, or after shutdown closed the reply queue and killed the process group that supplies its output pipes"
  )
)]
fn joined(worker: JoinHandle<()>) -> Result<(), Never> {
  match worker.join() {
    Ok(()) => {}
    Err(_panicked) => console_core_logging::error!("the process worker stopped unexpectedly"),
  }

  Ok(())
}

fn start<E: Send + 'static>(
  fork: &Fork,
  identity: Invocation,
  replies: &SyncSender<Arrived<E>>,
) -> io::Result<Running> {
  let platform = Linux {
    timing: Timing::Inherited,
  };
  let invocation = commands::resolve(&platform, &fork.command)?;
  let child = platform.fork(&invocation, Streams::COLLECT)?;
  let cancellation = child.cancellation()?;
  let fork_2 = fork.clone();
  let replies_2 = replies.clone();
  let worker = Builder::new().spawn(move || {
    let answer = match finished(child, None) {
      Ok(completion) => Answer {
        job: Some(fork_2.job),
        command: fork_2.command,
        output: completion.output,
        error_output: completion.error_output,
        status: completion.status,
      },
      Err(fault) => Answer {
        job: Some(fork_2.job),
        command: fork_2.command,
        output: String::new(),
        error_output: fault.to_string(),
        status: ExitStatus::Failure(None),
      },
    };

    match replies_2.send(Arrived::Reply(identity, answer)) {
      Ok(()) => {}
      Err(_stopped) => console_core_logging::trace!("the program stopped before its job reply arrived"),
    }
  })?;

  Ok(Running {
    invocation: identity,
    cancellation,
    interruption: Interruption::None,
    worker,
  })
}

#[cfg(test)]
mod tests {
  use std::sync::mpsc::sync_channel;
  use std::time::{Duration, Instant};

  use console_core_external_programs::Program;
  use console_core_state_machine::{Machine, Queue};
  use console_program_contract::{
    Command, Effect, Event, Exit, JobId, JobOwner, Subscription, Timer,
  };
  use console_program_runtime::{ExecutionError, Limit, Subscribed, execute};

  use super::super::{Arrived, Clock, Linux, Pure, subscription};
  use super::*;

  #[derive(Clone, Copy)]
  enum End {
    Stop,
    Error,
  }

  const INVALID: Timer = Timer {
    name: "invalid",
    interval: Duration::ZERO,
  };

  struct Shutdown;

  impl Machine for Shutdown {
    type Input = End;
    type State = ();
    type Request = Event<Never>;
    type Effect = Effect<Never>;

    #[cfg_attr(
      dylint_lib = "explicit043_no_unmatched_listen",
      expect(
        explicit043_no_unmatched_listen,
        reason = "the zero interval is rejected before subscription and exercises error cleanup"
      )
    )]
    fn initialize(
      end: &End,
      _previous: Option<()>,
      effects: &mut Queue<Effect<Never>>,
    ) -> Result<(), Never> {
      let Ok(sleeping) = Command::external(Program::Sleep, &["600"]);
      let Ok(finished) = Command::external(Program::True, &[]);

      for (id, command) in [(JobId(1), sleeping), (JobId(2), finished)] {
        let Ok(()) = effects.offer(Effect::Fork(Fork {
          job: Job {
            owner: JobOwner("shutdown test"),
            id,
          },
          command,
        }));
      }

      effects.offer(match end {
        End::Stop => Effect::Stop(Exit::Success),
        End::Error => Effect::Subscribe(Subscription::Timer(INVALID)),
      })
    }

    fn handle(
      (): (),
      _event: Event<Never>,
      _effects: &mut Queue<Effect<Never>>,
    ) -> Result<(), Never> {
      Ok(())
    }
  }

  #[test]
  fn shutdown_reaps_running_jobs_and_workers_on_both_exit_paths()
  -> Result<(), Box<dyn std::error::Error>> {
    for end in [End::Stop, End::Error] {
      let (forking, arrived) = sync_channel(1);

      forking.send(Arrived::Ended(Exit::Success))?;

      let Ok(subscriber) = subscription::connect(&[]);
      let Ok((subscriptions, _pool)) = subscriber.split();
      let mut interpreter = Pure;
      let mut platform = Linux {
        called: "shutdown test",
        interpreter: &mut interpreter,
        subscriptions,
        arrived: Some(arrived),
        forking,
        jobs: Jobs::default(),
        listening: Subscribed::No,
        clock: Clock {
          began: Instant::now(),
        },
      };

      match end {
        End::Stop => {
          let ended = execute::<Shutdown, _>(&end, &mut platform, Limit::Unbounded)?;

          assert_eq!(ended.exit, Exit::Success);
        }
        End::Error => assert_eq!(
          execute::<Shutdown, _>(&end, &mut platform, Limit::Unbounded),
          Err(ExecutionError::InvalidTimer(INVALID))
        ),
      }

      assert!(platform.arrived.is_none());
      assert!(platform.jobs.0.is_empty());
    }

    Ok(())
  }
}
