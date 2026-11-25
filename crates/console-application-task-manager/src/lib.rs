//! List open windows and keep a window selected while its actions are shown.
//! The card owns the selection actor. A confirmation captures a window's
//! identity, and dispatches only while that window is still selected.

use std::collections::BTreeMap;
use std::path::Path;

use console_actor::{Actor, Closed, SendError};
use console_applications::entry::{self, DesktopEntry, Worth};
use console_compositor::events::{self, CompositorEvent};
use console_compositor::{Clients, DispatchResult, Window, WindowAction};
use console_core_arguments::{Command, Operands};
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_events::again::Worth as Refresh;
use console_panel::card::{Card, Door};
use console_panel::page::{
  ActionOutcome, Aside, Handler, Page, Picture, Row, Rows, Showing, Subject,
};
use console_program_contract::EventGroup;

pub const WHO: &str = "console-task-manager";

pub const COMMAND: Command = Command {
  name: env!("CARGO_PKG_NAME"),
  about: "open, close, or force quit a desktop window",
  flags: &[],
  operands: Operands::None,
};

const TITLE: &str = "Task Manager";

#[derive(Clone)]
struct Task {
  window: Window,
  name: String,
  detail: String,
}

fn add_name(path: &Path, names: &mut BTreeMap<String, String>) -> Result<(), Never> {
  let said = match std::fs::read_to_string(path) {
    Ok(said) => said,
    Err(_entry_unavailable) => return Ok(()),
  };
  let Ok(entry) = DesktopEntry::read(&said);
  let Ok(worth) = entry.worth();
  let Ok(says) = entry.says();

  let name = match says.filter(|_| worth == Worth::Drawing) {
    Some(name) => name,
    None => return Ok(()),
  };
  let desktop_id = path.file_stem().and_then(|stem| stem.to_str());

  for key in [
    entry.startup_wm_class,
    desktop_id,
    desktop_id.and_then(|id| id.strip_prefix("console-")),
  ]
  .into_iter()
  .flatten()
  {
    names
      .entry(key.to_ascii_lowercase())
      .or_insert_with(|| name.to_string());
  }

  Ok(())
}

fn names() -> Result<BTreeMap<String, String>, Never> {
  let mut names = BTreeMap::new();
  let Ok(roots) = console_core_places::applications();
  let Ok(paths) = entry::files(&roots);

  for path in paths {
    let Ok(()) = add_name(&path, &mut names);
  }

  Ok(names)
}

fn task(window: Window, names: &BTreeMap<String, String>) -> Result<Task, Never> {
  let class = window.first_class.trim();
  let title = window.title.trim();
  let name = match names.get(&class.to_ascii_lowercase()) {
    Some(name) => name.clone(),
    None => match class {
      "" => "Window".to_string(),
      named => named.to_string(),
    },
  };
  let place = window.workspace_named.trim_start_matches("special:");
  let detail = match (title.is_empty() || title == name, place.is_empty()) {
    (false, false) => format!("Workspace {place} · {title}"),
    (true, false) => format!("Workspace {place}"),
    (false, true) => title.to_string(),
    (true, true) => String::new(),
  };

  Ok(Task {
    window,
    name,
    detail,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowPresence {
  Open,
  Closed,
}

impl From<bool> for WindowPresence {
  fn from(open: bool) -> Self {
    match bool::then_some(open, ()) {
      Some(()) => Self::Open,
      None => Self::Closed,
    }
  }
}

fn presence(target: &Window, open: &[Window]) -> Result<WindowPresence, Never> {
  Ok(WindowPresence::from(open.iter().any(|window| {
    target.address == window.address && target.pid == window.pid
  })))
}

enum WindowActionOutcome {
  Completed,
  Failed,
}

fn act(action: WindowAction, target: &Task) -> Result<WindowActionOutcome, Never> {
  let open = match console_compositor::ask(Clients) {
    Ok(open) => open,
    Err(_cannot_list_windows) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Task Manager"),
        "Couldn’t read open windows",
        None,
      );
      return Ok(WindowActionOutcome::Failed);
    }
  };
  let Ok(present) = presence(&target.window, &open);

  match present {
    WindowPresence::Closed => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Task Manager"),
        "That window has closed",
        None,
      );
      return Ok(WindowActionOutcome::Failed);
    }
    WindowPresence::Open => {}
  }

  let Ok(outcome) = action.dispatch(&target.window.address);

  Ok(match outcome {
    DispatchResult::Success => WindowActionOutcome::Completed,
    DispatchResult::Failure(_could_not_dispatch) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Task Manager"),
        "The compositor couldn’t complete that action",
        None,
      );
      WindowActionOutcome::Failed
    }
  })
}

