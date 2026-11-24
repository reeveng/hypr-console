use std::path::Path;
use std::process::Command;

use console_core_external_programs::Program;
use console_core_internal_programs::{CONFIRM_DOES, InternalProgram};
use console_core_never::Never;
use console_operating_system_linux::notification::Options;
use console_panel::page::{ActionOutcome, Aside, Handler, Page, Row, Rows};
use console_panel::surface;

use super::{PreviousRelease, UpdateError, checked, diagnostic, output, prompt, undo_target};

const UNDO: &str = "Undo last update";
const AUTHENTICATION_CANCELED: i32 = 126;

#[derive(Clone)]
enum Choice {
  Check,
  Undo(PreviousRelease),
}

enum Status {
  Available(PreviousRelease),
  Absent,
  Pending,
  Unavailable,
}

pub(super) fn show(live: &Path) -> Result<(), UpdateError> {
  let status = match previous() {
    Ok(status) => status,
    Err(fault) => {
      eprintln!("console-update: could not check the previous release: {fault}");
      Status::Unavailable
    }
  };
  let (send, receive) = std::sync::mpsc::sync_channel(1);
  let Ok(()) = surface::show(
    move || {
      let Ok(check_action) = choose(send.clone(), Choice::Check);
      let Ok(check) = Row::new("Check for updates", Aside(""), check_action);
      let Ok(undo) = match &status {
        Status::Available(previous) => {
          let Ok(action) = choose(send.clone(), Choice::Undo(previous.clone()));
          Row::new(UNDO, Aside(&previous.label), action)
        }
        Status::Absent => Row::naming(UNDO, Aside("No previous release")),
        Status::Pending => Row::naming(UNDO, Aside("A release switch is pending")),
        Status::Unavailable => Row::naming(UNDO, Aside("Previous release unavailable")),
      };
      let Ok(page) = Page::new("Console Updates", Rows::Fixed(vec![check, undo]));
      vec![page]
    },
    0,
    None,
  );

  match receive.try_recv() {
    Ok(Choice::Check) => prompt(live),
    Ok(Choice::Undo(previous)) => match undo(&previous) {
      Ok(()) => Ok(()),
      Err(fault) => {
        let Ok(mut notice) =
          Options::default().command("Could not undo Console update", Some(&fault.to_string()));

        match checked(&mut notice) {
          Ok(()) => {}
          Err(posting) => {
            eprintln!("console-update: could not show the rollback failure: {posting}")
          }
        }

        Err(fault)
      }
    },
    Err(std::sync::mpsc::TryRecvError::Empty | std::sync::mpsc::TryRecvError::Disconnected) => {
      Ok(())
    }
  }
}

fn choose(send: std::sync::mpsc::SyncSender<Choice>, choice: Choice) -> Result<Handler, Never> {
  Handler::call(move |_| {
    let _selection_is_reported_once = send.try_send(choice.clone());
    ActionOutcome::Close
  })
}

fn previous() -> Result<Status, UpdateError> {
  let Ok(updater) = InternalProgram::Update.path();
  let Ok(mut sudo) = Program::Sudo.command_from(Path::new("/usr/bin"));
  let status = output(sudo.args(["-n", "--", updater, "--rollback-status"]))?;

  match status.as_str() {
    "none" => Ok(Status::Absent),
    "pending" => Ok(Status::Pending),
    record => {
      let (target, label) = record
        .split_once('\n')
        .ok_or("invalid previous release status")?;
      undo_target(target)?;
      Ok(Status::Available(PreviousRelease {
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
    Err(fault) => eprintln!("console-update: could not show the rollback result: {fault}"),
  }

  Ok(())
}
