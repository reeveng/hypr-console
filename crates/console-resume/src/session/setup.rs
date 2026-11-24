//! A session keeps one setup per set of connected displays. Output IDs only
//! identify the current compositor's outputs and never go into the setup file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use console_compositor::setup::{
  Mode, ModeMatch, Output, OutputFocus, OutputRule, OutputScale, Outputs, RefreshHertz,
  TouchUpdate, WorkspaceLocations, configure_outputs, workspace_selector,
};
use console_compositor::{DispatchResult, Operation, Script};
use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_program_lifetime::Detached;
use console_waiting::Schedule;
use serde::{Deserialize, Serialize};

use super::Really;
use crate::Unresumed;

mod first_connection;

const SETUPS: &str = "setups.json";
const LOCK: &str = "setups.lock";
const SAMPLE_EVERY: Duration = Duration::from_secs(1);
const DISPLAY_WAIT: Duration = Duration::from_secs(2);
const DISPLAY_POLL: Duration = Duration::from_millis(100);

fn try_edit() -> Result<Option<std::fs::File>, Unresumed> {
  console_compositor::setup::try_edit()
    .map_err(|fault| Unresumed::ApplyingSetup(fault.to_string()))
}

pub(super) fn edit() -> Result<std::fs::File, Unresumed> {
  let editing = try_edit()?;
  editing
    .ok_or_else(|| Unresumed::ApplyingSetup("another display change is in progress".to_owned()))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Display {
  make: super::SavedText,
  model: super::SavedText,
  serial: super::SavedText,
  connector: Option<super::SavedText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, from = "f64", into = "f64")]
struct SavedRefresh(RefreshHertz);

impl From<f64> for SavedRefresh {
  fn from(value: f64) -> Self {
    Self(RefreshHertz(value))
  }
}

impl From<SavedRefresh> for f64 {
  fn from(SavedRefresh(RefreshHertz(value)): SavedRefresh) -> Self {
    value
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, from = "f64", into = "f64")]
struct SavedScale(OutputScale);

impl From<f64> for SavedScale {
  fn from(value: f64) -> Self {
    Self(OutputScale(value))
  }
}

