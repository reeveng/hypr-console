use std::path::Path;

use console_compositor::DispatchResult;
use console_compositor::quick_workspace::{self, Applications};
use console_core_controls::Switch;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_default_applications::editors;
use console_panel::page::{Aside, Handler, Row};

use crate::rows::Chosen;

pub const TITLE: &str = "Quick Workspace";

pub fn rows(back: impl Chosen) -> Result<Vec<Row>, Never> {
  let Ok(configuration) = crate::rows::configuration();
  let Ok(way_back) = Row::back(&configuration, move |showing| back(showing));
  let Ok(heading) = Row::naming(TITLE, Aside(""));
  let Ok(buttons) = InternalProgram::MappingPanel.path();
  let Ok(shortcuts) = Row::new(
    "Configure Shortcuts",
    Aside("Buttons"),
    Handler::Run(vec![buttons.to_owned()]),
  );
  let mut rows = vec![way_back, heading, shortcuts];
  let Ok(path) = quick_workspace::path();
  let path_3 = match path {
    Some(path_2) => path_2,
    None => {
      let Ok(message) = Row::text("Quick Workspace settings unavailable", Aside(""));
      rows.push(message);
      return Ok(rows);
    }
  };
  let selected = match quick_workspace::load(&path_3) {
    Ok(selected) => selected,
    Err(fault) => {
      let Ok(message) = Row::text(
        "Could not read Quick Workspace settings",
        Aside(&fault.to_string()),
      );
      rows.push(message);
      return Ok(rows);
    }
  };
  let Ok(applications) = editors::installed();

  for app in applications {
    let Ok(classes) = app.window_classes();

    match {
      ::core::primitive::bool::then_some(
        classes.is_empty()
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
          ),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    let at = path_3.clone();
    let state = match ::core::primitive::bool::then_some(selected.contains_key(&app.id), ()) {
      ::core::option::Option::Some(()) => Switch::On,
      ::core::option::Option::None => Switch::Off,
    };
    let name = app.says.clone();
    let Ok(handler) = Handler::and_stay(move |showing| {
      let Ok(classes_2) = app.window_classes();

      match toggle_at(&at, &app.id, classes_2) {
        Ok(()) => {
          let Ok(outcome) = quick_workspace::reload();
          showing.refresh();

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
    });
    let Ok(row) = Row::new(&name, Aside(""), handler);
    let Ok(row_2) = row.switched(state);
    rows.push(row_2);
  }

  Ok(rows)
}

fn toggle_at(at: &Path, id: &str, classes: &[String]) -> std::io::Result<()> {
  let mut selected = quick_workspace::load(at)?;

  match { ::core::primitive::bool::then_some(selected.remove(id).is_none(), ()) } {
    ::core::option::Option::Some(()) => {
      selected.insert(id.to_owned(), classes.to_vec());
    }
    ::core::option::Option::None => {}
  }

  let ::core::result::Result::Ok(saved) = serialize(&selected);
  console_core_atomic_writes::whole_with_folders(at, saved.as_bytes())
    .map_err(std::io::Error::other)
}

#[must_use]
fn serialize(
  selected: &Applications,
) -> ::core::result::Result<String, ::console_core_never::Never> {
  let token = |word: &str| !word.is_empty() && !word.chars().any(char::is_control);
  let mut saved = String::new();

  for (id, classes) in selected {
    match { ::core::primitive::bool::then_some(token(id), ()) } {
      ::core::option::Option::Some(()) => {
        saved.push_str(id);

        for class in classes.iter().filter(|class| token(class)) {
          saved.push('\t');
          saved.push_str(class);
        }

        saved.push('\n');
      }
      ::core::option::Option::None => {}
    }
  }

  ::core::result::Result::Ok(saved)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn apps_are_opt_in_and_one_switch_preserves_the_others()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("settings-quick-workspace")?;
    let at = folder.join("quick-workspace");
    let music = quick_workspace::MUSIC;
    let obs = "com.obsproject.Studio.desktop";
    let music_classes = ["music".to_owned()];
    let obs_classes = ["com.obsproject.Studio".to_owned(), "obs".to_owned()];

    let load = quick_workspace::load(&at)?;
    assert!(load.is_empty());
    toggle_at(&at, music, &music_classes)?;
    toggle_at(&at, obs, &obs_classes)?;
    toggle_at(&at, music, &music_classes)?;

    let saved = quick_workspace::load(&at)?;
    assert_eq!(
      saved,
      Applications::from([(obs.to_owned(), obs_classes.to_vec())])
    );
    Ok(())
  }
}
