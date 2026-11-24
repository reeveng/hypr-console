//! Configured operations and sequences encoded by the compositor backend.

use console_core_never::Never;

use crate::lua::{self, Exec};
use crate::{
  Action, BindingFlag, Carrying, Direction, DispatchResult, LaunchRules, Request, WindowAction,
  WindowChange, onto, request,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation<'a> {
  Window {
    address: &'a str,
    action: WindowAction,
  },
  ChangeWindow {
    address: &'a str,
    change: WindowChange<'a>,
  },
  FocusWorkspace(&'a str),
  FocusMonitor(&'a str),
  Layout(&'a str),
  MoveWorkspaceToMonitor {
    workspace: &'a str,
    monitor: i64,
  },
  MoveWorkspaceToOutput {
    workspace: &'a str,
    output: &'a str,
  },
  Preselect(Direction),
}

impl Operation<'_> {
  fn dispatcher(self) -> Result<String, Never> {
    match self {
      Self::Window { address, action } => action.dispatcher(address),
      Self::ChangeWindow { address, change } => change.dispatcher(address),
      Self::FocusWorkspace(name) => onto(name, Carrying::None),
      Self::Layout(message) => lua::layout(message),
      Self::FocusMonitor(monitor) => {
        let Ok(monitor_2) = lua::quote(monitor);

        Ok(format!(
          "hl.dsp.focus({{ monitor = {monitor} }})",
          monitor = monitor_2
        ))
      }
      Self::MoveWorkspaceToMonitor { workspace, monitor } => {
        lua::move_workspace_to_monitor(lua::WorkspaceMove {
          workspace,
          monitor: &monitor.to_string(),
        })
      }
      Self::MoveWorkspaceToOutput { workspace, output } => {
        lua::move_workspace_to_monitor(lua::WorkspaceMove {
          workspace,
          monitor: output,
        })
      }
      Self::Preselect(side) => lua::layout(match side {
        Direction::Left => "preselect l",
        Direction::Right => "preselect r",
        Direction::Up => "preselect u",
        Direction::Down => "preselect d",
      }),
    }
  }

  pub fn apply(self) -> Result<DispatchResult, Never> {
    let Ok(script) = Script::operations(&[self]);

    script.run()
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script(String);

impl Script {
  pub fn operations(operations: &[Operation<'_>]) -> Result<Self, Never> {
    let statements: Vec<_> = operations
      .iter()
      .map(|operation| {
        let Ok(dispatcher) = operation.dispatcher();
        let Ok(statement) = lua::dispatch(&dispatcher);

        statement
      })
      .collect();

    Ok(Self(statements.join(" ")))
  }

  pub(crate) fn from_lua(lua: String) -> Result<Self, Never> {
    Ok(Self(lua))
  }

  pub fn run(&self) -> Result<DispatchResult, Never> {
    request(Request::Script, &self.0)
  }
}

impl std::fmt::Display for Script {
  fn fmt(&self, format: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    format.write_str(&self.0)
  }
}

pub fn launch(command: &str, rules: &LaunchRules) -> Result<DispatchResult, Never> {
  let Ok(script) = lua::exec(command, Exec::WithRules(rules));

  request(Request::Script, &script)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingAction<'a> {
  Run(&'a str),
  Dispatch(Action),
}

pub struct Binding<'a> {
  pub keys: &'a str,
  pub description: &'a str,
  pub action: BindingAction<'a>,
  pub flags: &'a [BindingFlag],
}

impl Binding<'_> {
  pub fn script(self) -> Result<Script, Never> {
    let Ok(dispatcher) = match self.action {
      BindingAction::Run(command) => lua::exec(command, Exec::Dispatcher),
      BindingAction::Dispatch(action) => action.dispatcher(),
    };
    let Ok(lua) = lua::bind(lua::Binding {
      keys: self.keys,
      dispatcher: &dispatcher,
      description: self.description,
      flags: self.flags,
    });

    Script::from_lua(lua)
  }
}

pub fn unbind(keys: &str) -> Result<Script, Never> {
  let Ok(lua) = lua::unbind(keys);

  Script::from_lua(lua)
}
