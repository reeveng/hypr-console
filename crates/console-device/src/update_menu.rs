//! The desktop's update panel and its approval handoff.

use std::error::Error;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};

use console_core_external_programs::Program;
use console_core_internal_programs::{CONFIRM_DOES, InternalProgram};
use console_core_never::Never;
use console_notifications::delivery::Options;
use console_panel::card::{Build, Card, Door};
use console_panel::handoff::{self, DrawnBy};
use console_panel::page::{ActionOutcome, Aside, Handler, Page, Row, Rows};
use console_panel::picker::{self, Again, Alone};
use console_panel::surface;
use console_program_lifetime::threads;

mod commands;
mod record;
mod work;

use commands::diagnostic;
pub use commands::{checked, output};
pub use record::{Hex, PreviousRelease, ReleaseSlot, UndoFault, hex, undo_target};
use work::{Check, CheckResult, Rollback, View, Work};

pub const WHO: &str = "console-update";
pub const PROMPT_ARGUMENT: &str = "--prompt";
pub const PROMPT: console_core_arguments::Flag = console_core_arguments::Flag {
  spelling: PROMPT_ARGUMENT,
  takes: console_core_arguments::Takes::None,
  about: "open Console Updates",
};
pub const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: WHO,
  about: "choose when to install or undo a Console update",
  flags: &[PROMPT],
  operands: console_core_arguments::Operands::None,
};
pub const AVAILABLE_UPDATE: &str = "--available-update";

const UNDO: &str = "Undo last update";
const AUTHENTICATION_CANCELED: i32 = 126;

pub type UpdateError = Box<dyn Error>;

pub enum ReleaseSupport {
  Tagged,
  External,
}

pub fn release_support() -> std::io::Result<ReleaseSupport> {
  match std::fs::read_to_string("/sys/class/dmi/id/product_name") {
    Ok(product) => Ok(match product.trim() {
      "83E1" => ReleaseSupport::Tagged,
      _ => ReleaseSupport::External,
    }),
    Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(ReleaseSupport::External),
      None => Err(fault),
    },
  }
}

pub fn door(_arguments: &[String]) -> Result<Door, Never> {
  Door::closing(WHO)
}

#[derive(Clone)]
enum Choice {
  Install(String),
  Undo(PreviousRelease),
}

pub fn open() -> Result<(), UpdateError> {
  let Ok(alone) = picker::alone_once_drawn(WHO, Again::Closes);

  match alone {
    Alone::No => return Ok(()),
    Alone::Yes => {}
  }

  let Ok(drawn) = handoff::stood_in(WHO, &[PROMPT_ARGUMENT.to_owned()]);

  match drawn {
    DrawnBy::ByTheHost => Ok(()),
    DrawnBy::Here => open_here(),
  }
}

fn open_here() -> Result<(), UpdateError> {
  let support = release_support()?;

  match support {
    ReleaseSupport::External => {
      let Ok(build) = external_build();
      let Ok(()) = surface::show(build, 0, None);

      Ok(())
    }
    ReleaseSupport::Tagged => {
      let Ok((build, work, selected)) = prepare();
      let Ok(()) = surface::show(build, 0, None);
      let Ok(()) = work.close();

      choose_after_close(&selected)
    }
  }
}

pub fn card(_asked: &[String]) -> Result<Card, Never> {
  match release_support() {
    Ok(ReleaseSupport::Tagged) => tagged_card(),
    Ok(ReleaseSupport::External) => {
      let Ok(build) = external_build();
      Card::new(build)
    }
    Err(fault) => {
      console_core_logging::warn!("Console Updates: reading hardware model: {fault}");
      let Ok(build) = external_build();
      Card::new(build)
    }
  }
}

fn external_build() -> Result<Build, Never> {
  Ok(Box::new(|| {
    let Ok(deployment) = Row::text("Update method", Aside("Source or system package"));
    let Ok(tagged) = Row::text("Tagged releases", Aside("Legion Go 83E1"));
    let Ok(settings) = InternalProgram::SettingsPanel.path();
    let Ok(open) = Handler::run(&[settings, "Configuration"]);
    let Ok(about) = Row::new("About Console", Aside(""), open);
    let Ok(page) = Page::new(
      "Console Updates",
      Rows::Fixed(vec![deployment, tagged, about]),
    );
    let Ok(compact) = page.compact();

    vec![compact]
  }))
}

