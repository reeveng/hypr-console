//! Recorder status and Markdown reports understood by Console Notes.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::ops::Range;

use console_core_atomic_writes::whole;
use serde::{Deserialize, Serialize};

use crate::activity::{Span, read_history};
use crate::{Paths, Result, now, stored};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Recorder {
  pub updated: u64,
  pub interval: u64,
  pub status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusRecord {
  updated: u64,
  interval: u64,
  status: stored::Text<1024>,
}

impl Recorder {
  pub fn read(paths: &Paths) -> Result<Option<Self>> {
    let record = stored::read::<StatusRecord>(&paths.runtime.join("status.json"), 4096)?;
    Ok(record.map(|stored| Self {
      updated: stored.updated,
      interval: stored.interval,
      status: stored.status.0,
    }))
  }
}

pub fn publish(paths: &Paths, recorder: &Recorder) -> Result<()> {
  let bytes = serde_json::to_vec(recorder)?;
  whole(&paths.runtime.join("status.json"), &bytes)?;
  Ok(())
}

#[must_use]
fn label(text: &str) -> core::result::Result<String, console_core_never::Never> {
  Ok(
    text
      .chars()
      .filter(|character| !character.is_control())
      .map(|character| match character {
        ':' | ',' | ';' | '%' | '`' | '<' | '>' | '#' => ' ',
        other => other,
      })
      .take(96)
      .collect(),
  )
}

fn mermaid(spans: &[Span], range: Range<u64>) -> Result<String> {
  let mut lanes = BTreeMap::<String, Vec<(u64, u64)>>::new();

  for span in spans {
    let start = span.start.max(range.start);
    let end = span.end.min(range.end);

    match bool::then_some(start >= end, ()) {
      Some(()) => {
        continue;
      }
      None => {}
    }

    'over_name_2: for name in [
      Some(span.activity.app.as_str()),
      span.activity.site.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
      let lane = lanes
        .entry({
          let Ok(explicit_value) = label(name);
          explicit_value
        })
        .or_default();

      match lane.last_mut().filter(|previous| previous.1 == start) {
        Some(previous) => {
          previous.1 = end;
          continue 'over_name_2;
        }
        None => {}
      }

      lane.push((start, end));
    }
  }

  let mut document =
    String::from("# Habits\n\nSampled foreground activity. Blank time is unmeasured.\n\n");

  match bool::then_some(lanes.is_empty(), ()) {
    Some(()) => {
      document.push_str("No foreground activity in this range.\n");
      return Ok(document);
    }
    None => {}
  }

  document.push_str("```mermaid\ngantt\n    title Foreground activity\n    dateFormat X\n    axisFormat %b %d %H:%M\n    todayMarker off\n");
  let mut id = 0;

  for (name, periods) in lanes {
    writeln!(document, "    section {name}")?;

    for (start, end) in periods {
      id = i32::wrapping_add(id, 1);
      writeln!(
        document,
        "    Focus :t{id}, {start}, {}s",
        u64::wrapping_sub(end, start)
      )?;
    }
  }

  document.push_str("```\n");

  match bool::then_some(document.len() > 45_000, ()) {
    Some(()) => {
      return Err(Box::from(
        "the diagram is too large for Notes. Export a shorter range",
      ));
    }
    None => {}
  }

  Ok(document)
}

pub fn export(paths: &Paths, hours: u64) -> Result<()> {
  match bool::then_some(!(1..=u64::wrapping_mul(24, 30)).contains(&hours), ()) {
    Some(()) => {
      return Err(Box::from("choose between 1 and 720 hours"));
    }
    None => {}
  }

  let Ok(to) = now();
  let from = to.saturating_sub(u64::wrapping_mul(hours, 3600));
  let history = read_history(paths, to)?;
  let document = mermaid(&history, from..to)?;
  print!("{document}");
  Ok(())
}
