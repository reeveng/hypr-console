use std::path::Path;

use console_compositor::DispatchResult;
use console_compositor::quick_workspace::{
  self, AppSelection, Applications, Assignment, Workspaces,
};
use console_core_controls::Switch;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_default_applications::editors;
use console_panel::page::{Aside, Handler, Row};

use crate::choosing::Deeper;
use crate::rows::Chosen;

pub const TITLE: &str = "Quick Workspaces";

pub fn rows(
  back: impl Chosen,
  open: impl Fn(Deeper) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(configuration) = crate::rows::configuration();
  let Ok(way_back) = Row::back(&configuration, move |showing| back(showing));
  let Ok(heading) = Row::naming(TITLE, Aside(""));
  let Ok(buttons) = InternalProgram::MappingPanel.path();
  let Ok(shortcuts) = Row::new(
    "Create Workspaces and Set Shortcuts",
    Aside("Buttons"),
    Handler::Run(vec![buttons.to_owned()]),
  );
  let mut rows = vec![way_back, heading, shortcuts];
  let Ok(registered) = console_input_mapping::table::special_workspaces();
  let saved = load();
  let workspaces = match saved {
    Ok(workspaces) => workspaces,
    Err(fault) => {
      let Ok(message) = read_error(&fault);
      rows.push(message);
      return Ok(rows);
    }
  };
  let names = registered
    .into_iter()
    .chain(
      workspaces
        .keys()
        .filter(|name| name.as_str() != quick_workspace::NAME)
        .cloned(),
    )
    .collect::<std::collections::BTreeSet<_>>();

  for name in std::iter::once(quick_workspace::NAME.to_owned()).chain(names) {
    let Ok(at) = console_core_number_conversion::fitted(rows.len());
    let Ok(label) = workspace_label(&name);
    let Ok(count) = console_core_number_conversion::fitted::<_, u32>(
      workspaces.get(&name).map_or(0, Applications::len),
    );
    let Ok(handler) = open(Deeper {
      name: name.clone(),
      at,
    });
    let Ok(row) = Row::new(label, Aside(&count.to_string()), handler);
    let Ok(link) = row.opening();
    rows.push(link);
  }

  Ok(rows)
}

pub fn app_rows(
  workspace: &str,
  back: impl Chosen,
  select: impl Fn(Assignment) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
  let Ok(configuration) = crate::rows::configuration();
  let Ok(way_back) = Row::back(&configuration, move |showing| back(showing));
  let Ok(label) = workspace_label(workspace);
  let Ok(heading) = Row::naming(label, Aside("Apps"));
  let mut rows = vec![way_back, heading];
  let saved = load();
  let workspaces = match saved {
    Ok(workspaces) => workspaces,
    Err(fault) => {
      let Ok(message) = read_error(&fault);
      rows.push(message);
      return Ok(rows);
    }
  };
  let mut unlisted = match workspaces.get(workspace) {
    Some(apps) => apps.clone(),
    None => Applications::new(),
  };
  let Ok(applications) = editors::installed();

  for app in applications {
    let Ok(classes) = app.window_classes();

    match (classes.is_empty()
      || matches!(
        app.id.as_str(),
        "console-buttons.desktop"
          | "console-clipboard.desktop"
          | "console-forecast.desktop"
          | "console-input-dictation.desktop"
          | "console-notifications.desktop"
          | "console-syncthing.desktop"
          | "console-task-manager.desktop"
          | "console-update.desktop"
      ))
    .then_some(())
    {
      Some(()) => continue,
      None => {}
    }

    let selection = match unlisted.remove(&app.id) {
      Some(_) => AppSelection::Remove,
      None => AppSelection::Include,
    };
    let elsewhere = workspaces.iter().find_map(|(name, apps)| {
      (name != workspace && apps.contains_key(&app.id)).then_some(name.as_str())
    });
    let aside = match elsewhere {
      Some(name) => {
        let Ok(assigned) = workspace_label(name);
        assigned
      }
      None => "",
    };
    let assignment = Assignment {
      workspace: workspace.to_owned(),
      id: app.id.clone(),
      classes: classes.to_vec(),
      selection,
    };
    let Ok(row) = app_row(&app.says, Aside(aside), assignment, &select);
    rows.push(row);
  }

  for (id, classes) in unlisted {
    let assignment = Assignment {
      workspace: workspace.to_owned(),
      id: id.clone(),
      classes,
      selection: AppSelection::Remove,
    };
    let Ok(row) = app_row(&id, Aside("Unavailable"), assignment, &select);
    rows.push(row);
  }

  Ok(rows)
}

fn app_row(
  label: &str,
  aside: Aside<'_>,
  assignment: Assignment,
  select: impl Fn(Assignment) -> Result<Handler, Never>,
) -> Result<Row, Never> {
  let state = match assignment.selection {
    AppSelection::Remove => Switch::On,
    AppSelection::Include => Switch::Off,
  };
  let Ok(handler) = select(assignment);
  let Ok(row) = Row::new(label, aside, handler);

  row.switched(state)
}

fn workspace_label(workspace: &str) -> Result<&str, Never> {
  Ok(match workspace {
    quick_workspace::NAME => "Quick Workspace",
    name => name,
  })
}

fn load() -> std::io::Result<Workspaces> {
  let Ok(at) = quick_workspace::path();
  let file = at.ok_or_else(|| std::io::Error::other("No configuration directory"))?;

  quick_workspace::load_workspaces(&file)
}

fn read_error(fault: &std::io::Error) -> Result<Row, Never> {
  Row::text(
    "Could not read Quick Workspace settings",
    Aside(&fault.to_string()),
  )
}

pub fn save(assignment: &Assignment) -> Result<(), Never> {
  let Ok(at) = quick_workspace::path();
  let changed = at
    .as_deref()
    .ok_or_else(|| std::io::Error::other("No configuration directory"))
    .and_then(|file: &Path| quick_workspace::save_assignment(file, assignment));

  match changed {
    Ok(()) => {
      let Ok(outcome) = quick_workspace::reload();

      match outcome {
        DispatchResult::Success => {}
        DispatchResult::Failure(fault) => {
          let Ok(()) = console_notifications::notify(
            console_notifications::Application("Settings"),
            "Quick Workspace settings saved",
            Some(&format!("Reload failed. {fault}")),
          );
        }
      }
    }
    Err(fault) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        "Could not save Quick Workspace settings",
        Some(&fault.to_string()),
      );
    }
  }

  Ok(())
}