fn tagged_card() -> Result<Card, Never> {
  let Ok((build, work, selected)) = prepare();
  let Ok(card) = Card::new(build);

  card.shutting(Box::new(move || {
    let Ok(()) = work.close();
    let choice = match selected.try_recv() {
      Ok(choice) => choice,
      Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
    };

    match std::thread::Builder::new().spawn(move || match chosen(choice) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("Console Updates: {fault}"),
    }) {
      Ok(worker) => threads::let_go(worker),
      Err(fault) => {
        console_core_logging::warn!(
          "Console Updates: could not start the approval prompt: {fault}"
        );
        Ok(())
      }
    }
  }))
}

fn prepare() -> Result<(Build, Work, Receiver<Choice>), Never> {
  let (select, selected) = sync_channel(1);
  let Ok((work, checking, initial)) = Work::new();
  let reader = work.clone();
  let build = Box::new(move || {
    let actions = Actions { checking, select };
    let Ok(first) = rows(&initial, &actions);
    let Ok(current) = Rows::computed(move || match reader.view() {
      Ok(Some(view)) => {
        let Ok(drawn) = rows(&view, &actions);

        drawn
      }
      Ok(None) => Vec::new(),
    });
    let Ok(page) = Page::new("Console Updates", current);
    let Ok(compact) = page.compact();
    let Ok(menu) = compact.meanwhile(move || first.clone());

    vec![menu]
  });

  Ok((build, work, selected))
}

fn choose_after_close(selected: &Receiver<Choice>) -> Result<(), UpdateError> {
  match selected.try_recv() {
    Ok(choice) => chosen(choice),
    Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(()),
  }
}

fn chosen(choice: Choice) -> Result<(), UpdateError> {
  match choice {
    Choice::Install(tag) => install_prompt(&tag),
    Choice::Undo(previous) => match undo(&previous) {
      Ok(()) => Ok(()),
      Err(fault) => {
        let Ok(mut notice) =
          Options::default().command("Could not undo Console update", Some(&fault.to_string()));

        match checked(&mut notice) {
          Ok(()) => {}
          Err(posting) => {
            console_core_logging::warn!(
              "Console Updates: could not show the rollback failure: {posting}"
            )
          }
        }

        Err(fault)
      }
    },
  }
}

struct Actions {
  checking: SyncSender<u64>,
  select: SyncSender<Choice>,
}

fn rows(view: &View, actions: &Actions) -> Result<Vec<Row>, Never> {
  let Ok(check) = match &view.check {
    Check::Checking(_) => Row::naming("Checking for updates…", Aside("")),
    Check::Ready { attempt, result } => {
      let aside = match result {
        CheckResult::NotChecked => "",
        CheckResult::Current => "Up to date",
        CheckResult::Available(_) => "Update available",
        CheckResult::Failed(_) => "Check failed",
      };
      let Ok(action) = check_action(actions.checking.clone(), *attempt);

      Row::new("Check for updates", Aside(aside), action)
    }
  };
  let Ok(undo) = undo_row(&view.rollback, &actions.select);
  let mut drawn = vec![check, undo];

  match &view.check {
    Check::Ready {
      result: CheckResult::Available(tag),
      ..
    } => {
      let Ok(action) = choose(actions.select.clone(), Choice::Install(tag.clone()));
      let Ok(install) = Row::new("Install update", Aside(tag), action);

      drawn.push(install);
    }
    Check::Ready {
      result: CheckResult::Failed(fault),
      ..
    } => {
      let detail = fault.clone();
      let Ok(action) = Handler::and_stay(move |_| {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Console Updates"),
          "Could not check for updates",
          Some(&detail),
        );
      });
      let Ok(reason) = Row::new("Could not check for updates", Aside("Details"), action);

      drawn.push(reason);
    }
    Check::Checking(_)
    | Check::Ready {
      result: CheckResult::NotChecked | CheckResult::Current,
      ..
    } => {}
  }

  Ok(drawn)
}

#[cfg_attr(
  dylint_lib = "explicit063_fault_has_recovery",
  expect(
    explicit063_fault_has_recovery,
    reason = "a full queue already holds the single check requested by this row"
  )
)]
fn check_action(checking: SyncSender<u64>, seen: u64) -> Result<Handler, Never> {
  Handler::and_stay(move |_| match checking.try_send(seen) {
    Ok(()) | Err(TrySendError::Full(_)) => {}
    Err(TrySendError::Disconnected(_)) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Console Updates"),
        "Could not start the update check",
        None,
      );
    }
  })
}

