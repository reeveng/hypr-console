//! Power actions run in a separate process so they outlive the Settings panel.

use std::io;
use std::process::ExitCode;

use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};
use console_core_words::Words;
use console_notifications::Application;
use console_notifications::saying::{Content, LiveNotification, report};
use console_panel::page::{ActionOutcome, Aside, Handler, Row};
use console_program_contract::Command;

pub const FLAG: &str = "--power-action";
const APPLICATION: Application = Application("Settings");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Words)]
pub enum Request {
  #[words(
    word = "sleep",
    label = "Sleep",
    failure = "Could not put the device to sleep"
  )]
  Sleep,
  #[words(word = "restart", label = "Restart", failure = "Could not restart")]
  Restart,
  #[words(
    word = "shut-down",
    label = "Shut Down",
    failure = "Could not shut down"
  )]
  ShutDown,
  #[words(word = "log-out", label = "Log Out", failure = "Could not log out")]
  LogOut,
  #[words(
    word = "lock",
    label = "Lock Now",
    failure = "Could not lock the device"
  )]
  Lock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
  Accepted,
  Failed,
}

#[derive(Debug)]
enum Phase {
  PausingMusic,
  Requesting(Request),
  ReportingFailure,
  Done(Outcome),
}

enum Reply {
  MusicPauseFinished,
  PowerFinished(io::Result<()>),
  FailureReported,
}

#[derive(Debug)]
enum Effect {
  PauseMusic,
  RequestPower(Request),
  ReportFailure { request: Request, fault: io::Error },
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::PauseMusic => &[Service::MediaPlayer, Service::Notifications],
      Self::RequestPower(Request::Lock) => {
        &[Service::Session, Service::Process, Service::Notifications]
      }
      Self::RequestPower(Request::LogOut) => &[Service::Session, Service::Process],
      Self::RequestPower(Request::Sleep) => {
        &[Service::Power, Service::Process, Service::Notifications]
      }
      Self::RequestPower(Request::Restart | Request::ShutDown) => {
        &[Service::Power, Service::Process]
      }
      Self::ReportFailure { .. } => &[Service::Notifications],
    })
  }
}

struct Power;

type Turn = SingleTransition<Phase, Effect>;

impl SingleEffectMachine for Power {
  type Input = Request;
  type State = Phase;
  type Request = Reply;
  type Effect = Effect;

  fn initialize(input: &Request, _previous: Option<Phase>) -> Result<Turn, Never> {
    Ok(match *input {
      Request::Sleep => Turn {
        state: Phase::PausingMusic,
        effect: Some(Effect::PauseMusic),
      },
      request @ (Request::Restart | Request::ShutDown | Request::LogOut | Request::Lock) => {
        let Ok(turn) = requesting(request);
        turn
      }
    })
  }

  fn handle(state: Phase, reply: Reply) -> Result<Turn, Never> {
    Ok(match (state, reply) {
      (Phase::PausingMusic, Reply::MusicPauseFinished) => {
        let Ok(turn) = requesting(Request::Sleep);
        turn
      }
      (Phase::Requesting(request), Reply::PowerFinished(Err(fault))) => Turn {
        state: Phase::ReportingFailure,
        effect: Some(Effect::ReportFailure { request, fault }),
      },
      (Phase::Requesting(_), Reply::PowerFinished(Ok(()))) => Turn {
        state: Phase::Done(Outcome::Accepted),
        effect: None,
      },
      (Phase::ReportingFailure, Reply::FailureReported) => Turn {
        state: Phase::Done(Outcome::Failed),
        effect: None,
      },
      (waiting @ Phase::PausingMusic, Reply::PowerFinished(_) | Reply::FailureReported)
      | (waiting @ Phase::Requesting(_), Reply::MusicPauseFinished | Reply::FailureReported)
      | (
        waiting @ Phase::ReportingFailure,
        Reply::MusicPauseFinished | Reply::PowerFinished(_),
      )
      | (waiting @ Phase::Done(_), _) => Turn {
        state: waiting,
        effect: None,
      },
    })
  }
}

fn requesting(request: Request) -> Result<Turn, Never> {
  Ok(Turn {
    state: Phase::Requesting(request),
    effect: Some(Effect::RequestPower(request)),
  })
}

pub fn row(request: Request) -> Result<Row, Never> {
  let Ok(label) = request.label();
  let Ok(word) = request.word();
  let Ok(command) = Command::internal(InternalProgram::SettingsPanel, &[FLAG, word]);
  let Ok(chosen) =
    Handler::call(
      move |_| match console_panel::running::start_in_background(&command) {
        Ok(()) => ActionOutcome::Close,
        Err(fault) => {
          let Ok(()) = request.report(&fault.to_string());

          ActionOutcome::Stay
        }
      },
    );

  Row::new(label, Aside(""), chosen)
}

