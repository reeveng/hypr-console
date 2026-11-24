//! Sample one focused, visible window and account for elapsed time between samples.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use console_applications::entry::{self, DesktopEntry, Worth};
use console_compositor::socket;
use console_core_atomic_writes::whole;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::idle::{Idle, SeatState};
use crate::{APP_ID, DAY, Paths, Result, browser::Sites, now, private_directory, report, stored};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Activity {
  pub app: String,
  pub site: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Span {
  pub start: u64,
  pub end: u64,
  #[serde(flatten)]
  pub activity: Activity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSpan {
  start: u64,
  end: u64,
  app: stored::Text<65_536>,
  site: Option<stored::Text<65_536>>,
}

impl From<StoredSpan> for Span {
  fn from(record: StoredSpan) -> Self {
    Self {
      start: record.start,
      end: record.end,
      activity: Activity {
        app: record.app.0,
        site: record.site.map(|site| site.0),
      },
    }
  }
}

pub struct Window {
  pub address: String,
  pub pid: Option<u32>,
  class: String,
  monitor: i64,
}

const HISTORY_DAYS: u64 = 31;
const DAY_BYTES: u32 = 16 * 1024 * 1024;

fn body<T: DeserializeOwned>(request: &str) -> Result<T> {
  let reply = socket::send(request)?;
  let body = serde_json::from_slice(&reply)?;
  Ok(body)
}

struct FocusedReply {
  address: Option<String>,
  pid: Option<i64>,
  mapped: Option<bool>,
  hidden: Option<bool>,
  class: Option<String>,
  initial_class: Option<String>,
  title: Option<String>,
  monitor: Option<i64>,
}

type Properties = Map<String, Value>;

fn property<T: DeserializeOwned>(properties: &mut Properties, name: &str) -> Result<Option<T>> {
  match properties.remove(name) {
    Some(value) => serde_json::from_value(value).map_err(Box::from),
    None => Ok(None),
  }
}

impl FocusedReply {
  fn readings(mut properties: Properties) -> Result<Self> {
    let address = property(&mut properties, "address")?;
    let pid = property(&mut properties, "pid")?;
    let mapped = property(&mut properties, "mapped")?;
    let hidden = property(&mut properties, "hidden")?;
    let class = property(&mut properties, "class")?;
    let initial_class = property(&mut properties, "initialClass")?;
    let title = property(&mut properties, "title")?;
    let monitor = property(&mut properties, "monitor")?;
    Ok(Self { address, pid, mapped, hidden, class, initial_class, title, monitor })
  }
}

struct MonitorPower {
  id: Option<i64>,
  enabled: Option<bool>,
}

impl MonitorPower {
  fn readings(mut properties: Properties) -> Result<Self> {
    let id = property(&mut properties, "id")?;
    let enabled = property(&mut properties, "dpmsStatus")?;
    Ok(Self { id, enabled })
  }
}

pub fn focused() -> Result<Option<Window>> {
  let answer = body::<Option<Properties>>("j/activewindow")?;

  match answer {
    Some(properties) => {
      let reply = FocusedReply::readings(properties)?;
      let Ok(focused) = window(&reply);
      Ok(focused)
    }
    None => Ok(None),
  }
}

#[must_use]
fn window(
  reply: &FocusedReply,
) -> core::result::Result<Option<Window>, console_core_never::Never> {
  match (reply.mapped, reply.hidden) {
    (Some(true), Some(false)) => {}
    (Some(false) | None, _) | (Some(true), Some(true) | None) => return Ok(None),
  }

  let class = match reply
    .initial_class
    .as_deref()
    .filter(|class| !class.trim().is_empty())
    .or(reply.class.as_deref())
  {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let class_name = class.trim();
  let title = match reply.title.as_deref() {
    ::core::option::Option::Some(present) => present,
    ::core::option::Option::None => "",
  };

  match {
    bool::then_some(
      class_name.is_empty()
        || class_name.eq_ignore_ascii_case(APP_ID)
        || class_name.eq_ignore_ascii_case("habits")
        || class_name.eq_ignore_ascii_case("console-lock-screen")
        || title.starts_with("Habits · Foreground activity"),
      (),
    )
  } {
    Some(()) => {
      return Ok(None);
    }
    None => {}
  }

  let pid = reply.pid.and_then(|pid| match u32::try_from(pid) {
    Ok(pid_2) => Some(pid_2),
    Err(_outside_integer_range) => None,
  });
  let address = match reply.address.as_ref() {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let monitor = match reply.monitor {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  Ok(Some(Window {
    address: address.clone(),
    pid,
    class: class_name
      .chars()
      .filter(|character| !character.is_control())
      .take(128)
      .collect(),
    monitor,
  }))
}

#[must_use]
fn app_names() -> core::result::Result<BTreeMap<String, String>, console_core_never::Never> {
  let Ok(roots) = console_core_places::applications();
  let Ok(paths) = entry::files(&roots);
  let mut names = BTreeMap::new();

  for path in paths {
    let text = match fs::read_to_string(&path) {
      Ok(text) => text,
      Err(_unreadable_desktop_entry) => continue,
    };

    let Ok(entry) = DesktopEntry::read(&text);
    let Ok(worth) = entry.worth();
    let Ok(name) = entry.says();

    let name_2 = {
      let optional_name = name.filter(|_| worth == Worth::Drawing);

      match optional_name {
        Some(matched_name) => matched_name,
        None => continue,
      }
    };

    let id = path.file_stem().and_then(|stem| stem.to_str());

    for class in [
      entry.startup_wm_class,
      id,
      id.and_then(|id_2| id_2.strip_prefix("console-")),
    ]
    .into_iter()
    .flatten()
    {
      names
        .entry(class.to_lowercase())
        .or_insert_with(|| name_2.to_owned());
    }
  }

  Ok(names)
}

fn observe(
  sites: &Sites,
  idle: &Idle<'_>,
  names: &BTreeMap<String, String>,
) -> Result<(Option<Activity>, &'static str)> {
  let mut lock = body::<Properties>("j/locked")?;
  let locked = property(&mut lock, "locked")?;

  match locked {
    Some(true) => return Ok((None, "Screen locked")),
    Some(false) => {}
    None => return Err(Box::from("Hyprland did not report its lock state")),
  }

  let Ok(status) = idle.status();

  match status {
    SeatState::Idle => return Ok((None, "Idle")),
    SeatState::Active => {}
    SeatState::Unavailable => return Ok((None, "Waiting for the Wayland idle monitor")),
  }

  let window = {
    let optional_window = focused()?;

    match optional_window {
      Some(matched_window) => matched_window,
      None => return Ok((None, "No tracked window in focus")),
    }
  };

  let outputs = body::<Vec<Properties>>("j/monitors")?;
  let monitors = outputs.into_iter().map(MonitorPower::readings).collect::<Result<Vec<_>>>()?;
  let on_screen = monitors
    .iter()
    .any(|monitor| monitor.id == Some(window.monitor) && monitor.enabled == Some(true));

  match bool::then_some(!on_screen, ()) {
    Some(()) => {
      return Ok((None, "Display off"));
    }
    None => {}
  }

  let Ok(site) = sites.domain(&window);
  let app = match names.get(&window.class.to_lowercase()) {
    Some(name) => name.clone(),
    None => window.class,
  };
  Ok((Some(Activity { app, site }), "Recording"))
}

struct DayFile(String);

impl AsRef<Path> for DayFile {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

#[must_use]
fn day_path(paths: &Paths, day: u64) -> core::result::Result<PathBuf, console_core_never::Never> {
  Ok(
    paths
      .state
      .join("days")
      .join(DayFile(format!("{day}.json"))),
  )
}

fn read_day(paths: &Paths, day: u64) -> Result<Vec<Span>> {
  let Ok(at) = day_path(paths, day);
  let original = stored::read::<Vec<StoredSpan>>(&at, DAY_BYTES)?;

  Ok(match original {
    Some(spans) => spans.into_iter().map(Span::from).collect(),
    None => Vec::new(),
  })
}

pub fn read_history(paths: &Paths, now: u64) -> Result<Vec<Span>> {
  let last = now.wrapping_div(DAY);
  let mut history = Vec::new();

  for day in last.saturating_sub(HISTORY_DAYS)..=last {
    let spans = read_day(paths, day)?;
    history.extend(spans);
  }

  Ok(history)
}

fn append(
  spans: &mut Vec<Span>,
  span: Span,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let previous = spans
      .last_mut()
      .filter(|last| last.activity == span.activity && last.end == span.start);

    match previous {
      Some(last) => last.end = span.end,
      None => spans.push(span),
    }
  };

  Ok(())
}

struct History {
  days: BTreeMap<u64, Vec<Span>>,
}

impl History {
  fn load(paths: &Paths, now: u64) -> Result<Self> {
    let last = now.wrapping_div(DAY);
    let mut days = BTreeMap::new();

    for day in last.saturating_sub(HISTORY_DAYS)..=last {
      let spans = read_day(paths, day)?;

      match bool::then_some(!spans.is_empty(), ()) {
        Some(()) => {
          days.insert(day, spans);
        }
        None => {}
      }
    }

    Ok(Self { days })
  }

  fn keep(&mut self, paths: &Paths, span: Span) -> Result<()> {
    let days = bool::then(span.start < span.end, || {
      span.start.wrapping_div(DAY)..=span.end.saturating_sub(1).wrapping_div(DAY)
    });

    for day in days.into_iter().flatten() {
      let start = span.start.max(day.saturating_mul(DAY));
      let end = span.end.min(day.saturating_add(1).saturating_mul(DAY));
      let values = self.days.entry(day).or_default();
      let from = match values.last() {
        Some(last) => start.max(last.end),
        None => start,
      };

      match bool::then_some(from < end, ()) {
        Some(()) => {
          let Ok(()) = append(
            values,
            Span {
              start: from,
              end,
              activity: span.activity.clone(),
            },
          );
          let data = serde_json::to_vec(values)?;
          whole(
            &({
              let Ok(explicit_value) = day_path(paths, day);
              explicit_value
            }),
            &data,
          )?;
        }
        None => {}
      }
    }

    let earliest = span.end.wrapping_div(DAY);
    self
      .days
      .retain(|day, _| *day >= earliest.saturating_sub(HISTORY_DAYS));
    Ok(())
  }
}

#[derive(Clone)]
struct Sample {
  wall: u64,
  monotonic: Instant,
  activity: Option<Activity>,
}

#[must_use]
fn between(
  previous: Sample,
  current: &Sample,
  interval: u64,
) -> core::result::Result<Option<Span>, console_core_never::Never> {
  let elapsed = match current.wall.checked_sub(previous.wall) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let monotonic = (match current.monotonic.checked_duration_since(previous.monotonic) {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  })
  .as_secs();

  match bool::then_some(
    elapsed == 0 || elapsed > u64::wrapping_add(interval, 5) || elapsed.abs_diff(monotonic) > 2,
    (),
  ) {
    Some(()) => {
      return Ok(None);
    }
    None => {}
  }

  (match current.activity.as_ref() {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  });

  Ok(Some(Span {
    start: previous.wall,
    end: current.wall,
    activity: {
      let checked_value = match previous.activity {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };
      checked_value
    },
  }))
}

pub fn record(paths: &Paths) -> Result<()> {
  private_directory(&paths.runtime)?;
  private_directory(&paths.state.join("days"))?;
  let lock = open(
    paths.runtime.join("recorder.lock"),
    OFlags::CREATE | OFlags::WRONLY | OFlags::CLOEXEC,
    Mode::RUSR | Mode::WUSR,
  )?;

  match flock(&lock, FlockOperation::NonBlockingLockExclusive) {
    Ok(()) => {}
    Err(rustix::io::Errno::WOULDBLOCK) => return Ok(()),
    Err(fault) => return Err(Box::new(fault)),
  }

  crate::idle::with_monitor(|idle| record_samples(paths, idle))
}

struct Recording {
  sites: Sites,
  names: BTreeMap<String, String>,
  day: u64,
  history: History,
  previous: Option<Sample>,
  last_fault: Option<String>,
}

fn record_samples(paths: &Paths, idle: &Idle<'_>) -> Result<()> {
  let sites = Sites::bind(paths)?;
  let Ok(names) = app_names();
  let Ok(wall) = now();
  let history = History::load(paths, wall)?;
  let recording = Recording {
    sites,
    names,
    day: wall.saturating_div(DAY),
    history,
    previous: None,
    last_fault: None,
  };
  let ended = iterate(recording, |mut recording_2| {
    Ok(match recording_2.tick(paths, idle) {
      Ok(()) => Step::Again(recording_2),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })?;
  ended
}

impl Recording {
  fn tick(&mut self, paths: &Paths, idle: &Idle<'_>) -> Result<()> {
    let interval = paths.interval()?;
    let Ok(wall) = now();
    let day = wall.saturating_div(DAY);

    match bool::then_some(day != self.day, ()) {
      Some(()) => {
        let Ok(names) = app_names();
        self.names = names;
        self.day = day;
      }
      None => {}
    }

    let (activity, status) = match observe(&self.sites, idle, &self.names) {
      Ok(observation) => {
        self.last_fault = None;
        observation
      }
      Err(fault) => {
        let message = fault.to_string();

        match bool::then_some(self.last_fault.as_ref() != Some(&message), ()) {
          Some(()) => {
            console_core_logging::warn!("Foreground monitor: {message}");
          }
          None => {}
        }

        self.last_fault = Some(message);
        (None, "Waiting for Hyprland")
      }
    };
    let current = Sample {
      wall,
      monotonic: Instant::now(),
      activity,
    };
    let previous_span = self.previous.take().and_then(|previous| {
      let Ok(span) = between(previous, &current, interval);
      span
    });

    match previous_span {
      Some(span) => self.history.keep(paths, span)?,
      None => {}
    }

    let recorder = report::Recorder {
      updated: wall,
      interval,
      status: status.to_owned(),
    };
    report::publish(paths, &recorder)?;
    let started = current.monotonic;
    self.previous = Some(current);

    receive_before_next_sample(paths, &mut self.sites, started)
  }
}

fn receive_before_next_sample(paths: &Paths, sites: &mut Sites, started: Instant) -> Result<()> {
  let ended = iterate(sites, |sites_2| -> core::result::Result<_, Never> {
    let interval = match paths.interval() {
      Ok(interval) => Duration::from_secs(interval),
      Err(fault) => return Ok(Step::Halt(Err(fault))),
    };

    match started.elapsed().cmp(&interval) {
      std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => Ok(Step::Halt(Ok(()))),
      std::cmp::Ordering::Less => {
        Ok(match sites_2.wait_for_signal() {
          Ok(()) => Step::Again(sites_2),
          Err(fault) => Step::Halt(Err(fault)),
        })
      }
    }
  })?;
  ended
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_switch_counts_the_previous_sample_and_sleep_leaves_a_gap() -> Result<()> {
    let a = Activity {
      app: String::from("Editor"),
      site: None,
    };
    let b = Activity {
      app: String::from("Browser"),
      site: Some(String::from("example.org")),
    };
    let instant = Instant::now();
    let before = Sample {
      wall: 100,
      monotonic: instant,
      activity: Some(a.clone()),
    };
    let next = instant
      .checked_add(Duration::from_secs(60))
      .ok_or("test clock overflow")?;
    let after = Sample {
      wall: 160,
      monotonic: next,
      activity: Some(b),
    };
    assert_eq!(
      ({
        let Ok(explicit_value) = between(before.clone(), &after, 60);
        explicit_value
      }),
      Some(Span {
        start: 100,
        end: 160,
        activity: a
      })
    );
    let asleep = Sample {
      wall: 165,
      ..after.clone()
    };
    assert!(
      ({
        let Ok(explicit_value) = between(before.clone(), &asleep, 60);
        explicit_value
      })
      .is_none()
    );
    let idle = Sample {
      activity: None,
      ..after
    };
    assert!(
      ({
        let Ok(explicit_value) = between(before, &idle, 60);
        explicit_value
      })
      .is_none()
    );
    Ok(())
  }

  #[test]
  fn hidden_windows_are_not_observed_and_titles_are_not_needed() -> Result<()> {
    let properties = serde_json::from_str::<Properties>(
      r#"{
      "mapped": true, "hidden": false, "class": "foot", "pid": 10,
      "monitor": 0, "address": "0x34", "title": "terminal document title", "floating": false
    }"#,
    )?;
    let mut reply = FocusedReply::readings(properties)?;
    let Ok(found) = window(&reply);
    let observed = found.ok_or("visible window is missing")?;
    assert_eq!(observed.class, "foot");
    reply.hidden = Some(true);
    let Ok(hidden) = window(&reply);
    assert!(hidden.is_none());
    Ok(())
  }

  #[test]
  fn history_preserves_the_flat_record_and_checks_its_fields() -> Result<()> {
    let original = r#"{"start":100,"end":160,"app":"Browser","site":"example.org"}"#;
    let stored = serde_json::from_str::<StoredSpan>(original)?;
    let span = Span::from(stored);
    let encoded = serde_json::to_string(&span)?;
    assert_eq!(encoded, original);
    let unexpected = serde_json::from_str::<StoredSpan>(
      r#"{"start":100,"end":160,"app":"Browser","site":null,"extra":1}"#
    );

    match unexpected {
      Ok(_) => Err(Box::from("the history reader accepted an unknown field")),
      Err(fault) => {
        assert!(fault.is_data());
        Ok(())
      }
    }
  }
}