impl From<SavedScale> for f64 {
  fn from(SavedScale(OutputScale(value)): SavedScale) -> Self {
    value
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Geometry {
  mode: (i64, i64),
  refresh: SavedRefresh,
  position: (i64, i64),
  scale: SavedScale,
  transform: i64,
  mirror: Option<Display>,
}

enum Agreement {
  Same,
  Different,
}

impl From<bool> for Agreement {
  fn from(same: bool) -> Self {
    match bool::then_some(same, ()) {
      Some(()) => Self::Same,
      None => Self::Different,
    }
  }
}

impl Geometry {
  fn matches(&self, actual: &Self) -> Result<Agreement, Never> {
    let saved_mode = Mode {
      size: self.mode,
      refresh: self.refresh.0,
    };
    let actual_mode = Mode {
      size: actual.mode,
      refresh: actual.refresh.0,
    };

    let Ok(agreement) = saved_mode.matches(actual_mode);
    let same = agreement == ModeMatch::Same
      && self.position == actual.position
      && self.scale == actual.scale
      && self.transform == actual.transform
      && self.mirror == actual.mirror;
    Ok(Agreement::from(same))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "bool", into = "bool")]
enum Focus {
  Focused,
  Unfocused,
}

impl From<bool> for Focus {
  fn from(focused: bool) -> Self {
    match focused.then_some(()) {
      Some(()) => Self::Focused,
      None => Self::Unfocused,
    }
  }
}

impl From<Focus> for bool {
  fn from(focus: Focus) -> Self {
    matches!(focus, Focus::Focused)
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Screen {
  display: Display,
  geometry: Geometry,
  #[serde(default)]
  specifications: first_connection::Specifications,
  active: Option<String>,
  focused: Focus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
struct ScreenList(Vec<Screen>);

impl<'de> Deserialize<'de> for ScreenList {
  fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    decoder.deserialize_seq(ScreenSequence)
  }
}

struct ScreenSequence;

impl<'de> serde::de::Visitor<'de> for ScreenSequence {
  type Value = ScreenList;

  fn expecting(&self, message: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    message.write_str("a list of displays within the session file limit")
  }

  fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
    const LIMIT: u32 = console_core_atomic_writes::MAX_TEXT_BYTES / 2;
    let mut screens = Vec::new();

    for _ in 0..LIMIT {
      let next = input.next_element()?;

      match next {
        Some(screen) => screens.push(screen),
        None => return Ok(ScreenList(screens)),
      }
    }

    let extra = input.next_element::<serde::de::IgnoredAny>()?;

    match extra {
      Some(_) => Err(serde::de::Error::custom("too many displays in the session setup")),
      None => Ok(ScreenList(screens)),
    }
  }
}

impl std::ops::Deref for ScreenList {
  type Target = [Screen];

  fn deref(&self) -> &[Screen] {
    &self.0
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
  screens: ScreenList,
  workspaces: BTreeMap<String, Display>,
}

pub(super) struct Live {
  outputs: Vec<(Display, Output)>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Topology(Vec<(Display, String, i64)>);

impl Live {
  pub(super) fn read() -> Result<Self, Unresumed> {
    let outputs = console_compositor::ask(Outputs)?;

    let Ok(live) = Self::from_outputs(outputs);
    Ok(live)
  }

  fn from_outputs(outputs: Vec<Output>) -> Result<Self, Never> {
    let mut displays: Vec<_> = outputs
      .iter()
      .map(|output| {
        let same_panel = |other: &&Output| {
          (
            other.make.as_str(),
            other.model.as_str(),
            other.serial.as_str(),
          ) == (
            output.make.as_str(),
            output.model.as_str(),
            output.serial.as_str(),
          )
        };
        let serial = output.serial.trim();
        let unique = !serial.is_empty()
          && serial != "0"
          && outputs.iter().filter(same_panel).nth(1).is_none();
        Display {
          make: super::SavedText(output.make.clone()),
          model: super::SavedText(output.model.clone()),
          serial: super::SavedText(output.serial.clone()),
          connector: (!unique).then(|| super::SavedText(output.connector.clone())),
        }
      })
      .zip(outputs.iter().cloned())
      .collect();
    displays.sort_by(|(a, _), (b, _)| a.cmp(b));

    Ok(Self { outputs: displays })
  }

  pub(super) fn topology(&self) -> Result<Topology, Never> {
    Ok(Topology(
      self
        .outputs
        .iter()
        .map(|(display, output)| (display.clone(), output.connector.clone(), output.id))
        .collect(),
    ))
  }

  fn displays(&self) -> Result<Displays, Never> {
    Ok(match self.outputs.as_slice() {
      [] => Displays::Missing,
      [_, ..] => Displays::Connected,
    })
  }

  fn output(&self, display: &Display) -> Result<Option<&Output>, Never> {
    Ok(
      self
        .outputs
        .iter()
        .find_map(|(identity, output)| bool::then_some(identity == display, output)),
    )
  }

  fn geometry(&self, output: &Output) -> Result<Geometry, Never> {
    let mirror = self
      .outputs
      .iter()
      .find(|(_, source)| Some(source.id) == output.mirror);

    Ok(Geometry {
      mode: output.mode,
      refresh: SavedRefresh(output.refresh),

      position: mirror.map_or(output.position, |(_, source)| source.position),
      scale: SavedScale(output.scale),
      transform: output.transform,
      mirror: mirror.map(|(display, _)| display.clone()),
    })
  }

  fn snapshot(&self) -> Result<Profile, Unresumed> {
    let locations = console_compositor::ask(WorkspaceLocations)?;
    let mut connectors = BTreeMap::new();

    for (display, output) in &self.outputs {
      let _first = connectors
        .entry(output.connector.as_str())
        .or_insert(display);
    }

    let workspaces = locations
      .into_iter()
      .filter_map(|location| {
        let display = connectors.get(location.connector.as_str())?;
        let workspace = location.workspace;
        let Ok(selector) = workspace_selector(workspace.id, &workspace.named);

        Some((selector, (*display).clone()))
      })
      .collect();
    let screens = self
      .outputs
      .iter()
      .map(|(display, output)| {
        let Ok(geometry) = self.geometry(output);
        let Ok(specifications) = first_connection::Specifications::of(output);

        Screen {
          display: display.clone(),
          geometry,
          specifications,
          active: output.active.as_ref().map(|workspace| {
            let Ok(selector) = workspace_selector(workspace.id, &workspace.named);
            selector
          }),
          focused: Focus::from(output.focused == OutputFocus::Focused),
        }
      })
      .collect();

    Ok(Profile {
      screens: ScreenList(screens),
      workspaces,
    })
  }
}

pub(super) fn read(at: &Path) -> Result<Vec<Profile>, Unresumed> {
  let file = at.join(SETUPS);
  let Ok(stored) = console_core_atomic_writes::read(&file);

  match stored {
    Stored::Absent => Ok(Vec::new()),
    Stored::Failed(fault) => Err(Unresumed::Read(file, fault)),
    Stored::Text(text) => {
      serde_json::from_str(&text).map_err(|fault| Unresumed::Unparsed(file, fault))
    }
  }
}

pub(super) fn matching<'a>(
  profiles: &'a [Profile],
  live: &Live,
) -> Result<Option<&'a Profile>, Never> {
  Ok(
    profiles
      .iter()
      .find(|profile| matches!(profile.matches(live), Ok(Agreement::Same))),
  )
}

enum Displays {
  Missing,
  Connected,
}

#[derive(Clone, Copy)]
pub(super) enum RecallMode {
  Remember,
  Recall,
}

#[derive(Default)]
pub(super) struct Watcher {
  last_sample: Option<Instant>,
  seen: Option<Topology>,
  active: Option<Topology>,
  notifications: Vec<Detached>,
}

impl Watcher {
  pub(super) fn changed(&mut self) -> Result<(), Never> {
    self.seen = None;
    self.active = None;
    Ok(())
  }

  pub(super) fn step(
    &mut self,
    now: Instant,
    at: &Path,
    really: Really,
    recall: RecallMode,
  ) -> Result<(), Unresumed> {
    let too_soon = self
      .last_sample
      .is_some_and(|last| now.duration_since(last) < SAMPLE_EVERY);

    match bool::then_some(too_soon, ()) {
      Some(()) => return Ok(()),
      None => {}
    }

    self.last_sample = Some(now);
    let Ok(notifications) =
      console_program_lifetime::reap(std::mem::take(&mut self.notifications));
    self.notifications = notifications;

    let available_edit = try_edit()?;
    let editing = match available_edit {
      Some(available) => available,
      None => return Ok(()),
    };
    let live = Live::read()?;
    let Ok(displays) = live.displays();

    match displays {
      Displays::Missing => {
        let Ok(()) = self.changed();
        return Ok(());
      }
      Displays::Connected => {}
    }

    let Ok(topology) = live.topology();

    match bool::then_some(self.seen.as_ref() == Some(&topology), ()) {
      Some(()) => {}
      None => {
        self.seen = Some(topology);
        return Ok(());
      }
    }

    let settled = self.active.as_ref() == Some(&topology);

    match (bool::then_some(settled, ()), recall) {
      (None, RecallMode::Recall) => {
        let profiles = read(at)?;
        let Ok(profile) = matching(&profiles, &live);

        match profile {
          Some(saved) => {
            saved.recall(&live, really)?;
            self.active = Some(topology);
            return Ok(());
          }
          None => {}
        }

        match really {
          Really::Truly => {
            let (current, notices) = first_connection::prepare(&profiles, live)?;
            save(at, &current)?;
            self.active = Some(topology);
            drop(editing);
            self
              .notifications
              .extend(notices.into_iter().filter_map(|notice| {
                let Ok(child) = first_connection::notify(notice);
                child
              }));
            return Ok(());
          }
          Really::Simulated => self.active = Some(topology),
        }
      }
      (None, RecallMode::Remember) => self.active = Some(topology),
      (Some(()), _) => {}
    }

    match really {
      Really::Truly => save(at, &live),
      Really::Simulated => Ok(()),
    }
  }
}

pub(super) fn save(at: &Path, live: &Live) -> Result<(), Unresumed> {
  let Ok(displays) = live.displays();

  match displays {
    Displays::Missing => return Err(Unresumed::NoDisplays),
    Displays::Connected => {}
  }

  fs::create_dir_all(at).map_err(|fault| Unresumed::Making(at.to_owned(), fault))?;
  let lock = at.join(LOCK);
  let guard = console_core_atomic_writes::open_lock_file(&lock)
    .map_err(|fault| Unresumed::Making(lock.clone(), fault))?;
  guard
    .lock()
    .map_err(|fault| Unresumed::Unsaid(lock.display().to_string(), fault))?;

  let mut profiles = read(at)?;
  let current = live.snapshot()?;

  match profiles
    .iter()
    .position(|profile| matches!(profile.matches(live), Ok(Agreement::Same)))
  {
    Some(previous) => {
      match { ::core::primitive::bool::then_some(profiles.get(previous) == Some(&current), ()) } {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      profiles = profiles
        .into_iter()
        .enumerate()
        .filter_map(|(index, profile)| bool::then_some(index != previous, profile))
        .collect();
    }
    ::core::option::Option::None => {}
  }

  profiles.push(current);

  let live_read = Live::read()?;

  let Ok(observed) = live_read.topology();
  let Ok(expected) = live.topology();

  match bool::then_some(observed != expected, ()) {
    ::core::option::Option::Some(()) => {
      return Err(Unresumed::DisplaysChanged);
    }
    ::core::option::Option::None => {}
  }

  let file = at.join(SETUPS);
  let bytes =
    serde_json::to_vec(&profiles).map_err(|fault| Unresumed::Unparsed(file.clone(), fault))?;
  console_core_atomic_writes::whole(&file, &bytes).map_err(Unresumed::Unwritten)
}

impl Profile {
  fn matches(&self, live: &Live) -> Result<Agreement, Never> {
    let same = self
      .screens
      .iter()
      .map(|screen| &screen.display)
      .eq(live.outputs.iter().map(|(display, _)| display));
    Ok(Agreement::from(same))
  }

  pub(super) fn workspace_outputs<'a>(
    &'a self,
    live: &'a Live,
  ) -> Result<BTreeMap<&'a str, &'a str>, Never> {
    Ok(
      self
        .workspaces
        .iter()
        .filter_map(|(workspace, display)| {
          let Ok(found) = live.output(display);
          let output = found?;
          Some((workspace.as_str(), output.connector.as_str()))
        })
        .collect(),
    )
  }

  pub(super) fn recall(&self, live: &Live, really: Really) -> Result<(), Unresumed> {
    let displays = self.apply_displays(live, really);
    let workspaces = Live::read().and_then(|current| self.apply_workspaces(&current, really));

    displays.and(workspaces)
  }

  pub(super) fn apply_displays(&self, live: &Live, really: Really) -> Result<(), Unresumed> {
    let mut rules = Vec::new();

    for screen in self.screens.iter() {
      let output = {
        let Ok(optional_output) = live.output(&screen.display);

        match optional_output {
          Some(matched_output) => matched_output,
          ::core::option::Option::None => continue,
        }
      };

      let geometry = &screen.geometry;

      let Ok(actual) = live.geometry(output);
      let Ok(agreement) = geometry.matches(&actual);

      match agreement {
        Agreement::Same => continue,
        Agreement::Different => {}
      }

      match {
        ::core::primitive::bool::then_some(
          geometry.mode.0 <= 0
            || geometry.mode.1 <= 0
            || !f64::from(geometry.refresh).is_finite()
            || f64::from(geometry.refresh) <= 0.0
            || !f64::from(geometry.scale).is_finite()
            || f64::from(geometry.scale) <= 0.0
            || !(0..=7).contains(&geometry.transform),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          return Err(Unresumed::ApplyingSetup(format!(
            "the saved display mode for {} is invalid",
            output.connector
          )));
        }
        ::core::option::Option::None => {}
      }

      let mirror = geometry
        .mirror
        .as_ref()
        .and_then(|display| {
          let Ok(found) = live.output(display);
          found
        })
        .map(|source| source.connector.as_str());
      rules.push(OutputRule {
        connector: &output.connector,
        mode: geometry.mode,
        refresh: geometry.refresh.0,
        position: geometry.position,
        scale: geometry.scale.0,
        transform: geometry.transform,
        mirror,
        touch: TouchUpdate::from(
          output.connector.starts_with("eDP-") && output.transform != geometry.transform,
        ),
      });
    }

    match { ::core::primitive::bool::then_some(rules.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let duplicated: BTreeSet<_> = rules.iter().filter_map(|rule| rule.mirror).collect();
    let (sources, remaining): (Vec<_>, Vec<_>) = rules
      .iter()
      .copied()
      .partition(|rule| rule.mirror.is_none() && duplicated.contains(rule.connector));

    match { ::core::primitive::bool::then_some(!sources.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        let Ok(script) = configure_outputs(&sources);
        apply(&script, really)?;

        match { ::core::primitive::bool::then_some(really == Really::Truly, ()) } {
          ::core::option::Option::Some(()) => {
            self.wait_for_outputs(|current| {
              let ready: BTreeSet<_> = current
                .outputs
                .iter()
                .filter(|(_, output)| output.mirror.is_none())
                .map(|(_, output)| output.connector.as_str())
                .collect();

              sources
                .iter()
                .all(|source| ready.contains(source.connector))
            })?;
          }
          ::core::option::Option::None => {}
        }
      }
      ::core::option::Option::None => {}
    }

    let Ok(script) = configure_outputs(&remaining);
    apply(&script, really)?;

    match { ::core::primitive::bool::then_some(really == Really::Simulated, ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    self.wait_for_outputs(|current| {
      self.screens.iter().all(|screen| {
        let Ok(found) = current.output(&screen.display);
        found.is_some_and(|output| {
          let Ok(observed) = current.geometry(output);

          matches!(screen.geometry.matches(&observed), Ok(Agreement::Same))
        })
      })
    })
  }

  fn wait_for_outputs(&self, ready: impl Fn(&Live) -> bool) -> Result<(), Unresumed> {
    let Ok(patience) = Schedule::asking_every(DISPLAY_WAIT, DISPLAY_POLL);
    let Ok(ready_2) = console_waiting::until_some(patience, || {
      let current = match Live::read() {
        Ok(current) => current,
        Err(fault) => return Ok(Some(Err(fault))),
      };

      match {
        ::core::primitive::bool::then_some(
          matches!(self.matches(&current), Ok(Agreement::Different)),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          return Ok(Some(Err(Unresumed::DisplaysChanged)));
        }
        ::core::option::Option::None => {}
      }

      Ok(ready(&current).then_some(Ok(())))
    });

    match ready_2 {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => Err(Unresumed::ApplyingSetup(
        "the displays did not reach the saved layout".to_owned(),
      )),
    }
  }

  pub(super) fn apply_workspaces(&self, live: &Live, really: Really) -> Result<(), Unresumed> {
    match {
      ::core::primitive::bool::then_some(
        matches!(self.matches(live), Ok(Agreement::Different)),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(Unresumed::DisplaysChanged);
      }
      ::core::option::Option::None => {}
    }

    let locations = console_compositor::ask(WorkspaceLocations)?;
    let located: BTreeMap<_, _> = locations
      .iter()
      .map(|location| {
        let workspace = &location.workspace;
        let Ok(selector) = workspace_selector(workspace.id, &workspace.named);
        (selector, location.connector.as_str())
      })
      .collect();
    let mut operations = Vec::new();

    for (workspace, display) in &self.workspaces {
      let Ok(found) = live.output(display);

      match (located.get(workspace), found) {
        (Some(connector), Some(output)) => {
          let move_to = *connector != output.connector && output.mirror.is_none();

          match bool::then_some(move_to, ()) {
            Some(()) => operations.push(Operation::MoveWorkspaceToOutput {
              workspace,
              output: &output.connector,
            }),
            None => {}
          }
        }
        (Some(_) | None, None) | (None, Some(_)) => {}
      }
    }

    let moved = !operations.is_empty();

    for screen in self.screens.iter() {
      let (active, output) = {
        let Ok(found) = live.output(&screen.display);
        let outcome_active = (&screen.active, found);

        match outcome_active {
          (Some(matched_active), Some(matched_output)) => (matched_active, matched_output),
          (::core::option::Option::None, _) | (_, ::core::option::Option::None) => continue,
        }
      };

      match { ::core::primitive::bool::then_some(output.mirror.is_some(), ()) } {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      let visible = output.active.as_ref().map(|workspace| {
        let Ok(selector) = workspace_selector(workspace.id, &workspace.named);
        selector
      });

      match { ::core::primitive::bool::then_some(!moved && visible.as_ref() == Some(active), ()) }
      {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(!located.contains_key(active), ()) } {
        ::core::option::Option::Some(()) => {
          operations.push(Operation::FocusMonitor(&output.connector));
        }
        ::core::option::Option::None => {}
      }

      operations.push(Operation::FocusWorkspace(active));
    }

    let focused = self
      .screens
      .iter()
      .find(|screen| screen.focused == Focus::Focused)
      .and_then(|screen| {
        let Ok(found) = live.output(&screen.display);
        found
      });

    match focused {
      Some(output) => {
        let needs_focus = !operations.is_empty() || output.focused == OutputFocus::Unfocused;
        operations.extend(bool::then_some(
          needs_focus,
          Operation::FocusMonitor(&output.connector),
        ));
      }
      None => {}
    }

    match { ::core::primitive::bool::then_some(operations.is_empty(), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(script) = Script::operations(&operations);
    apply(&script, really)
  }
}

fn apply(script: &Script, really: Really) -> Result<(), Unresumed> {
  match { ::core::primitive::bool::then_some(really == Really::Simulated, ()) } {
    ::core::option::Option::Some(()) => {
      println!("{script}");
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let Ok(result) = script.run();

  match result {
    DispatchResult::Success => Ok(()),
    DispatchResult::Failure(fault) => Err(Unresumed::ApplyingSetup(fault)),
  }
}
