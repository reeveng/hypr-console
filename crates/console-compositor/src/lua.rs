//! Lua values and commands sent to Hyprland.

use console_core_never::Never;
mod monitor;

pub(crate) use monitor::touch_transform;
pub use monitor::{MonitorRule, ScreenDeclaration, Undeclared, disable_monitor};

const EXEC: &str = "exec_cmd";

pub fn quote(text: &str) -> Result<String, Never> {
  let mut quoted = String::with_capacity(text.len().saturating_add(2));
  quoted.push('"');

  for character in text.chars() {
    match character {
      '"' => quoted.push_str("\\\""),
      '\\' => quoted.push_str("\\\\"),
      '\n' => quoted.push_str("\\n"),
      '\r' => quoted.push_str("\\r"),
      '\t' => quoted.push_str("\\t"),
      plain => match (u32::from(plain) < 0x20).then_some(()) {
        Some(()) => quoted.push_str(&format!("\\{:03}", u32::from(plain))),
        None => quoted.push(plain),
      },
    }
  }

  quoted.push('"');

  Ok(quoted)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exec<'a> {
  Dispatcher,
  WithRules(&'a LaunchRules),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchRules(String);

impl LaunchRules {
  pub fn with_tag(mut self, tag: &str) -> Result<Self, Never> {
    let Ok(value) = quote(&format!("+{tag}"));

    match { ::core::primitive::bool::then_some(!self.0.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        self.0.push_str(", ");
      }
      ::core::option::Option::None => {}
    }

    self.0.push_str(&format!(r#"["tag"] = {value}"#));

    Ok(self)
  }

  pub fn from_legacy(prefix: &str) -> Result<Self, Never> {
    let entries = prefix
      .split(';')
      .map(str::trim)
      .filter(|rule| !rule.is_empty());
    let rendered: Vec<String> = entries
      .map(|rule| {
        let (key, value) = match rule.split_once(' ') {
          Some((key, value)) => {
            let Ok(quoted_value) = quote(value);

            (key, quoted_value)
          }
          None => (rule, "true".to_owned()),
        };
        let name = match key {
          "fullscreenstate" => "fullscreen_state",
          "noinitialfocus" => "no_initial_focus",
          "keepaspectratio" => "keep_aspect_ratio",
          "maxsize" => "max_size",
          "minsize" => "min_size",
          name => name,
        };
        let Ok(quoted_name) = quote(name);

        format!("[{quoted_name}] = {value}")
      })
      .collect();

    Ok(Self(rendered.join(", ")))
  }
}

pub fn exec(command: &str, form: Exec<'_>) -> Result<String, Never> {
  let Ok(quoted) = quote(command);

  Ok(match form {
    Exec::Dispatcher => format!("hl.dsp.{EXEC}({quoted})"),
    Exec::WithRules(LaunchRules(rules)) => format!("hl.{EXEC}({quoted}, {{{rules}}})"),
  })
}

pub(crate) fn dispatch(dispatcher: &str) -> Result<String, Never> {
  Ok(format!("hl.dispatch({dispatcher})"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingFlag {
  Locked,
  Repeating,
}

pub(crate) struct Binding<'a> {
  pub keys: &'a str,
  pub dispatcher: &'a str,
  pub description: &'a str,
  pub flags: &'a [BindingFlag],
}

pub(crate) fn bind(
  Binding {
    keys,
    dispatcher,
    description,
    flags,
  }: Binding<'_>,
) -> Result<String, Never> {
  let Ok(keys_2) = quote(keys);
  let Ok(about) = quote(description);
  let mut options = vec![format!("description = {about}")];

  options.extend(flags.iter().map(|flag| match flag {
    BindingFlag::Locked => "locked = true".to_owned(),
    BindingFlag::Repeating => "repeating = true".to_owned(),
  }));

  Ok(format!(
    "hl.bind({keys}, {dispatcher}, {{ {} }})",
    options.join(", "),
    keys = keys_2
  ))
}

pub(crate) fn unbind(keys: &str) -> Result<String, Never> {
  let Ok(keys_2) = quote(keys);

  Ok(format!("hl.unbind({keys})", keys = keys_2))
}

pub fn window(address: &str) -> Result<String, Never> {
  quote(&format!("address:{address}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coordinates {
  Absolute,
  Relative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullscreenMode {
  Maximized,
  Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowChange<'a> {
  MoveToWorkspace(&'a str),
  MoveTo(i64, i64),
  Resize {
    x: i64,
    y: i64,
    coordinates: Coordinates,
  },
  ToggleFloating,
  TogglePinned,
  ToggleFullscreen(FullscreenMode),
}

impl WindowChange<'_> {
  pub(crate) fn dispatcher(self, address: &str) -> Result<String, Never> {
    let Ok(window) = window(address);

    Ok(match self {
      Self::MoveToWorkspace(workspace) => {
        let Ok(workspace_2) = quote(workspace);

        format!(
          "hl.dsp.window.move({{ window = {window}, workspace = {workspace}, follow = false }})",
          workspace = workspace_2
        )
      }
      Self::MoveTo(x, y) => {
        format!("hl.dsp.window.move({{ window = {window}, x = {x}, y = {y}, relative = false }})")
      }
      Self::Resize { x, y, coordinates } => {
        let relative = coordinates == Coordinates::Relative;

        format!(
          "hl.dsp.window.resize({{ window = {window}, x = {x}, y = {y}, relative = {relative} }})"
        )
      }
      Self::ToggleFloating => {
        format!("hl.dsp.window.float({{ window = {window}, action = \"toggle\" }})")
      }
      Self::TogglePinned => {
        format!("hl.dsp.window.pin({{ window = {window}, action = \"toggle\" }})")
      }
      Self::ToggleFullscreen(mode) => {
        let mode_2 = match mode {
          FullscreenMode::Maximized => "maximized",
          FullscreenMode::Fullscreen => "fullscreen",
        };

        format!(
          "hl.dsp.window.fullscreen({{ window = {window}, mode = \"{mode}\", action = \"toggle\" }})",
          mode = mode_2
        )
      }
    })
  }
}

pub(crate) struct WorkspaceMove<'a> {
  pub workspace: &'a str,
  pub monitor: &'a str,
}

pub(crate) fn move_workspace_to_monitor(
  WorkspaceMove { workspace, monitor }: WorkspaceMove<'_>,
) -> Result<String, Never> {
  let Ok(workspace_2) = quote(workspace);
  let Ok(monitor_2) = quote(monitor);

  Ok(format!(
    "hl.dsp.workspace.move({{ workspace = {workspace}, monitor = {monitor} }})",
    workspace = workspace_2,
    monitor = monitor_2
  ))
}

pub(crate) fn layout(command: &str) -> Result<String, Never> {
  let Ok(command_2) = quote(command);

  Ok(format!("hl.dsp.layout({command})", command = command_2))
}

pub struct WindowWorkspace<'a> {
  pub name: &'a str,
  pub class: &'a str,
  pub workspace: &'a str,
}

pub fn window_workspace_rule(
  WindowWorkspace {
    name,
    class,
    workspace,
  }: WindowWorkspace<'_>,
) -> Result<String, Never> {
  let Ok(rule_name) = quote(name);
  let Ok(window_class) = quote(class);
  let Ok(target) = quote(workspace);

  Ok(format!(
    "hl.window_rule({{ name = {rule_name}, match = {{ class = {window_class} }}, workspace = {target} }})"
  ))
}

pub fn background_color(color: &str) -> Result<String, Never> {
  let Ok(color_value) = quote(color);

  Ok(format!(
    "hl.config({{ misc = {{ background_color = {color_value} }} }})"
  ))
}

pub const DISABLE_WATCHDOG_WARNING: &str =
  "hl.config({ misc = { disable_watchdog_warning = true } })";
pub(crate) const EXIT: &str = "hl.dsp.exit()";
pub(crate) const ENABLE_DISPLAY: &str = r#"hl.dsp.dpms({ action = "enable" })"#;
pub(crate) const DISABLE_DISPLAY: &str = r#"hl.dsp.dpms({ action = "disable" })"#;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn both_exec_forms_keep_shell_text_inside_one_lua_argument() {
    let command = "printf '%s\\n' \"first\"\nnext\u{7}word";
    let Ok(rules) = LaunchRules::from_legacy("float");

    assert_eq!(
      exec(command, Exec::Dispatcher),
      Ok(r#"hl.dsp.exec_cmd("printf '%s\\n' \"first\"\nnext\007word")"#.to_owned())
    );
    assert_eq!(
      exec(command, Exec::WithRules(&rules)),
      Ok(
        r#"hl.exec_cmd("printf '%s\\n' \"first\"\nnext\007word", {["float"] = true})"#.to_owned()
      )
    );
  }

  #[test]
  fn legacy_launch_rules_become_escaped_lua_table_entries() {
    let Ok(rules) =
      LaunchRules::from_legacy("monitor 0;workspace 2 silent;float;fullscreenstate 0");

    assert_eq!(
      exec("foot", Exec::WithRules(&rules)),
      Ok(r#"hl.exec_cmd("foot", {["monitor"] = "0", ["workspace"] = "2 silent", ["float"] = true, ["fullscreen_state"] = "0"})"#.to_owned())
    );
  }
}
