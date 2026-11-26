//! Output layout and workspace placement reported by Hyprland.
//! A display edit holds the same lock that excludes other edits and session snapshots.

use console_core_never::Never;
use rustix::fs::{OFlags, open};
use serde_json::Value;
use std::fmt;
use std::fs::{File, TryLockError};

use crate::{HyprctlError, Query, Question, Script, Workspace, parse_workspace, quote};

#[derive(Debug, Clone, Copy)]
pub struct SessionClients;

#[derive(Debug, PartialEq, Eq)]
pub struct SessionClient {
  pub window: crate::Window,
  pub tags: Vec<String>,
}

impl Question for SessionClients {
  const ASKED: Query = Query::Clients;
  type Reply = Vec<SessionClient>;

  fn said(value: Value) -> Result<Self::Reply, HyprctlError> {
    let listed = value
      .as_array()
      .ok_or(HyprctlError::MissingField(Query::Clients, "a list"))?;
    Ok(
      listed
        .iter()
        .filter_map(|client| {
          let Ok(parsed) = crate::parse_window(client);
          let window = parsed?;
          let tags = client
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();

          Some(SessionClient { window, tags })
        })
        .collect(),
    )
  }
}

#[derive(Debug, Clone, Copy)]
pub struct Outputs;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped numeric field is a refresh frequency in hertz"
  )
)]
pub struct RefreshHertz(pub f64);

impl fmt::Display for RefreshHertz {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(to)
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped field is a dimensionless ratio of physical to logical pixels"
  )
)]
pub struct OutputScale(pub f64);