fn undo_row(rollback: &Rollback, select: &SyncSender<Choice>) -> Result<Row, Never> {
  match rollback {
    Rollback::Checking => Row::naming(UNDO, Aside("Checking previous release…")),
    Rollback::Available(previous) => {
      let Ok(action) = choose(select.clone(), Choice::Undo(previous.clone()));
      Row::new(UNDO, Aside(&previous.label), action)
    }
    Rollback::Absent => Row::naming(UNDO, Aside("No previous release")),
    Rollback::Pending => Row::naming(UNDO, Aside("A release switch is pending")),
    Rollback::Unavailable => Row::naming(UNDO, Aside("Previous release unavailable")),
  }
}

fn choose(send: SyncSender<Choice>, choice: Choice) -> Result<Handler, Never> {
  Handler::call(move |_| {
    let _selection_is_reported_once = send.try_send(choice.clone());
    ActionOutcome::Close
  })
}

fn rollback_command() -> Result<Command, Never> {
  let Ok(updater) = InternalProgram::Update.path();
  let Ok(mut sudo) = Program::Sudo.command_from(Path::new("/usr/bin"));
  sudo.args(["-n", "--", updater, "--rollback-status"]);

  Ok(sudo)
}

fn previous(status: &str) -> Result<Rollback, UpdateError> {
  match status {
    "none" => Ok(Rollback::Absent),
    "pending" => Ok(Rollback::Pending),
    record => {
      let (target, label) = record
        .split_once('\n')
        .ok_or("invalid previous release status")?;
      undo_target(target)?;
      Ok(Rollback::Available(PreviousRelease {
        target: target.to_owned(),
        label: label.to_owned(),
      }))
    }
  }
}

fn undo(PreviousRelease { target, label }: &PreviousRelease) -> Result<(), UpdateError> {
  let Ok(updater) = InternalProgram::Update.path();
  let Ok(mut sudo) = Program::Sudo.command_from(Path::new("/usr/bin"));
  let approval = output(sudo.args(["-n", "--", updater, "--pattern-status"]))?;

  match approval.as_str() {
    "enrolled" => {
      let Ok(dialog) = InternalProgram::UpdatePattern.path();
      return checked(Command::new(dialog).args(["--undo", target.as_str(), label.as_str()]));
    }
    "absent" => {}
    _ => {
      return Err(UpdateError::from(
        "the updater returned an invalid pattern status",
      ));
    }
  }

  let Ok(confirm) = InternalProgram::Confirm.path();
  let confirmed = Command::new(confirm)
    .env(CONFIRM_DOES, UNDO)
    .arg(format!(
      "Restore {label}? This will restart Console services."
    ))
    .status()?;

  match confirmed.code() {
    Some(0) => {}
    Some(1) => return Ok(()),
    Some(_) | None => {
      return Err(UpdateError::from(
        "could not show the rollback confirmation",
      ));
    }
  }

  let Ok(mut pkexec) = Program::Pkexec.command();
  let restored = pkexec.args([updater, "--undo", target.as_str()]).output()?;

  match restored.status.code() {
    Some(0) => {}
    Some(AUTHENTICATION_CANCELED) => return Ok(()),
    Some(_) | None => {
      let Ok(details) = diagnostic(&restored.stderr);
      return Err(UpdateError::from(format!(
        "rollback exited with {}: {details}",
        restored.status
      )));
    }
  }

  let Ok(mut notice) =
    Options::default().command("Previous Console release restored", Some(label));

  match checked(&mut notice) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("Console Updates: could not show the rollback result: {fault}")
    }
  }

  Ok(())
}

fn install_prompt(tag: &str) -> Result<(), UpdateError> {
  let Ok(updater) = InternalProgram::Update.path();
  let Ok(mut sudo) = Program::Sudo.command_from(Path::new("/usr/bin"));

  let status = output(sudo.args(["-n", "--", updater, "--pattern-status"]))?;

  match status.as_str() {
    "enrolled" => {
      let Ok(dialog) = InternalProgram::UpdatePattern.path();
      return checked(Command::new(dialog).arg(tag));
    }
    "absent" => {}
    _ => {
      return Err(UpdateError::from(
        "the updater returned an invalid pattern status",
      ));
    }
  }

  let question = format!("Install {tag} now? This will restart Console services.");
  let Ok(confirm) = InternalProgram::Confirm.path();
  let accepted = Command::new(confirm)
    .env(console_core_internal_programs::CONFIRM_DOES, "Install")
    .arg(question)
    .status()?;

  match { ::core::primitive::bool::then_some(accepted.success(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(mut pkexec) = Program::Pkexec.command();
      checked(pkexec.args([updater, tag]))?;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}
