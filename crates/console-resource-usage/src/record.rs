//! JSONL records have a one-mebibyte limit, excluding the line ending.
//! Older records can leave out optional readings. A malformed optional
//! number or amount remains absent, and a malformed line is skipped.

use std::cmp::Ordering;
use std::io::{BufRead, Read};

use console_core_never::Never;
use console_core_number_conversion::fitted;
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::Value;

use crate::{Moment, Seconds, WattHours};

pub(super) const MAX_RECORD_BYTES: u64 = 1_048_576;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoggedMoment {
  at: u64,
  #[serde(default)]
  up: OptionalNumber<f64>,
  #[serde(default)]
  slept: OptionalNumber<f64>,
  #[serde(default)]
  energy: OptionalNumber<f64>,
  #[serde(default)]
  draw: OptionalNumber<f64>,
  #[serde(default)]
  gpu: OptionalNumber<f64>,
  #[serde(default)]
  percent: OptionalNumber<i32>,
  #[serde(default)]
  busy: LoggedAmounts,
  #[serde(default)]
  held: LoggedAmounts,
  #[serde(default)]
  restarts: LoggedAmounts,
  #[serde(default)]
  crashes: LoggedAmounts,
}

#[derive(Default)]
struct OptionalNumber<T>(Option<T>);

impl<'de, T: DeserializeOwned> Deserialize<'de> for OptionalNumber<T> {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    let value = Value::deserialize(decoder)?;

    Ok(Self(match serde_json::from_value(value) {
      Ok(number) => Some(number),
      Err(_not_the_recorded_number_type) => None,
    }))
  }
}

#[derive(Default)]
struct LoggedAmounts(Vec<(String, f64)>);

impl<'de> Deserialize<'de> for LoggedAmounts {
  fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
    let value = Value::deserialize(decoder)?;

    Ok(Self(match value {
      Value::Object(values) => values
        .into_iter()
        .filter_map(|(name, amount)| amount.as_f64().map(|number| (name, number)))
        .collect(),
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
        Vec::new()
      }
    }))
  }
}

pub fn of(text: &str) -> Result<Option<Moment>, Never> {
  let Ok(bytes) = fitted::<_, u64>(text.len());

  match bytes.cmp(&MAX_RECORD_BYTES) {
    Ordering::Greater => return Ok(None),
    Ordering::Equal | Ordering::Less => {}
  }

  let logged = match serde_json::from_str::<LoggedMoment>(text) {
    Ok(logged) => logged,
    Err(_unreadable) => return Ok(None),
  };

  Ok(Some(Moment {
    at: logged.at,
    up: logged.up.0.map(Seconds),
    slept: Seconds(match logged.slept.0 {
      Some(seconds) => seconds,
      None => 0.0,
    }),
    energy: logged.energy.0.map(WattHours),
    draw: logged.draw.0,
    gpu: logged.gpu.0,
    percent: logged.percent.0,
    busy: logged.busy.0,
    held: logged.held.0,
    restarts: logged.restarts.0,
    crashes: logged.crashes.0,
  }))
}

pub fn read_moments(reader: impl BufRead) -> Result<Vec<Moment>, Never> {
  let records = LogRecords {
    reader,
    line: Vec::new(),
  };

  Ok(records.flatten().collect())
}

struct LogRecords<R> {
  reader: R,
  line: Vec<u8>,
}

impl<R: BufRead> Iterator for LogRecords<R> {
  type Item = Option<Moment>;

  fn next(&mut self) -> Option<Self::Item> {
    self.line.clear();
    let mut bounded = self
      .reader
      .by_ref()
      .take(MAX_RECORD_BYTES.saturating_add(2));

    match bounded.read_until(b'\n', &mut self.line) {
      Ok(0) => return None,
      Err(_log_could_not_be_read) => return None,
      Ok(_bytes) => {}
    }

    let Ok(bytes) = fitted::<_, u64>(self.line.len());

    match (bytes.cmp(&MAX_RECORD_BYTES), self.line.last()) {
      (Ordering::Greater, Some(b'\n')) | (Ordering::Equal | Ordering::Less, _) => {}
      (Ordering::Greater, Some(_) | None) => {
        return match self.reader.skip_until(b'\n') {
          Ok(_skipped) => Some(None),
          Err(_unreadable) => None,
        };
      }
    }

    let text = match std::str::from_utf8(&self.line) {
      Ok(text) => text,
      Err(_not_text) => return Some(None),
    };
    let Ok(moment) = of(text.trim_end_matches(['\r', '\n']));

    Some(moment)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn older_records_keep_valid_readings_and_omit_unavailable_values()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(parsed) = of(r#"{"at":7,"up":32.4,"draw":"unknown","busy":{"a":3.2,"b":"unknown"}}"#);
    let logged = parsed.ok_or("the old record should still be readable")?;

    assert_eq!(logged.at, 7);
    assert_eq!(logged.up, Some(Seconds(32.4)));
    assert_eq!(logged.slept, Seconds(0.0));
    assert_eq!(logged.draw, None);
    assert_eq!(logged.busy, [("a".to_string(), 3.2)]);
    assert!(logged.held.is_empty());

    Ok(())
  }

  #[test]
  fn reading_recovers_at_the_next_line_after_oversized_or_unknown_records() {
    let mut lines = br#"{"at":1}"#.to_vec();
    lines.resize(1_048_576, b' ');
    lines.extend_from_slice(b"\r\n");

    let mut oversized = br#"{"at":2}"#.to_vec();
    oversized.resize(1_048_580, b' ');
    lines.extend(oversized);
    lines.extend_from_slice(b"\n{\"at\":3,\"unrecognized\":0}\n{\"at\":4}\r\n");

    let Ok(logged) = read_moments(lines.as_slice());
    let timestamps: Vec<_> = logged.into_iter().map(|moment| moment.at).collect();

    assert_eq!(timestamps, [1, 4]);
  }
}