impl fmt::Display for OutputScale {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(to)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFocus {
  Focused,
  Unfocused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchUpdate {
  Preserve,
  Rotate,
}

impl From<bool> for TouchUpdate {
  fn from(rotate: bool) -> Self {
    match rotate.then_some(()) {
      Some(()) => Self::Rotate,
      None => Self::Preserve,
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
  pub id: i64,
  pub connector: String,
  pub make: String,
  pub model: String,
  pub serial: String,
  pub physical_size_mm: Option<(i64, i64)>,
  pub mode: (i64, i64),
  pub refresh: RefreshHertz,
  pub available_modes: Vec<Mode>,
  pub position: (i64, i64),
  pub scale: OutputScale,
  pub transform: i64,
  pub mirror: Option<i64>,
  pub active: Option<Workspace>,
  pub focused: OutputFocus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mode {
  pub size: (i64, i64),
  pub refresh: RefreshHertz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeMatch {
  Same,
  Different,
}

impl Mode {
  #[must_use]
  pub fn read(said: &str) -> Result<Self, InvalidMode> {
    let mode = match said.strip_suffix("Hz") {
      Some(without_units) => without_units,
      None => said,
    };
    let (size, frequency) = mode.split_once('@').ok_or(InvalidMode)?;
    let (width, height) = size.split_once('x').ok_or(InvalidMode)?;
    let wide = width.parse::<i64>().map_err(|_| InvalidMode)?;
    let tall = height.parse::<i64>().map_err(|_| InvalidMode)?;
    let refresh = frequency.parse::<f64>().map_err(|_| InvalidMode)?;

    match (wide > 0 && tall > 0 && refresh.is_finite() && refresh > 0.0).then_some(()) {
      Some(()) => Ok(Self {
        size: (wide, tall),
        refresh: RefreshHertz(refresh),
      }),
      None => Err(InvalidMode),
    }
  }

  #[must_use]
  pub fn matches(self, other: Self) -> Result<ModeMatch, Never> {
    let equivalent = self.size == other.size && (self.refresh.0 - other.refresh.0).abs() < 0.0051;
    Ok(match equivalent.then_some(()) {
      Some(()) => ModeMatch::Same,
      None => ModeMatch::Different,
    })
  }
}

impl std::fmt::Display for Mode {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let (wide, tall) = self.size;
    write!(to, "{wide}x{tall}@{}", self.refresh)
  }
}

impl Output {
  #[must_use]
  pub fn current_mode(&self) -> Result<Mode, Never> {
    Ok(Mode {
      size: self.mode,
      refresh: self.refresh,
    })
  }

  #[must_use]
  pub fn rule<'a>(&'a self, outputs: &'a [Self]) -> Result<OutputRule<'a>, Never> {
    let source = outputs.iter().find(|other| Some(other.id) == self.mirror);
    Ok(OutputRule {
      connector: &self.connector,
      mode: self.mode,
      refresh: self.refresh,
      position: source.map_or(self.position, |other| other.position),
      scale: self.scale,
      transform: self.transform,
      mirror: source.map(|other| other.connector.as_str()),
      touch: TouchUpdate::Preserve,
    })
  }
}

pub fn try_edit() -> Result<Option<File>, std::io::Error> {
  let request_socket = crate::socket::socket(crate::socket::Socket::Requests)
    .map_err(|fault| std::io::Error::new(std::io::ErrorKind::NotFound, fault))?;
  let at = request_socket.with_file_name("console-display.lock");
  let descriptor = open(
    at,
    OFlags::CREATE | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
  )?;
  let lock = File::from(descriptor);

  match lock.try_lock() {
    Ok(()) => Ok(Some(lock)),
    Err(TryLockError::WouldBlock) => Ok(None),
    Err(TryLockError::Error(fault)) => Err(fault),
  }
}

impl Question for Outputs {
  const ASKED: Query = Query::EveryMonitor;
  type Reply = Vec<Output>;

  fn said(value: Value) -> Result<Vec<Output>, HyprctlError> {
    let listed = value
      .as_array()
      .ok_or(HyprctlError::MissingField(Query::EveryMonitor, "a list"))?;
    listed
      .iter()
      .filter(|output| {
        output.get("disabled").and_then(Value::as_bool) != Some(true)
          && output.get("name").and_then(Value::as_str) != Some("FALLBACK")
      })
      .map(output)
      .collect()
  }
}

fn output(value: &Value) -> Result<Output, HyprctlError> {
  let missing = |field| HyprctlError::MissingField(Query::EveryMonitor, field);
  let text = |field| value.get(field).and_then(Value::as_str);
  let integer = |field| value.get(field).and_then(Value::as_i64);
  let number = |field| integer(field).ok_or_else(|| missing(field));
  let decimal = |field| {
    value
      .get(field)
      .and_then(Value::as_f64)
      .ok_or_else(|| missing(field))
  };
  let active = value.get("activeWorkspace").and_then(|workspace| {
    let Ok(parsed) = parse_workspace(workspace);
    parsed
  });
  let id = number("id")?;
  let connector = text("name").ok_or(missing("name"))?;
  let wide = number("width")?;
  let tall = number("height")?;
  let refresh = decimal("refreshRate")?;
  let x = number("x")?;
  let y = number("y")?;
  let scale = decimal("scale")?;
  let transform = number("transform")?;

  Ok(Output {
    id,
    connector: connector.to_owned(),
    make: match text("make") {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => "",
    }
    .to_owned(),
    model: match text("model") {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => "",
    }
    .to_owned(),
    serial: match text("serial") {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => "",
    }
    .to_owned(),
    physical_size_mm: integer("physicalWidth")
      .zip(integer("physicalHeight"))
      .filter(|(physical_width, physical_height)| *physical_width > 0 && *physical_height > 0),
    mode: (wide, tall),
    refresh: RefreshHertz(refresh),
    available_modes: value
      .get("availableModes")
      .and_then(Value::as_array)
      .into_iter()
      .flatten()
      .filter_map(Value::as_str)
      .filter_map(|spelling| match Mode::read(spelling) {
        Ok(mode) => Some(mode),
        Err(InvalidMode) => {
          console_core_logging::debug!("ignoring an invalid mode for {connector}: {spelling:?}");
          None
        }
      })
      .collect(),
    position: (x, y),
    scale: OutputScale(scale),
    transform,
    mirror: text("mirrorOf").and_then(|source| match source.parse() {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None,
    }),
    active: active.filter(|workspace| !workspace.named.is_empty()),
    focused: match (value.get("focused").and_then(Value::as_bool) == Some(true)).then_some(()) {
      Some(()) => OutputFocus::Focused,
      None => OutputFocus::Unfocused,
    },
  })
}

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceLocations;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceLocation {
  pub workspace: Workspace,
  pub connector: String,
  pub layout: Option<String>,
}

impl Question for WorkspaceLocations {
  const ASKED: Query = Query::Workspaces;
  type Reply = Vec<WorkspaceLocation>;

  fn said(value: Value) -> Result<Vec<WorkspaceLocation>, HyprctlError> {
    let listed = value
      .as_array()
      .ok_or(HyprctlError::MissingField(Query::Workspaces, "a list"))?;

    Ok(
      listed
        .iter()
        .filter_map(|one| {
          let Ok(parsed) = parse_workspace(one);
          let workspace = parsed?;
          let connector = one.get("monitor").and_then(Value::as_str)?;

          Some(WorkspaceLocation {
            workspace,
            connector: connector.to_owned(),
            layout: one
              .get("tiledLayout")
              .and_then(Value::as_str)
              .map(str::to_owned),
          })
        })
        .collect(),
    )
  }
}

#[must_use]
pub fn workspace_selector(id: i64, name: &str) -> Result<String, Never> {
  let Ok(kind) = crate::WorkspaceKind::of(name);

  Ok(match kind {
    crate::WorkspaceKind::Special => name.to_owned(),
    crate::WorkspaceKind::Regular => match {
      ::core::primitive::bool::then_some(id < 0 && !name.is_empty() && name != id.to_string(), ())
    } {
      ::core::option::Option::Some(()) => {
        format!("name:{name}")
      }
      ::core::option::Option::None => id.to_string(),
    },
  })
}

#[derive(Debug, Clone, Copy)]
pub struct OutputRule<'a> {
  pub connector: &'a str,
  pub mode: (i64, i64),
  pub refresh: RefreshHertz,
  pub position: (i64, i64),
  pub scale: OutputScale,
  pub transform: i64,
  pub mirror: Option<&'a str>,
  pub touch: TouchUpdate,
}

pub fn configure_outputs(rules: &[OutputRule<'_>]) -> Result<Script, Never> {
  let mut lines: Vec<_> = rules
    .iter()
    .map(|rule| {
      let Ok(output) = quote(rule.connector);
      let Ok(mirror) = quote(match rule.mirror {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => "",
      });
      let (wide, tall) = rule.mode;
      let (x, y) = rule.position;
      let refresh = rule.refresh;
      let scale = rule.scale;
      let transform = rule.transform;

      format!(
        r#"hl.monitor({{ output = {output}, mode = "{wide}x{tall}@{refresh}", position = "{x}x{y}", scale = {scale}, transform = {transform}, mirror = {mirror} }})"#
      )
    })
    .collect();

  match rules.iter().find(|rule| rule.touch == TouchUpdate::Rotate) {
    Some(panel) => {
      let Ok(touch) = crate::lua::touch_transform(panel.connector, panel.transform);

      lines.push(touch);
    }
    ::core::option::Option::None => {}
  }

  Script::from_lua(lines.join(" "))
}
