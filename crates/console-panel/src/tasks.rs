//! What a press started that has not finished, and the row that says so.
//!
//! A lift's call button lights when it is pressed and stays lit until the car
//! comes, and pressing it again does nothing. That is the whole of what is here,
//! and for as long as a panel had only `Showing::later` none of it was: a row
//! started a program on a thread of its own and never heard it end, so `Pair`
//! said `Pair` for the whole of a pairing, a second press started a second
//! pairing beside the first, and the only sign that anything was happening was
//! a note that said what had been set going and nothing about how it went.
//!
//! **What is running is a machine, and a row reads it.** [`Tasks`] holds the
//! commands started and not yet answered. A [`Task`] whose command is already
//! running is not started again, for every caller and whatever it was pressed
//! from, and a row that names a command with [`Row::while_running`] is drawn in
//! the words it gave for that and with [`YET`] beside it, quieter than a row
//! that can be pressed and with nothing to press, for as long as the command
//! runs. That is the look `Searching for Devices` already had, made the look of
//! everything in progress. It is still: a turning mark is a clock in a loop that
//! has none, and a handheld woken thirty times a second for a whole pairing.
//!
//! **A row stands for what its own press started, or for a command it names.**
//! A pairing a PIN was typed for is not the command `Pair` would have run
//! without one, so a task is kept with the command of the row it was started
//! from as well as with its own, and a row is in progress when either is the one
//! it names. A file unzipped from a menu is the other half: the menu has gone by
//! the time it runs, and the file's own row names the command.
//!
//! A task's reply goes to desktop notifications. An out-loud task can also
//! keep a transient notification current while it runs, for instructions
//! such as entering a Bluetooth PIN.
//!
//! **A task can be stopped.** A row in progress offers what else can be done
//! with it, and what can be done is to stop it: "Stop Downloading A Song?",
//! asked rather than done, because the press that reaches it is the one beside
//! the press that started it. A card can also ask to interrupt a task.
//! [`TaskEvent::Interrupt`] records the request, and the command is
//! told to stop by the [`ProcessIdentity`] it was given when it began -- a value
//! in the state like any other, since the inode in it is that one process's
//! for as long as the machine is up. An interrupt that arrives before the
//! command has begun is kept and said when it does, and one that arrives twice
//! insists. A task is a fork, so what it started goes with it, and it is bound
//! to the thread that waits on it rather than let go: an orphaned yt-dlp after
//! the panel's host has gone is a download no row can see or stop.
//!
//! The host keeps running tasks across panel openings so their rows can still
//! show progress and offer a way to stop them.

use std::thread;

use console_actor::{Actor, SendError};
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_notifications::Application;
use console_notifications::saying::LiveNotification;
use console_operating_system_linux::program_lifetime::ProcessIdentity;
use console_operating_system_linux::program_runtime::commands::{self, Linux, Timing};
use console_program_contract::{Answer, Command, ExitStatus, Notification};
use console_program_lifetime::{Insistence, Interruption, threads};
use console_program_runtime::commands::{Reporter, Streams};

use crate::page::{
  Act, ActionOutcome, Progress, Retained, Row, Showing, Subject, WhileRunning, YET, retained,
};

pub const STOP: &str = "Stop";

pub type Reply = Retained<dyn Fn(&Answer) -> Result<Option<Notification>, Never> + Send + Sync>;

pub type InProgress = Actor<Tasks>;

#[derive(Clone)]
pub struct Task {
  pub command: Command,
  pub then: Reply,
  pub speaks: Speaks,
  application: Application,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaks {
  AtTheEnd,
  OutLoud,
}

impl Task {
  pub fn of(command: Command) -> Result<Task, Never> {
    let Ok(then) = nothing_to_reply();

    Ok(Task {
      command,
      then,
      speaks: Speaks::AtTheEnd,
      application: Application("Console"),
    })
  }

  pub fn out_loud(self) -> Result<Task, Never> {
    Ok(Task {
      speaks: Speaks::OutLoud,
      ..self
    })
  }

  pub fn from_application(self, application: Application) -> Result<Task, Never> {
    Ok(Task {
      application,
      ..self
    })
  }