impl Request {
  pub fn run(self) -> Result<ExitCode, Never> {
    const MAX_EFFECTS: u32 = 3;
    let Ok(first) = Power::initialize(&self, None);
    let Ok(mut progress) = LiveNotification::new(APPLICATION);
    let finished = Power::run_effects(first, MAX_EFFECTS, &mut progress, |notice, effect| {
      let Ok(reply) = carry_out(notice, effect);

      reply
    });

    Ok(match finished {
      Ok(Phase::Done(Outcome::Accepted)) => ExitCode::SUCCESS,
      Ok(Phase::Done(Outcome::Failed)) => ExitCode::FAILURE,
      Ok(incomplete @ (Phase::PausingMusic | Phase::Requesting(_) | Phase::ReportingFailure)) => {
        let Ok(()) = self.report(&format!("The power request stopped in {incomplete:?}"));

        ExitCode::FAILURE
      }
      Err(pending) => {
        let Ok(()) = self.report(&format!(
          "The power request still had {:?} pending",
          pending.effect
        ));

        ExitCode::FAILURE
      }
    })
  }

  fn perform(self) -> io::Result<()> {
    let Ok(mut command) = self.command();
    let output = command.output()?;

    match output.status.success().then_some(()) {
      Some(()) => Ok(()),
      None => {
        let said = match std::str::from_utf8(&output.stderr) {
          Ok(details) => match details.trim() {
            "" => output.status.to_string(),
            detail => detail.to_owned(),
          },
          Err(_invalid) => format!("{} (diagnostics were not UTF-8)", output.status),
        };

        Err(io::Error::other(said))
      }
    }
  }

  fn command(self) -> Result<std::process::Command, Never> {
    Ok(match self {
      Self::Sleep => {
        let Ok(mut command) = Program::Systemctl.command();
        command.arg("suspend");

        command
      }
      Self::Restart => {
        let Ok(command) = finish("reboot");
        command
      }
      Self::ShutDown => {
        let Ok(command) = finish("poweroff");
        command
      }
      Self::LogOut => {
        let Ok(command) = finish("logout");
        command
      }
      Self::Lock => {
        let Ok(mut command) = Program::Systemctl.command();
        command.args(["--user", "start", console_lock_screen::LOCK_SERVICE]);

        command
      }
    })
  }

  fn report(self, body: &str) -> Result<(), Never> {
    let Ok(summary) = self.failure();

    report(APPLICATION, "power-settings", Content { summary, body })
  }
}

fn carry_out(progress: &mut LiveNotification, effect: Effect) -> Result<Reply, Never> {
  Ok(match effect {
    Effect::PauseMusic => {
      let Ok(()) = progress.update("Pausing music…");
      let Ok(()) = console_application_music::player::pause();

      Reply::MusicPauseFinished
    }
    Effect::RequestPower(request) => {
      let Ok(()) = match request {
        Request::Lock => progress.update("Locking the device…"),
        Request::Sleep => progress.update("Preparing to sleep…"),
        Request::Restart | Request::ShutDown | Request::LogOut => Ok(()),
      };

      Reply::PowerFinished(request.perform())
    }
    Effect::ReportFailure { request, fault } => {
      let Ok(()) = request.report(&fault.to_string());

      Reply::FailureReported
    }
  })
}

fn finish(action: &str) -> Result<std::process::Command, Never> {
  let Ok(mut command) = InternalProgram::Resume.command();
  command.args(["finish", "--show-progress", action]);

  Ok(command)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sleep_waits_for_music_before_requesting_suspend_and_reports_a_refusal() {
    let Ok(first) = Power::initialize(&Request::Sleep, None);
    assert!(matches!(first.effect, Some(Effect::PauseMusic)));

    let Ok(second) = Power::handle(first.state, Reply::MusicPauseFinished);
    assert!(matches!(
      second.effect,
      Some(Effect::RequestPower(Request::Sleep))
    ));

    let denied = io::Error::other("sleep inhibited");
    let Ok(third) = Power::handle(second.state, Reply::PowerFinished(Err(denied)));
    assert!(matches!(
      third.effect,
      Some(Effect::ReportFailure {
        request: Request::Sleep,
        ..
      })
    ));

    let Ok(finished) = Power::handle(third.state, Reply::FailureReported);
    assert!(matches!(finished.state, Phase::Done(Outcome::Failed)));
    assert!(finished.effect.is_none());
  }
}
