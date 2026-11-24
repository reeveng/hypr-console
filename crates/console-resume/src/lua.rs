//! How this program spells what it wants the compositor to do.
//!
//! Hyprland 0.56 kept the IPC socket and changed what `dispatch` means. Under a
//! Lua config it wraps the payload as `return hl.dispatch(<payload>)` and hands
//! it to the interpreter, so the old `dispatch exec foo` string form is a syntax
//! error and every dispatcher fails. Reads were untouched, which is why saving a
//! session went on looking healthy while restoring one never got off the ground.
//!
//! So everything asked for here is asked as Lua. What carries it is
//! `console-compositor`, which is where this desktop keeps hyprctl: the fork
//! this was ported from opened the socket itself and had a second copy of where
//! that socket lives, which is the fault that crate exists to end.
//!
//! This module reads launch rules from the resume file. The compositor crate
//! encodes the Lua required to launch the saved command.

use std::collections::BTreeMap;

use console_compositor::{DispatchResult, LaunchRules};
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start<'a> {
  pub rules: LaunchRules,
  pub command: &'a str,
}

pub fn exec(start: &Start<'_>) -> Result<DispatchResult, Never> {
  console_compositor::launch(start.command, &start.rules)
}

pub fn split_exec_line(line: &str) -> Result<Start<'_>, Never> {
  split_exec_line_on(line, None)
}

pub(crate) fn split_exec_line_on<'a>(
  line: &'a str,
  workspace_outputs: Option<&BTreeMap<&str, &str>>,
) -> Result<Start<'a>, Never> {
  let line_2 = line.trim();

  let inside = match line_2.strip_prefix('[') {
    Some(rest) => rest.split_once(']'),
    None => None,
  };

  match inside {
    Some((prefix, command)) => {
      let Ok(rules) = match workspace_outputs {
        None => LaunchRules::from_legacy(prefix),
        Some(workspace_outputs_2) => {
          let mut rules: Vec<_> = prefix
            .split(';')
            .map(str::trim)
            .filter(|rule| !rule.starts_with("monitor "))
            .collect();
          let monitor = rules
            .iter()
            .find_map(|rule| rule.strip_prefix("workspace "))
            .and_then(|workspace| {
              let name = match workspace.strip_suffix(" silent") {
                Some(verbose) => verbose,
                None => workspace,
              };
              workspace_outputs_2.get(name)
            })
            .map(|monitor| format!("monitor {monitor}"));
          rules.extend(monitor.as_deref());
          LaunchRules::from_legacy(&rules.join(";"))
        }
      };

      Ok(Start {
        rules,
        command: command.trim(),
      })
    }
    None => Ok(Start {
      rules: LaunchRules::default(),
      command: line_2,
    }),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_compositor::lua::{quote, window};

  #[test]
  fn quotes_what_would_otherwise_end_the_string() {
    assert_eq!(quote(r#"say "hi""#), Ok(r#""say \"hi\"""#.to_string()));
    assert_eq!(quote(r"C:\path"), Ok(r#""C:\\path""#.to_string()));
    assert_eq!(quote("two\nlines"), Ok(r#""two\nlines""#.to_string()));
  }

  #[test]
  fn a_character_no_keyboard_types_is_spelled_rather_than_sent() {
    assert_eq!(quote("a\u{7}b"), Ok(r#""a\007b""#.to_string()));
  }

  #[test]
  fn splits_a_line_into_rules_and_command() {
    let Ok(rules) = LaunchRules::from_legacy("workspace 2 silent");

    assert_eq!(
      split_exec_line("[workspace 2 silent] foot --working-directory=/home"),
      Ok(Start {
        rules,
        command: "foot --working-directory=/home"
      })
    );
    assert_eq!(
      split_exec_line("foot"),
      Ok(Start {
        rules: LaunchRules::default(),
        command: "foot"
      })
    );
  }

  #[test]
  fn a_saved_monitor_rule_uses_the_current_connector() {
    let line = "[monitor 1;workspace 7 silent] foot";
    let monitors = BTreeMap::from([("7", "DP-3")]);
    let Ok(resolved) = LaunchRules::from_legacy("workspace 7 silent;monitor DP-3");
    let Ok(undocked) = LaunchRules::from_legacy("workspace 7 silent");

    assert_eq!(
      split_exec_line_on(line, Some(&monitors)),
      Ok(Start {
        rules: resolved,
        command: "foot"
      })
    );
    assert_eq!(
      split_exec_line_on(line, Some(&BTreeMap::new())),
      Ok(Start {
        rules: undocked,
        command: "foot"
      })
    );
  }

  #[test]
  fn a_window_is_named_to_a_dispatcher_by_the_address_it_answers_to() {
    assert_eq!(window("0x55f1"), Ok(r#""address:0x55f1""#.to_string()));
  }
}