  pub fn then(
    self,
    reply: impl Fn(&Answer) -> Result<Option<Notification>, Never> + Send + Sync + 'static,
  ) -> Result<Task, Never> {
    let Ok(then) = retained(reply);

    Ok(Task { then, ..self })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
  pub command: Command,
  pub row: Command,
  pub identity: Option<ProcessIdentity>,
  pub interruption: Interruption,
}

impl Started {
  pub fn of(command: Command, row: Command) -> Result<Started, Never> {
    Ok(Started {
      command,
      row,
      identity: None,
      interruption: Interruption::None,
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Began {
  pub command: Command,
  pub identity: ProcessIdentity,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Running {
  pub started: Vec<Started>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
  pub answer: Answer,
  pub notification: Option<Notification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskEvent {
  Start(Started),
  Began(Began),
  Interrupt(Command),
  Ended(Ended),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskEffect {
  Run(Started),
  Interrupt(ProcessIdentity, Insistence),
  Notify(Notification),
}

impl EffectDependencies for TaskEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Run(_) | Self::Interrupt(..) => &[Service::Process],
      Self::Notify(_) => &[Service::Notifications],
    })
  }
}

impl Running {
  pub fn running(&self, command: &Command) -> Result<Progress, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self
          .started
          .iter()
          .any(|started| &started.command == command),
        (),
      ) {
        ::core::option::Option::Some(()) => Progress::Running,
        ::core::option::Option::None => Progress::Idle,
      },
    )
  }

  pub fn stands_for(&self, named: &Command) -> Result<Progress, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self
          .started
          .iter()
          .any(|started| &started.command == named || &started.row == named),
        (),
      ) {
        ::core::option::Option::Some(()) => Progress::Running,
        ::core::option::Option::None => Progress::Idle,
      },
    )
  }

  pub fn answered(&self, answer: Answer) -> Result<Answer, Never> {
    let interruption = self
      .started
      .iter()
      .find(|started| started.command == answer.command)
      .map(|started| started.interruption);
    let Ok(status) = match interruption {
      Some(Interruption::Requested | Interruption::Insisting) => {
        answer.status.after_interrupting()
      }
      Some(Interruption::None) | None => Ok(answer.status),
    };

    Ok(Answer { status, ..answer })
  }
}

pub struct Tasks;

impl Machine for Tasks {
  type Input = ();
  type State = Running;
  type Request = TaskEvent;
  type Effect = TaskEffect;

  fn initialize(
    _input: &(),
    previous: Option<Running>,
    _effects: &mut Queue<TaskEffect>,
  ) -> Result<Running, Never> {
    Ok(match previous {
      Some(running) => running,
      None => Running::default(),
    })
  }

  fn handle(
    state: Running,
    request: TaskEvent,
    effects: &mut Queue<TaskEffect>,
  ) -> Result<Running, Never> {
    match request {
      TaskEvent::Start(started) => start(state, started, effects),
      TaskEvent::Began(began) => begin(state, began, effects),
      TaskEvent::Interrupt(command) => interrupt(state, &command, effects),
      TaskEvent::Ended(ended) => end(state, ended, effects),
    }
  }
}

fn start(
  state: Running,
  started: Started,
  effects: &mut Queue<TaskEffect>,
) -> Result<Running, Never> {
  let Ok(already) = state.running(&started.command);

  match already {
    Progress::Running => Ok(state),
    Progress::Idle => {
      let Ok(()) = effects.offer(TaskEffect::Run(started.clone()));
      let mut running = state;

      running.started.push(started);

      Ok(running)
    }
  }
}

fn begin(
  state: Running,
  began: Began,
  effects: &mut Queue<TaskEffect>,
) -> Result<Running, Never> {
  let mut running = state;

  for started in running
    .started
    .iter_mut()
    .filter(|started| started.command == began.command)
  {
    started.identity = Some(began.identity);

    match started.interruption {
      Interruption::None => {}
      Interruption::Requested => {
        let Ok(()) = effects.offer(TaskEffect::Interrupt(began.identity, Insistence::Ask));
      }
      Interruption::Insisting => {
        let Ok(()) = effects.offer(TaskEffect::Interrupt(began.identity, Insistence::Insist));
      }
    }
  }

  Ok(running)
}

