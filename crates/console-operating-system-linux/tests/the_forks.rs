//! A command forked, heard back from and stopped, by a program through the
//! loop, with a real child at the other end.
//!
//! What a fork is for is a program that is still listening while the command
//! runs, so the first thing asked is the one a `Run` cannot pass: a press that
//! arrives while the command is running is heard while it is running, and the
//! interrupt it decides on reaches the child before the child would have
//! finished on its own. Then the deadline, which is not the runtime's at all:
//! a timer the program subscribed to, and an interrupt when it falls due --
//! asked first, and insisted on when the command will not be asked.

use std::time::Duration;

use console_core_external_programs::Program;
use console_core_state_machine::{Machine, Never, Queue};
use console_operating_system_linux::program_runtime::{
  Clock, Interpreter, Subscribed, Tell, run,
};
use console_program_contract::{
  Command, Effect, Event, Exit, ExitStatus, Fork, Task, JobId, JobOwner, Subscription, Timer,
};
use console_program_lifetime::threads;

const DEADLINE: Timer = Timer {
  name: "deadline",
  interval: Duration::from_millis(100),
};

const LONG_AFTER: Timer = Timer {
  name: "long after",
  interval: Duration::from_secs(2),
};

const FIRST_JOB: Task = Task {
  owner: JobOwner("fork-test"),
  id: JobId(1),
};
const SECOND_JOB: Task = Task {
  owner: JobOwner("fork-test"),
  id: JobId(2),
};

