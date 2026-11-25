//! The Linux delivery backend for desktop notifications.

use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_program_contract::{Command as EffectCommand, Executable};

const SENDER: Program = Program::NotifySend;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Urgency {
  #[default]
  ServerDefault,
  Normal,
  Low,
  Critical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IdOutput {
  #[default]
  Omitted,
  Requested,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum History {
  #[default]
  Keep,
  Transient,
}

#[derive(Clone, Copy, Debug)]
pub struct Action<'a> {
  pub key: &'a str,
  pub label: &'a str,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
  pub app_name: Option<&'a str>,
  pub icon: Option<&'a str>,
  pub expire_ms: Option<u32>,
  pub urgency: Urgency,
  pub synchronous_key: Option<&'a str>,
  pub id_output: IdOutput,
  pub history: History,
  pub replace_id: Option<u32>,
  pub value: Option<i64>,
}

impl Options<'_> {
  #[must_use]
  fn flags(self) -> Result<Vec<String>, Never> {
    let urgency = match self.urgency {
      Urgency::ServerDefault => None,
      Urgency::Normal => Some("--urgency=normal".to_string()),
      Urgency::Low => Some("--urgency=low".to_string()),
      Urgency::Critical => Some("--urgency=critical".to_string()),
    };
    let print_id = match self.id_output {
      IdOutput::Omitted => None,
      IdOutput::Requested => Some("--print-id".to_string()),
    };
    let history = match self.history {
      History::Keep => None,
      History::Transient => Some("--hint=int:transient:1".to_string()),
    };

    let mut options: Vec<String> = [
      self.app_name.map(|name| format!("--app-name={name}")),
      self.icon.map(|icon| format!("--icon={icon}")),
      print_id,
      history,
      urgency,
      self.expire_ms.map(|time| format!("--expire-time={time}")),
      self
        .synchronous_key
        .map(|key| format!("--hint=string:x-canonical-private-synchronous:{key}")),
    ]
    .into_iter()
    .flatten()
    .collect();

    match self.value {
      Some(value) => options.extend(["-h".to_string(), format!("int:value:{value}")]),
      None => {}
    }

    options.extend(self.replace_id.map(|id| format!("--replace-id={id}")));

    Ok(options)
  }

  pub fn effect(self, summary: &str, body: Option<&str>) -> Result<EffectCommand, Never> {
    let Ok(mut arguments) = self.flags();
    arguments.push("--".to_string());
    arguments.push(summary.to_string());
    arguments.extend(body.map(str::to_string));

    Ok(EffectCommand {
      program: Executable::External(SENDER),
      arguments,
    })
  }

  pub fn command(self, summary: &str, body: Option<&str>) -> Result<Command, Never> {
    let Ok(effect) = self.effect(summary, body);
    let Ok(mut command) = SENDER.command();
    command.args(effect.arguments);

    Ok(command)
  }

  pub fn action_command(self, summary: &str, action: Action<'_>) -> Result<Command, Never> {
    let Ok(effect) = self.effect(summary, None);
    let Ok(mut command) = SENDER.command();
    command.arg(format!("--action={}={}", action.key, action.label));
    command.args(effect.arguments);

    Ok(command)
  }

  pub fn invocation(self, summary: &str, body: Option<&str>) -> Result<Vec<String>, Never> {
    let Ok(effect) = self.effect(summary, body);

    SENDER.words(effect.arguments)
  }
}