struct WindowSelection;

enum SelectionRequest {
  Choose(Task),
  Back,
}

impl Machine for WindowSelection {
  type Input = ();
  type State = Option<Task>;
  type Request = SelectionRequest;
  type Effect = ActionOutcome;

  fn initialize(
    _input: &Self::Input,
    previous: Option<Self::State>,
    _effects: &mut Queue<Self::Effect>,
  ) -> Result<Self::State, Never> {
    Ok(match previous {
      Some(selected) => selected,
      None => None,
    })
  }

  fn handle(
    selected: Self::State,
    request: Self::Request,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<Self::State, Never> {
    match request {
      SelectionRequest::Choose(task) => Ok(Some(task)),
      SelectionRequest::Back => {
        let action = match selected {
          Some(_return_to_windows) => ActionOutcome::Stay,
          None => ActionOutcome::Close,
        };
        let Ok(()) = effects.offer(action);

        Ok(None)
      }
    }
  }
}

type Selection = Actor<WindowSelection>;

fn placeholder(message: &str) -> Result<Vec<Row>, Never> {
  let Ok(row) = Row::placeholder(message);

  Ok(vec![row])
}

fn task_rows(names: &BTreeMap<String, String>, selected: &Selection) -> Result<Vec<Row>, Never> {
  let open = match console_compositor::ask(Clients) {
    Ok(open) => open,
    Err(_cannot_list_windows) => return placeholder("Couldn’t read open windows"),
  };
  let mut tasks = Vec::with_capacity(open.len());

  for window in open {
    let Ok(task) = task(window, names);
    tasks.push(task);
  }

  tasks.sort_by(|left, right| {
    left
      .window
      .workspace
      .cmp(&right.window.workspace)
      .then_with(|| left.name.cmp(&right.name))
      .then_with(|| left.window.title.cmp(&right.window.title))
      .then_with(|| left.window.address.cmp(&right.window.address))
  });

  match tasks.as_slice() {
    [] => return placeholder("No open windows"),
    [_, ..] => {}
  }

  let mut rows = Vec::with_capacity(tasks.len());

  for task in tasks {
    let selection = selected.clone();
    let Ok(row) = Row::stacked(Picture::None, &task.name, Aside(&task.detail));
    let Ok(handler) = Handler::and_stay(move |showing| {
      match selection.send(SelectionRequest::Choose(task.clone())) {
        Ok(_chosen) => showing.replace(2),
        Err(SendError::Closed | SendError::MachineDefect(_)) => {
          let Ok(()) = console_notifications::notify(
            console_notifications::Application("Task Manager"),
            "Couldn’t select that window",
            None,
          );
        }
      }
    });
    let Ok(choosing) = row.choosing(handler);
    let Ok(opening) = choosing.opening();

    rows.push(opening);
  }

  Ok(rows)
}

fn back(selected: &Selection, showing: &dyn Showing) -> Result<ActionOutcome, Never> {
  let outcome = match selected.send(SelectionRequest::Back) {
    Ok(effects) => match effects.as_slice() {
      [outcome] => *outcome,
      [] | [_, _, ..] => ActionOutcome::Close,
    },
    Err(SendError::Closed | SendError::MachineDefect(_)) => ActionOutcome::Close,
  };

  match outcome {
    ActionOutcome::Stay => showing.replace(0),
    ActionOutcome::Close => {}
  }

  Ok(outcome)
}

fn return_to_windows_after(
  action: WindowAction,
  task: &Task,
  selected: &Selection,
  showing: &dyn Showing,
) -> Result<(), Never> {
  let Ok(outcome) = act(action, task);

  match outcome {
    WindowActionOutcome::Completed => {
      let Ok(_returned) = back(selected, showing);
    }
    WindowActionOutcome::Failed => {}
  }

  Ok(())
}

fn confirmed_target(target: &Task, selected: &Selection) -> Result<WindowPresence, Never> {
  match selected.get() {
    Ok(Some(current)) => presence(&target.window, std::slice::from_ref(&current.window)),
    Ok(None) | Err(Closed) => Ok(WindowPresence::Closed),
  }
}

fn force_quit_row(task: &Task, selected: &Selection) -> Result<Row, Never> {
  let force = task.clone();
  let selection = selected.clone();
  let Ok(force_quit) = Handler::and_stay(move |showing| {
    let target = force.clone();
    let confirmation = selection.clone();
    let label = format!("{} · {}", force.name, force.detail);

    showing.sure(
      "Force quit this app? Its open windows will close.",
      Subject(&label),
      &["Force quit"],
      Box::new(move |confirmed_showing, _force_quit| {
        let Ok(current) = confirmed_target(&target, &confirmation);

        match current {
          WindowPresence::Open => {
            let Ok(()) = return_to_windows_after(
              WindowAction::ForceQuit,
              &target,
              &confirmation,
              confirmed_showing,
            );
          }
          WindowPresence::Closed => {}
        }
      }),
    );
  });

  Row::new("Force quit…", Aside(""), force_quit)
}

fn action_rows(task: &Task, selected: &Selection) -> Result<Vec<Row>, Never> {
  let go = task.clone();
  let close = task.clone();
  let back_selection = selected.clone();
  let close_selection = selected.clone();

  let Ok(back_row) = Row::back("Open windows", move |showing| {
    let Ok(_returned) = back(&back_selection, showing);
  });
  let Ok(task_row) = Row::stacked(Picture::None, &task.name, Aside(&task.detail));
  let Ok(focus) = Handler::call(move |_| {
    let Ok(outcome) = act(WindowAction::Focus, &go);

    match outcome {
      WindowActionOutcome::Completed => ActionOutcome::Close,
      WindowActionOutcome::Failed => ActionOutcome::Stay,
    }
  });
  let Ok(go_row) = Row::new("Go to window", Aside(""), focus);
  let Ok(close_window) = Handler::and_stay(move |showing| {
    let Ok(()) = return_to_windows_after(WindowAction::Close, &close, &close_selection, showing);
  });
  let Ok(close_row) = Row::new("Close window", Aside(""), close_window);

  let mut rows = vec![back_row, task_row, go_row, close_row];

  match bool::then_some(task.window.pid > 0, ()) {
    Some(()) => {
      let Ok(force_row) = force_quit_row(task, selected);
      rows.push(force_row);
    }
    None => {}
  }

  Ok(rows)
}

fn window_changed(line: &str) -> Result<Refresh, Never> {
  let Ok(event) = events::read(line);

  Ok(match event {
    CompositorEvent::WindowOpened(_)
    | CompositorEvent::WindowClosed(_)
    | CompositorEvent::WindowRenamed(_)
    | CompositorEvent::WindowMoved => Refresh::Querying,
    CompositorEvent::WindowFloated
    | CompositorEvent::WindowPinned
    | CompositorEvent::WindowFilled
    | CompositorEvent::WindowFocused
    | CompositorEvent::LayerOpened
    | CompositorEvent::LayerClosed
    | CompositorEvent::WorkspaceChanged
    | CompositorEvent::ScreenFocused
    | CompositorEvent::ConfigurationReloaded
    | CompositorEvent::Ignored => Refresh::Ignoring,
  })
}

fn current_rows(
  names: &BTreeMap<String, String>,
  selected: &Selection,
) -> Result<Vec<Row>, Never> {
  match selected.get() {
    Ok(Some(task)) => action_rows(&task, selected),
    Ok(None) => task_rows(names, selected),
    Err(Closed) => placeholder("Couldn’t read the current selection"),
  }
}

pub fn door(_asked: &[String]) -> Result<Door, Never> {
  Door::closing(WHO)
}

pub fn card(_asked: &[String]) -> Result<Card, Never> {
  Card::supervised::<WindowSelection, _>((), |selected| {
    let Ok(names) = names();
    let browsing = selected.clone();
    let backing = selected.clone();
    let Ok(rows) = Rows::computed(move || {
      let Ok(current) = current_rows(&names, &browsing);

      current
    });
    let Ok(page) = Page::new(TITLE, rows);
    let Ok(watching) = page.with_subscription(EventGroup::Compositor, window_changed);
    let Ok(ready) = watching.on_back(move |showing| {
      let Ok(outcome) = back(&backing, showing);

      outcome
    });

    Ok(vec![ready])
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn two_windows() -> Result<Vec<Window>, console_compositor::HyprctlError> {
    let answer = console_compositor::read_value(
      r#"[
      {"address":"0x100","title":"Draft","initialClass":"org.example.Writer","pid":10,
       "workspace":{"id":2,"name":"2"}},
      {"address":"0x200","title":"Review","initialClass":"org.example.Writer","pid":10,
       "workspace":{"id":3,"name":"3"}}
    ]"#,
    )?;
    let Ok(open) = console_compositor::windows_open(&answer);

    Ok(open)
  }

  #[test]
  fn two_windows_of_one_app_keep_their_own_label_and_target()
  -> Result<(), Box<dyn std::error::Error>> {
    let open = two_windows()?;
    let first_window = open.first().ok_or("the first window should be open")?;
    let second_window = open.get(1).ok_or("the second window should be open")?;
    let names = BTreeMap::from([("org.example.writer".to_string(), "Writer".to_string())]);
    let Ok(first) = task(first_window.clone(), &names);
    let Ok(second) = task(second_window.clone(), &names);

    assert_eq!(first.name, second.name);
    assert_eq!(first.detail, "Workspace 2 · Draft");
    assert_eq!(second.detail, "Workspace 3 · Review");
    assert_eq!(presence(&second.window, &open), Ok(WindowPresence::Open));
    assert_eq!(
      presence(&second.window, std::slice::from_ref(first_window)),
      Ok(WindowPresence::Closed)
    );

    let Ok(focus) = WindowAction::Focus.dispatcher(&second.window.address);
    let Ok(force_quit) = WindowAction::ForceQuit.dispatcher(&second.window.address);

    assert_eq!(focus, r#"hl.dsp.focus({ window = "address:0x200" })"#);
    assert_eq!(
      force_quit,
      r#"hl.dsp.window.kill({ window = "address:0x200" })"#
    );

    Ok(())
  }

  #[test]
  fn confirmation_requires_the_same_selection_and_a_living_card()
  -> Result<(), Box<dyn std::error::Error>> {
    let open = two_windows()?;
    let first = open.first().ok_or("the first window should be open")?;
    let second = open.get(1).ok_or("the second window should be open")?;
    let Ok(target) = task(first.clone(), &BTreeMap::new());
    let Ok(another) = task(second.clone(), &BTreeMap::new());
    let console_actor::Booted { actor, scope, .. } = console_actor::boot::<WindowSelection>(())?;

    let _chosen = actor.send(SelectionRequest::Choose(target.clone()))?;
    assert_eq!(confirmed_target(&target, &actor), Ok(WindowPresence::Open));
    assert_eq!(confirmed_target(&target, &actor), Ok(WindowPresence::Open));

    let _another_chosen = actor.send(SelectionRequest::Choose(another))?;
    assert_eq!(
      confirmed_target(&target, &actor),
      Ok(WindowPresence::Closed)
    );

    let returned = actor.send(SelectionRequest::Back)?;
    assert_eq!(returned, [ActionOutcome::Stay]);
    assert_eq!(
      confirmed_target(&target, &actor),
      Ok(WindowPresence::Closed)
    );

    let _chosen_again = actor.send(SelectionRequest::Choose(target.clone()))?;
    let Ok(()) = scope.close();
    assert_eq!(
      confirmed_target(&target, &actor),
      Ok(WindowPresence::Closed)
    );

    Ok(())
  }
}