fn interrupt(
  state: Running,
  command: &Command,
  effects: &mut Queue<TaskEffect>,
) -> Result<Running, Never> {
  let mut running = state;

  for started in running
    .started
    .iter_mut()
    .filter(|started| &started.command == command || &started.row == command)
  {
    let Ok((insistence, interruption)) = started.interruption.again();

    started.interruption = interruption;

    match started.identity {
      Some(identity) => {
        let Ok(()) = effects.offer(TaskEffect::Interrupt(identity, insistence));
      }
      None => {}
    }
  }

  Ok(running)
}

fn end(
  mut state: Running,
  ended: Ended,
  effects: &mut Queue<TaskEffect>,
) -> Result<Running, Never> {
  state
    .started
    .retain(|task| task.command != ended.answer.command);

  match ended.notification {
    Some(notification) => {
      let Ok(()) = effects.offer(TaskEffect::Notify(notification));
    }
    None => {}
  }

  Ok(state)
}

pub fn shown(rows: Vec<Row>, running: &Running) -> Result<Vec<Row>, Never> {
  Ok(
    rows
      .into_iter()
      .map(|row| {
        let Ok(row_2) = lit(row, running);

        row_2
      })
      .collect(),
  )
}

fn lit(row: Row, running: &Running) -> Result<Row, Never> {
  let named = match &row.while_running {
    Some(named) => named.clone(),
    None => return Ok(row),
  };
  let Ok(progress) = running.stands_for(&named.command);

  Ok(match progress {
    Progress::Idle => Row {
      while_running: Some(WhileRunning { progress, ..named }),
      ..row
    },
    Progress::Running => {
      let Ok(stop) = stopping(&named);

      Row {
        says: named.says.clone(),
        aside: YET.to_string(),
        does: None,
        more: Some(stop),
        while_running: Some(WhileRunning { progress, ..named }),
        ..row
      }
    }
  })
}

fn stopping(named: &WhileRunning) -> Result<Act, Never> {
  let asked = format!("{STOP} {}?", named.says);
  let command = named.command.clone();

  let Ok(stop) = retained(move |showing: &dyn Showing| {
    let stopped = command.clone();

    showing.sure(
      &asked,
      Subject(""),
      &[STOP],
      Box::new(move |showing_2, _stop| showing_2.interrupt(&stopped)),
    );

    ActionOutcome::Stay
  });

  Ok(stop)
}

pub fn kept() -> Result<console_actor::Booted<Tasks>, console_actor::BootError> {
  console_actor::boot::<Tasks>(())
}

pub fn start_from(tasks: &Actor<Tasks>, task: Task, row: Option<&Command>) -> Result<(), Never> {
  let Task {
    command,
    then,
    speaks,
    application,
  } = task;
  let row_3 = match row {
    Some(row_2) => row_2.clone(),
    None => command.clone(),
  };
  let Ok(started) = Started::of(command, row_3);
  let Ok(effects) = sent(tasks, TaskEvent::Start(started));

  carry(tasks, effects, (&then, speaks, application))
}

pub fn interrupt_from(tasks: &Actor<Tasks>, command: &Command) -> Result<(), Never> {
  let Ok(effects) = sent(tasks, TaskEvent::Interrupt(command.clone()));
  let Ok(nothing) = nothing_to_reply();

  carry(
    tasks,
    effects,
    (&nothing, Speaks::AtTheEnd, Application("Console")),
  )
}

pub fn running_now(tasks: &Actor<Tasks>) -> Result<Running, Never> {
  Ok(match tasks.get() {
    Ok(running) => running,
    Err(gone) => {
      console_core_logging::warn!("console-panel: what is running could not be read: {gone}");

      Running::default()
    }
  })
}

fn nothing_to_reply() -> Result<Reply, Never> {
  let Ok(ignored) = retained(|_answer: &Answer| Ok(None));

  Ok(ignored)
}

fn sent(tasks: &Actor<Tasks>, event: TaskEvent) -> Result<Vec<TaskEffect>, Never> {
  Ok(match tasks.send(event) {
    Ok(effects) => effects,
    Err(SendError::MachineDefect(effects)) => effects,
    Err(SendError::Closed) => {
      console_core_logging::warn!(
        "console-panel: what is running is no longer kept, so a task was neither started nor heard"
      );

      Vec::new()
    }
  })
}

