//! The default workspace for new windows, shared with the Console session.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_compositor::DispatchResult;
use console_core_atomic_writes::Stored;
use console_core_controls::Switch;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_places::Base;
use console_panel::page::{Aside, Handler, Row};
use console_panel::tasks::Task;
use console_program_contract::{Command, ExitStatus, Notification};

const PREFERENCE: &str = "new-window-workspace";
pub const TOGGLE_WINDOW_PLACEMENT: &str = "--toggle-window-placement";

#[derive(Clone, Copy)]
enum Placement {
  Current,
  New,
}

impl Placement {
  fn read(saved: Stored) -> Result<Self, std::io::Error> {
    match saved {
      Stored::Text(value) => Ok(match value.trim() {
        "new" => Self::New,
        _ => Self::Current,
      }),
      Stored::Absent => Ok(Self::Current),
      Stored::Failed(fault) => Err(std::io::Error::other(fault)),
    }
  }

  fn switched(self) -> Result<Switch, Never> {
    Ok(match self {
      Self::Current => Switch::Off,
      Self::New => Switch::On,
    })
  }

  fn toggled_value(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Current => "new",
      Self::New => "current",
    })
  }
}

fn path() -> Result<Option<PathBuf>, Never> {
  let Ok(configuration) = Base::Configuration.ours();

  Ok(configuration.map(|folder| folder.join(PREFERENCE)))
}

pub fn row() -> Result<Option<Row>, Never> {
  let Ok(preference) = path();
  let at = match preference {
    Some(at) => at,
    None => return Ok(None),
  };
  let Ok(stored) = console_core_atomic_writes::read(&at);
  let placement = match Placement::read(stored) {
    Ok(current) => current,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: reading {PREFERENCE}: {fault}");
      Placement::Current
    }
  };
  let Ok(state) = placement.switched();
  let Ok(task) = toggle_task();
  let applying = task.command.clone();
  let Ok(change) = Handler::and_stay(move |showing| showing.run(task.clone()));
  let Ok(row) = Row::new("Open new windows on a new workspace", Aside(""), change);
  let Ok(switch) = row.switched(state);
  let Ok(with_progress) = switch.while_running(&applying, "Applying window placement");

  Ok(Some(with_progress))
}

fn save_toggled(at: &Path) -> std::io::Result<()> {
  let Ok(stored) = console_core_atomic_writes::read(at);
  let current = Placement::read(stored)?;
  let Ok(next) = current.toggled_value();

  console_core_atomic_writes::whole_with_folders(at, next.as_bytes())
    .map_err(std::io::Error::other)
}

pub fn toggle_window_placement() -> Result<ExitCode, Never> {
  let Ok(preference) = path();
  let at = match preference {
    Some(at) => at,
    None => {
      console_core_logging::warn!(
        "settings-panel: no configuration directory for window placement"
      );
      return Ok(ExitCode::FAILURE);
    }
  };

  match save_toggled(&at) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("settings-panel: could not save window placement: {fault}");
      return Ok(ExitCode::FAILURE);
    }
  }

  let Ok(outcome) = console_compositor::reload_configuration();

  Ok(match outcome {
    DispatchResult::Success => ExitCode::SUCCESS,
    DispatchResult::Failure(fault) => {
      console_core_logging::warn!("Window placement saved. Reload Console to apply it. {fault}");
      ExitCode::FAILURE
    }
  })
}

fn toggle_task() -> Result<Task, Never> {
  let Ok(command) = Command::internal(InternalProgram::SettingsPanel, &[TOGGLE_WINDOW_PLACEMENT]);
  let Ok(task) = Task::of(command);

  let Ok(attributed) = task.from_application(console_notifications::Application("Settings"));
  attributed.then(|answer| {
    Ok(match answer.status {
      ExitStatus::Success | ExitStatus::Interrupt => None,
      ExitStatus::Failure(_) => Some(Notification {
        summary: "Could not apply window placement".to_owned(),
        body: Some(answer.error_output.clone()),
      }),
    })
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn window_placement_defaults_to_current_and_toggles_existing_preferences()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("settings-window-placement")?;
    let at = folder.join(PREFERENCE);
    let Ok(stored) = console_core_atomic_writes::read(&at);
    let placement = Placement::read(stored)?;
    let Ok(default) = placement.switched();
    assert_eq!(default, Switch::Off);

    save_toggled(&at)?;
    let first = console_core_atomic_writes::read_text(&at)?;
    assert_eq!(first, "new");

    save_toggled(&at)?;
    let second = console_core_atomic_writes::read_text(&at)?;
    assert_eq!(second, "current");

    Ok(())
  }
}
