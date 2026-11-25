//! The default workspace for new windows, shared with the Console session.

use std::path::Path;

use console_core_atomic_writes::{self as writes, Stored};
use console_core_controls::Switch;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_places::Base;
use console_panel::page::{Aside, Handler, Row, Showing};
use console_panel::tasks::Task;
use console_program_contract::{Command, ExitStatus, Notification};

const PREFERENCE: &str = "new-window-workspace";

pub fn row() -> Result<Option<Row>, Never> {
  let Ok(configuration) = Base::Configuration.ours();
  let at = match configuration {
    Some(folder) => folder.join(PREFERENCE),
    None => return Ok(None),
  };
  let Ok(state) = current_setting(&at);
  let Ok(reload) = reload();
  let applying = reload.command.clone();
  let Ok(change) = Handler::and_stay(move |showing| {
    let Ok(()) = toggle(&at, showing, &reload);
  });
  let Ok(row) = Row::new("Open new windows on a new workspace", Aside(""), change);
  let Ok(switch) = row.switched(state);
  let Ok(with_progress) = switch.while_running(&applying, "Applying window placement");

  Ok(Some(with_progress))
}

fn current_setting(at: &Path) -> Result<Switch, Never> {
  let Ok(saved) = writes::read(at);

  Ok(match saved {
    Stored::Text(value) => match value.trim() {
      "current" => Switch::Off,
      _ => Switch::On,
    },
    Stored::Absent => Switch::On,
    Stored::Failed(fault) => {
      console_core_logging::warn!("settings-panel: reading {PREFERENCE}: {fault}");
      Switch::On
    }
  })
}

fn toggle(at: &Path, showing: &dyn Showing, reload: &Task) -> Result<(), Never> {
  let Ok(state) = current_setting(at);
  let next = match state {
    Switch::On => "current",
    Switch::Off => "new",
  };

  match writes::whole_with_folders(at, next.as_bytes()) {
    Ok(()) => {
      showing.run(reload.clone());
      showing.refresh();
    }
    Err(fault) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        "Could not save window placement",
        Some(&fault.to_string()),
      );
    }
  }

  Ok(())
}

fn reload() -> Result<Task, Never> {
  let Ok(command) = Command::internal(InternalProgram::Compositor, &["reload"]);
  let Ok(task) = Task::of(command);

  let Ok(attributed) = task.from_application(console_notifications::Application("Settings"));
  attributed.then(|answer| {
    Ok(match answer.status {
      ExitStatus::Success | ExitStatus::Interrupt => None,
      ExitStatus::Failure(_) => Some(Notification {
        summary: "Window placement saved. Reload Console to apply it.".to_owned(),
        body: None,
      }),
    })
  })
}