fn carry(
  tasks: &Actor<Tasks>,
  effects: Vec<TaskEffect>,
  (then, speaks, application): (&Reply, Speaks, Application),
) -> Result<(), Never> {
  for effect in effects {
    match effect {
      TaskEffect::Run(started) => {
        let Ok(()) = run(tasks, started, (then, speaks, application));
      }
      TaskEffect::Interrupt(identity, insistence) => {
        let Ok(_its_answer_is_on_the_way_either_way) = identity.interrupt(insistence);
      }
      TaskEffect::Notify(notice) => {
        let Ok(()) =
          console_notifications::notify(application, &notice.summary, notice.body.as_deref());
      }
    }
  }

  Ok(())
}

fn run(
  tasks: &Actor<Tasks>,
  started: Started,
  (then, speaks, application): (&Reply, Speaks, Application),
) -> Result<(), Never> {
  let tasks_2 = tasks.clone();
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "a public task completion closure remains registered for later commands while this worker retains its own invocation"
    )
  )]
  let then_2 = then.clone();

  let running = thread::spawn(move || {
    let Ok(ran) = ran(&tasks_2, &started.command, speaks, application);
    let Ok(running) = running_now(&tasks_2);
    let Ok(answer) = running.answered(ran);
    let Ok(notification) = then_2(&answer);
    let Ok(effects) = sent(
      &tasks_2,
      TaskEvent::Ended(Ended {
        answer,
        notification,
      }),
    );
    let Ok(nothing) = nothing_to_reply();
    let Ok(()) = carry(&tasks_2, effects, (&nothing, Speaks::AtTheEnd, application));

    let Ok(()) = crate::frames::tell(crate::frames::Notice::Rows);
  });

  let Ok(()) = crate::frames::tell(crate::frames::Notice::Rows);

  threads::let_go(running)
}

struct TaskOutput {
  live: LiveNotification,
  speaks: Speaks,
  lines: Vec<String>,
}

impl Reporter for TaskOutput {
  fn line(&mut self, line: &str) -> Result<(), Never> {
    let line_2 = line.trim().to_string();

    match (self.speaks, line_2.is_empty()) {
      (Speaks::AtTheEnd, true | false) | (Speaks::OutLoud, true) => {}
      (Speaks::OutLoud, false) => {
        let Ok(()) = self.live.update(&line_2);
      }
    }

    self.lines.push(line_2);

    Ok(())
  }
}