fn start(job: Task, command: &Command) -> Result<Effect<Logged>, Never> {
  Ok(Effect::Fork(Fork {
    job,
    command: command.clone(),
  }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Logged {
  Received,
  Ticked,
  Reply(ExitStatus, String, String),
  JobReply(Task, ExitStatus),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deadline {
  Enforced,
  Watched,
  TwoJobs,
  None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Plan {
  command: Command,
  deadline: Deadline,
}

fn listening(deadline: Deadline) -> Result<Option<Subscription>, Never> {
  Ok(match deadline {
    Deadline::Enforced => Some(Subscription::Timer(DEADLINE)),
    Deadline::Watched => Some(Subscription::Timer(LONG_AFTER)),
    Deadline::TwoJobs | Deadline::None => None,
  })
}

impl Machine for Plan {
  type Input = Plan;
  type State = Plan;
  type Request = Event<()>;
  type Effect = Effect<Logged>;

  fn initialize(
    input: &Plan,
    _previous: Option<Plan>,
    _effects: &mut Queue<Effect<Logged>>,
  ) -> Result<Plan, Never> {
    Ok(input.clone())
  }

  fn handle(
    state: Plan,
    event: Event<()>,
    effects: &mut Queue<Effect<Logged>>,
  ) -> Result<Plan, Never> {
    let Ok(first) = start(FIRST_JOB, &state.command);
    let decided = match event {
      Event::Opened => match state.deadline {
        Deadline::Enforced => vec![first, Effect::Subscribe(Subscription::Timer(DEADLINE))],
        Deadline::Watched => vec![first, Effect::Subscribe(Subscription::Timer(LONG_AFTER))],
        Deadline::TwoJobs => {
          let Ok(second) = start(SECOND_JOB, &state.command);

          vec![first, second, Effect::Interrupt(FIRST_JOB)]
        }
        Deadline::None => vec![first],
      },
      Event::Custom(()) => vec![
        Effect::Custom(Logged::Received),
        Effect::Interrupt(FIRST_JOB),
      ],
      Event::Tick(_, _) => match state.deadline {
        Deadline::Enforced => vec![Effect::Interrupt(FIRST_JOB)],
        Deadline::Watched | Deadline::TwoJobs | Deadline::None => {
          vec![Effect::Custom(Logged::Ticked)]
        }
      },
      Event::Replied(answer) => match state.deadline {
        Deadline::TwoJobs => match answer.job {
          Some(job) => vec![Effect::Custom(Logged::JobReply(job, answer.status))],
          None => Vec::new(),
        },
        Deadline::Enforced | Deadline::Watched | Deadline::None => {
          let Ok(listening) = listening(state.deadline);
          let stop_listening = match listening {
            Some(timer) => vec![Effect::Unsubscribe(timer)],
            None => Vec::new(),
          };

          std::iter::once(Effect::Custom(Logged::Reply(
            answer.status,
            answer.output,
            answer.error_output,
          )))
          .chain(stop_listening)
          .chain(std::iter::once(Effect::Stop(Exit::Success)))
          .collect()
        }
      },
      Event::Modified(_) | Event::Written(_) | Event::Chosen(_) | Event::Stopping => Vec::new(),
    };
    let Ok(()) = effects.offer_all(decided);

    Ok(state)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pressing {
  Once,
  Never,
}

struct Keeping {
  kept: Vec<Logged>,
  pressing: Pressing,
}

impl Interpreter for Keeping {
  type Event = ();
  type Effect = Logged;

  fn interpret(&mut self, logged: &Logged, _clock: &Clock) -> Vec<Event<()>> {
    self.kept.push(logged.clone());

    Vec::new()
  }

  fn listen(&mut self, tell: Tell<()>) -> Result<Subscribed, Never> {
    match self.pressing {
      Pressing::Never => Ok(Subscribed::No),
      Pressing::Once => {
        let Ok(()) = threads::let_go(std::thread::spawn(move || {
          let Ok(_told) = tell.tell(());
        }));

        Ok(Subscribed::Yes)
      }
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit082_no_built_shell_line",
  expect(
    explicit082_no_built_shell_line,
    reason = "fixed test scripts exercise the shell and child lifetime contract as supplied by the fixture"
  )
)]
fn shell(script: &str) -> Result<Command, Never> {
  Command::external(Program::Sh, &["-c", script])
}

fn ran(plan: &Plan, pressing: Pressing) -> Result<Vec<Logged>, Never> {
  let mut keeping = Keeping {
    kept: Vec::new(),
    pressing,
  };
  let Ok(_code) = run::<Plan, Keeping>("forks", plan, &mut keeping);

  Ok(keeping.kept)
}

#[test]
fn a_fork_is_answered_with_what_it_said_and_how_it_ended() {
  let Ok(command) = shell("echo out; echo why >&2; exit 3");
  let plan = Plan {
    command,
    deadline: Deadline::None,
  };
  let Ok(logged) = ran(&plan, Pressing::Never);

  assert_eq!(
    logged,
    vec![Logged::Reply(
      ExitStatus::Failure(Some(3)),
      "out\n".to_string(),
      "why\n".to_string()
    )],
    "a program with only a fork in flight was finished before its answer came"
  );
}

#[test]
fn a_press_is_heard_while_a_fork_runs_and_the_interrupt_it_decides_ends_the_fork() {
  let Ok(command) = shell("sleep 5");
  let plan = Plan {
    command,
    deadline: Deadline::None,
  };
  let Ok(logged) = ran(&plan, Pressing::Once);

  assert_eq!(
    logged,
    vec![
      Logged::Received,
      Logged::Reply(ExitStatus::Interrupt, String::new(), String::new())
    ],
    "the loop was deaf while the command ran"
  );
}

#[test]
fn a_deadline_is_a_timer_the_program_kept_and_an_interrupt_when_it_fell_due() {
  let Ok(command) = shell("exec sleep 5");
  let plan = Plan {
    command,
    deadline: Deadline::Enforced,
  };
  let Ok(logged) = ran(&plan, Pressing::Never);

  assert_eq!(
    logged,
    vec![Logged::Reply(
      ExitStatus::Interrupt,
      String::new(),
      String::new()
    )]
  );
}

#[test]
fn a_fork_that_will_not_be_asked_is_ended_when_it_is_interrupted_again() {
  let Ok(command) = shell("trap '' TERM; exec sleep 5");
  let plan = Plan {
    command,
    deadline: Deadline::Enforced,
  };
  let Ok(logged) = ran(&plan, Pressing::Never);

  assert_eq!(
    logged,
    vec![Logged::Reply(
      ExitStatus::Interrupt,
      String::new(),
      String::new()
    )],
    "a command that ignores being asked ran to its end"
  );
}

#[test]
fn a_fork_that_finished_before_it_was_interrupted_says_how_it_finished() {
  let Ok(command) = shell("true");
  let plan = Plan {
    command,
    deadline: Deadline::Enforced,
  };
  let Ok(logged) = ran(&plan, Pressing::Never);

  assert_eq!(
    logged,
    vec![Logged::Reply(
      ExitStatus::Success,
      String::new(),
      String::new()
    )]
  );
}

#[test]
fn a_fork_is_answered_when_it_ends_though_what_it_started_still_holds_what_it_printed_into() {
  let Ok(command) = shell("sleep 20 & echo left");
  let plan = Plan {
    command,
    deadline: Deadline::Watched,
  };
  let Ok(logged) = ran(&plan, Pressing::Never);

  assert_eq!(
    logged,
    vec![Logged::Reply(
      ExitStatus::Success,
      "left\n".to_string(),
      String::new()
    )],
    "the answer waited for what the fork left running rather than for the fork"
  );
}

#[test]
fn interrupting_one_of_two_identical_commands_leaves_the_other_running() {
  let Ok(command) = shell("exec sleep 0.3");
  let plan = Plan {
    command,
    deadline: Deadline::TwoJobs,
  };
  let Ok(logged) = ran(&plan, Pressing::Never);

  assert_eq!(logged.len(), 2);
  assert!(logged.contains(&Logged::JobReply(FIRST_JOB, ExitStatus::Interrupt)));
  assert!(logged.contains(&Logged::JobReply(SECOND_JOB, ExitStatus::Success)));
}