fn ran(
  tasks: &Actor<Tasks>,
  command: &Command,
  speaks: Speaks,
  application: Application,
) -> Result<Answer, Never> {
  let Ok(mut platform) = Linux::new(Timing::Background);
  let starting = match console_program_runtime::commands::resolve(&mut platform, command) {
    Ok(invocation) => platform.fork(&invocation, Streams::COLLECT),
    Err(fault) => Err(fault),
  };
  let started = match starting {
    Ok(started) => started,
    Err(fault) => {
      let Ok(name) = command.program.name();

      return Ok(Answer {
        job: None,
        command: command.clone(),
        output: String::new(),
        error_output: format!("{name} would not run: {fault}"),
        status: ExitStatus::Failure(None),
      });
    }
  };
  let Ok(identity) = started.identity();
  let Ok(began) = sent(
    tasks,
    TaskEvent::Began(Began {
      command: command.clone(),
      identity,
    }),
  );
  let Ok(nothing) = nothing_to_reply();
  let Ok(()) = carry(tasks, began, (&nothing, Speaks::AtTheEnd, application));
  let Ok(live) = LiveNotification::new(application);
  let mut output = TaskOutput {
    live,
    speaks,
    lines: Vec::new(),
  };
  let completion = commands::finished(started, Some(&mut output));

  Ok(match completion {
    Ok(completion_2) => Answer {
      job: None,
      command: command.clone(),
      output: output.lines.join("\n").trim().to_string(),
      error_output: completion_2.error_output.trim().to_string(),
      status: completion_2.status,
    },
    Err(fault) => {
      let Ok(name) = command.program.name();

      Answer {
        job: None,
        command: command.clone(),
        output: String::new(),
        error_output: format!("{name} would not run: {fault}"),
        status: ExitStatus::Failure(None),
      }
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  use std::process::Stdio;

  use console_core_external_programs::Program;
  use console_operating_system_linux::program_lifetime::forked;

  use crate::page::Aside;

  type Failure = Box<dyn std::error::Error>;

  fn pairing(address: &str) -> Result<Command, Never> {
    Command::external(Program::Busctl, &["pair", address])
  }

  fn with_a_pin(address: &str) -> Result<Command, Never> {
    Command::external(Program::Busctl, &["pair", "--pin", "0000", address])
  }

  fn answered(command: &Command, status: ExitStatus) -> Result<Answer, Never> {
    Ok(Answer {
      job: None,
      command: command.clone(),
      output: String::new(),
      error_output: String::new(),
      status,
    })
  }

  fn said(summary: &str) -> Result<Notification, Never> {
    Notification::of(summary)
  }

  fn an_identity() -> Result<ProcessIdentity, Failure> {
    let Ok(mut sleeping) = Program::Sh.command();

    sleeping
      .args(["-c", "exec sleep 600"])
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null());

    let fork = forked(&mut sleeping)?;
    let Ok(identity) = fork.identity();

    Ok(identity)
  }

  fn row_for(command: &Command) -> Result<Row, Failure> {
    let Ok(does) = crate::page::Handler::and_stay(|_showing| {});
    let Ok(row) = Row::new("Pair", Aside(""), does);
    let Ok(row_2) = row.while_running(command, "Pairing");

    Ok(row_2)
  }

  #[test]
  fn a_second_press_while_the_first_runs_starts_nothing() {
    let Ok(one) = pairing("A");
    let started = Started {
      command: one.clone(),
      row: one.clone(),
      identity: None,
      interruption: Interruption::None,
    };
    let Ok(said) = console_core_state_machine::run::<Tasks>(
      &(),
      &[
        TaskEvent::Start(started.clone()),
        TaskEvent::Start(started.clone()),
      ],
    );

    assert_eq!(
      said.on(0),
      Ok(Some([TaskEffect::Run(started.clone())].as_slice()))
    );
    assert_eq!(
      said.on(1),
      Ok(Some([].as_slice())),
      "the same command was started twice at once"
    );
    assert_eq!(said.state.started, vec![started]);
  }

  #[test]
  fn once_it_has_answered_it_can_be_pressed_again() -> Result<(), Failure> {
    let Ok(one) = pairing("A");
    let started = Started {
      command: one.clone(),
      row: one.clone(),
      identity: None,
      interruption: Interruption::None,
    };
    let Ok(answer) = answered(&one, ExitStatus::Success);
    let Ok(said) = console_core_state_machine::run::<Tasks>(
      &(),
      &[
        TaskEvent::Start(started.clone()),
        TaskEvent::Ended(Ended {
          answer,
          notification: None,
        }),
        TaskEvent::Start(started.clone()),
      ],
    );

    assert_eq!(said.on(2), Ok(Some([TaskEffect::Run(started)].as_slice())));

    Ok(())
  }

  #[test]
  fn two_devices_pair_apart() {
    let Ok(one) = pairing("A");
    let Ok(other) = pairing("B");
    let first = Started {
      command: one.clone(),
      row: one,
      identity: None,
      interruption: Interruption::None,
    };
    let second = Started {
      command: other.clone(),
      row: other,
      identity: None,
      interruption: Interruption::None,
    };
    let Ok(said) = console_core_state_machine::run::<Tasks>(
      &(),
      &[
        TaskEvent::Start(first.clone()),
        TaskEvent::Start(second.clone()),
      ],
    );

    assert_eq!(
      said.effects(),
      Ok(vec![TaskEffect::Run(first), TaskEffect::Run(second)])
    );
  }

  #[test]
  fn a_row_is_in_progress_while_its_command_runs_and_itself_again_after() -> Result<(), Failure> {
    let Ok(one) = pairing("A");
    let Ok(other) = pairing("B");
    let first = row_for(&one)?;
    let second = row_for(&other)?;
    let rows = vec![first, second];
    let running = Running {
      started: vec![Started {
        command: one.clone(),
        row: one.clone(),
        identity: None,
        interruption: Interruption::None,
      }],
      ..Running::default()
    };
    let Ok(shown) = shown(rows.clone(), &running);
    let lit = shown.first().ok_or("no first row")?;
    let unlit = shown.get(1).ok_or("no second row")?;

    assert_eq!((lit.says.as_str(), lit.aside.as_str()), ("Pairing", YET));
    assert!(lit.does.is_none(), "a row in progress can still be pressed");
    assert_eq!(lit.progress(), Ok(Progress::Running));
    assert_eq!(
      (unlit.says.as_str(), unlit.progress()),
      ("Pair", Ok(Progress::Idle)),
      "the other device's row was lit by this one's pairing"
    );

    let Ok(after) = super::shown(rows, &Running::default());
    let back = after.first().ok_or("no first row")?;

    assert_eq!((back.says.as_str(), back.aside.as_str()), ("Pair", ""));
    assert!(back.does.is_some());

    Ok(())
  }

  #[test]
  fn a_row_stands_for_what_its_press_started_when_that_was_not_the_command_it_names()
  -> Result<(), Failure> {
    let Ok(plain) = pairing("A");
    let Ok(typed) = with_a_pin("A");
    let running = Running {
      started: vec![Started {
        command: typed,
        row: plain.clone(),
        identity: None,
        interruption: Interruption::None,
      }],
      ..Running::default()
    };
    let row = row_for(&plain)?;
    let Ok(shown) = shown(vec![row], &running);
    let lit = shown.first().ok_or("no row")?;

    assert_eq!(lit.progress(), Ok(Progress::Running));

    Ok(())
  }

  #[test]
  fn a_task_posts_its_reply_once_for_each_completion_status() {
    let Ok(command) = pairing("A");
    let Ok(reply) = said("Pairing ended");
    let statuses = [
      ExitStatus::Success,
      ExitStatus::Failure(Some(1)),
      ExitStatus::Interrupt,
    ];

    for status in statuses {
      let Ok(answer) = answered(&command, status);
      let Ok(started) = Started::of(command.clone(), command.clone());
      let Ok(trace) = console_core_state_machine::run::<Tasks>(
        &(),
        &[
          TaskEvent::Start(started),
          TaskEvent::Ended(Ended {
            answer,
            notification: Some(reply.clone()),
          }),
        ],
      );

      assert_eq!(
        trace.on(1),
        Ok(Some([TaskEffect::Notify(reply.clone())].as_slice()))
      );
      assert!(trace.state.started.is_empty());
    }
  }

  #[test]
  fn a_program_is_answered_with_what_it_printed_what_it_complained_of_and_how_it_ended()
  -> Result<(), Failure> {
    let Ok(_turn) = crate::frames::serial_fixture();
    let kept = kept()?;
    let Ok(talking) = Command::external(
      Program::Sh,
      &["-c", "echo one; echo two; echo why >&2; exit 3"],
    );
    let Ok(started) = sent(
      &kept.actor,
      TaskEvent::Start(Started {
        command: talking.clone(),
        row: talking.clone(),
        identity: None,
        interruption: Interruption::None,
      }),
    );
    let Ok(answer) = ran(
      &kept.actor,
      &talking,
      Speaks::AtTheEnd,
      Application("Console"),
    );

    assert_eq!(started.len(), 1);
    assert_eq!(answer.output, "one\ntwo");
    assert_eq!(answer.error_output, "why");
    assert_eq!(answer.status, ExitStatus::Failure(Some(3)));
    let Ok(()) = kept.scope.close();

    Ok(())
  }

  #[test]
  fn an_interrupt_is_said_to_the_process_the_command_began_as() -> Result<(), Failure> {
    let Ok(one) = pairing("A");
    let Ok(started) = Started::of(one.clone(), one.clone());
    let identity = an_identity()?;
    let Ok(said) = console_core_state_machine::run::<Tasks>(
      &(),
      &[
        TaskEvent::Start(started),
        TaskEvent::Began(Began {
          command: one.clone(),
          identity,
        }),
        TaskEvent::Interrupt(one.clone()),
        TaskEvent::Interrupt(one),
      ],
    );

    assert_eq!(
      said.on(1),
      Ok(Some([].as_slice())),
      "a command that began was interrupted before anyone asked"
    );
    assert_eq!(
      said.on(2),
      Ok(Some(
        [TaskEffect::Interrupt(identity, Insistence::Ask)].as_slice()
      ))
    );
    assert_eq!(
      said.on(3),
      Ok(Some(
        [TaskEffect::Interrupt(identity, Insistence::Insist)].as_slice()
      )),
      "asking twice did not insist"
    );

    Ok(())
  }

  #[test]
  fn an_interrupt_that_came_before_the_command_began_is_said_when_it_begins()
  -> Result<(), Failure> {
    let Ok(one) = pairing("A");
    let Ok(started) = Started::of(one.clone(), one.clone());
    let identity = an_identity()?;
    let Ok(said) = console_core_state_machine::run::<Tasks>(
      &(),
      &[
        TaskEvent::Start(started),
        TaskEvent::Interrupt(one.clone()),
        TaskEvent::Began(Began {
          command: one,
          identity,
        }),
      ],
    );

    assert_eq!(said.on(1), Ok(Some([].as_slice())));
    assert_eq!(
      said.on(2),
      Ok(Some(
        [TaskEffect::Interrupt(identity, Insistence::Ask)].as_slice()
      )),
      "the interrupt was lost while the command started"
    );

    Ok(())
  }

  #[test]
  fn a_row_interrupts_what_its_own_press_started() -> Result<(), Failure> {
    let Ok(plain) = pairing("A");
    let Ok(typed) = with_a_pin("A");
    let Ok(other) = pairing("B");
    let Ok(started) = Started::of(typed.clone(), plain.clone());
    let Ok(beside) = Started::of(other.clone(), other.clone());
    let identity = an_identity()?;
    let elsewhere = an_identity()?;
    let Ok(said) = console_core_state_machine::run::<Tasks>(
      &(),
      &[
        TaskEvent::Start(started),
        TaskEvent::Start(beside),
        TaskEvent::Began(Began {
          command: typed,
          identity,
        }),
        TaskEvent::Began(Began {
          command: other,
          identity: elsewhere,
        }),
        TaskEvent::Interrupt(plain),
      ],
    );

    assert_eq!(
      said.on(4),
      Ok(Some(
        [TaskEffect::Interrupt(identity, Insistence::Ask)].as_slice()
      ))
    );

    Ok(())
  }

  #[test]
  fn interrupting_what_is_not_running_does_nothing() {
    let Ok(one) = pairing("A");
    let Ok(said) = console_core_state_machine::run::<Tasks>(&(), &[TaskEvent::Interrupt(one)]);

    assert_eq!(said.effects(), Ok(Vec::new()));
    assert_eq!(said.state, Running::default());
  }

  #[test]
  fn an_answer_after_an_interrupt_says_it_was_interrupted_unless_it_finished_first()
  -> Result<(), Failure> {
    let Ok(one) = pairing("A");
    let Ok(started) = Started::of(one.clone(), one.clone());
    let asked = Running {
      started: vec![Started {
        interruption: Interruption::Requested,
        ..started.clone()
      }],
      ..Running::default()
    };
    let unasked = Running {
      started: vec![started],
      ..Running::default()
    };
    let Ok(cut_short) = answered(&one, ExitStatus::Failure(None));
    let Ok(finished) = answered(&one, ExitStatus::Success);

    assert_eq!(
      asked
        .answered(cut_short.clone())
        .map(|answer| answer.status),
      Ok(ExitStatus::Interrupt)
    );
    assert_eq!(
      asked.answered(finished).map(|answer| answer.status),
      Ok(ExitStatus::Success)
    );
    assert_eq!(
      unasked.answered(cut_short).map(|answer| answer.status),
      Ok(ExitStatus::Failure(None))
    );

    Ok(())
  }

  #[test]
  fn a_row_in_progress_can_be_asked_to_stop_and_one_that_is_not_cannot() -> Result<(), Failure> {
    let Ok(one) = pairing("A");
    let row = row_for(&one)?;
    let Ok(started) = Started::of(one.clone(), one);
    let running = Running {
      started: vec![started],
      ..Running::default()
    };
    let Ok(lit) = shown(vec![row.clone()], &running);
    let Ok(unlit) = shown(vec![row], &Running::default());

    assert!(
      lit
        .first()
        .is_some_and(|running_row| running_row.more.is_some()),
      "a row in progress has nothing else to offer, so nothing can stop it"
    );
    assert!(
      unlit
        .first()
        .is_some_and(|idle_row| idle_row.more.is_none())
    );

    Ok(())
  }
}
