//! One waiting, as a line, and the same line read back.
//!
//! Written by hand rather than derived, because the order of the fields is the
//! order the time went in and a map that sorted them would put `built` before
//! `gtk` and tell the story backwards. Escaping is still `serde_json`'s: the
//! words in a line are folder names and window titles, and one of them will
//! eventually have a quotation mark in it.
//!
//! `at` and `up` are seconds, and `load` is the one-minute load average.
//! The total wait and each named phase are milliseconds. Details such as the
//! row count, door, and folder go under `with`.

use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::fitted;
use serde::de::{Deserialize, Deserializer, Error, MapAccess, Visitor};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this type defines the seconds unit and keeps the journal's fractional representation"
  )
)]
pub struct UptimeSeconds(pub f64);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the one-minute load average is a dimensionless average count of runnable or blocked tasks"
  )
)]
pub struct LoadAverage(pub f64);

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
  Count(u64),
  Word(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
  pub at: u64,
  pub up: UptimeSeconds,
  pub load: LoadAverage,
  pub who: String,
  pub what: String,
  pub waited: Duration,
  pub marks: Vec<(String, Duration)>,
  pub notes: Vec<(String, Value)>,
}

pub fn milliseconds(took: Duration) -> Result<f64, Never> {
  Ok((took.as_secs_f64() * 10_000.0).round() / 10.0)
}

fn json_string(said: &str) -> Result<String, Never> {
  Ok(serde_json::Value::String(said.to_string()).to_string())
}

pub fn serialize(entry: &Entry) -> Result<String, Never> {
  let Ok(who) = json_string(&entry.who);
  let Ok(what) = json_string(&entry.what);
  let Ok(waited) = milliseconds(entry.waited);

  let mut said = format!(
    "{{\"at\":{},\"up\":{:.1},\"load\":{:.2},\"who\":{who},\"what\":{what},\"waited\":{waited:.1}",
    entry.at, entry.up.0, entry.load.0,
  );

  for (name, took) in &entry.marks {
    let Ok(name_2) = json_string(name);
    let Ok(took_2) = milliseconds(*took);

    said.push_str(&format!(",{name}:{took:.1}", took = took_2, name = name_2));
  }

  match ::core::primitive::bool::then_some(entry.notes.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      said.push_str(",\"with\":{");
      let mut first = true;

      for (name, note) in &entry.notes {
        match ::core::primitive::bool::then_some(first, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => said.push(','),
        }

        first = false;

        let value = match note {
          Value::Count(many) => many.to_string(),
          Value::Word(word) => {
            let Ok(word_2) = json_string(word);

            word_2
          }
        };
        let Ok(name_3) = json_string(name);

        said.push_str(&format!("{name}:{value}", name = name_3));
      }

      said.push('}');
    }
  }

  said.push('}');

  Ok(said)
}

#[derive(Default)]
struct TimedLine {
  at: Option<u64>,
  up: Option<serde_json::Value>,
  load: Option<serde_json::Value>,
  who: Option<String>,
  what: Option<String>,
  waited: Option<serde_json::Number>,
  notes: Option<serde_json::Value>,
  marks: serde_json::Map<String, serde_json::Value>,
}

struct JournalLine {
  bytes: u64,
}

impl<'de> Visitor<'de> for JournalLine {
  type Value = TimedLine;

  fn expecting(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    to.write_str("a journal line object")
  }

  fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<TimedLine, M::Error> {
    let mut line = TimedLine::default();

    for _field in 0..=self.bytes {
      let next = map.next_key::<String>()?;
      let name = match next {
        Some(name) => name,
        None => return Ok(line),
      };

      let read = match name.as_str() {
        "at" => read_field(&mut map, &mut line.at, "at"),
        "up" => read_field(&mut map, &mut line.up, "up"),
        "load" => read_field(&mut map, &mut line.load, "load"),
        "who" => read_field(&mut map, &mut line.who, "who"),
        "what" => read_field(&mut map, &mut line.what, "what"),
        "waited" => read_field(&mut map, &mut line.waited, "waited"),
        "with" => read_field(&mut map, &mut line.notes, "with"),
        _phase_name => {
          let elapsed = map.next_value()?;
          let _earlier_phase = line.marks.insert(name, elapsed);

          Ok(())
        }
      };
      read?;
    }

    Err(M::Error::custom(
      "journal fields exceed the byte length of the line",
    ))
  }
}

fn read_field<'de, T: Deserialize<'de>, M: MapAccess<'de>>(
  map: &mut M,
  field: &mut Option<T>,
  name: &'static str,
) -> Result<(), M::Error> {
  match field {
    Some(_) => Err(M::Error::duplicate_field(name)),
    None => {
      let value = map.next_value()?;
      *field = Some(value);

      Ok(())
    }
  }
}

fn decoded_line(said: &str) -> Result<TimedLine, serde_json::Error> {
  let mut decoder = serde_json::Deserializer::from_str(said);
  let Ok(bytes) = fitted::<_, u64>(said.len());
  let line = decoder.deserialize_map(JournalLine { bytes })?;

  decoder.end()?;

  Ok(line)
}

pub fn read(said: &str) -> Result<Option<Entry>, Never> {
  let logged = match decoded_line(said) {
    Ok(logged) => logged,
    Err(_unreadable) => return Ok(None),
  };
  let (at, who, what, duration) = match (logged.at, logged.who, logged.what, logged.waited) {
    (Some(at), Some(who), Some(what), Some(duration)) => (at, who, what, duration),
    (_, _, _, _) => return Ok(None),
  };
  let milliseconds = match duration.as_f64() {
    Some(milliseconds) => milliseconds,
    None => return Ok(None),
  };
  let waited = match Duration::try_from_secs_f64(milliseconds / 1000.0) {
    Ok(waited) => waited,
    Err(_out_of_range) => return Ok(None),
  };
  let marks = logged
    .marks
    .into_iter()
    .filter_map(|(name, value)| {
      let took = value.as_f64()?;

      match Duration::try_from_secs_f64(took / 1000.0) {
        Ok(elapsed) => Some((name, elapsed)),
        Err(_invalid_phase_duration) => None,
      }
    })
    .collect();
  let recorded_notes = match logged.notes {
    Some(serde_json::Value::Object(notes)) => Some(notes),
    Some(_) | None => None,
  };
  let notes = recorded_notes
    .into_iter()
    .flatten()
    .filter_map(|(name, value)| {
      let note = match value {
        serde_json::Value::Number(many) => {
          let count = many.as_u64()?;

          Value::Count(count)
        }
        serde_json::Value::String(word) => Value::Word(word),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => return None,
      };

      Some((name, note))
    })
    .collect();

  Ok(Some(Entry {
    at,
    up: match logged.up.as_ref().and_then(serde_json::Value::as_f64) {
      Some(up) => UptimeSeconds(up),
      None => UptimeSeconds(0.0),
    },
    load: match logged.load.as_ref().and_then(serde_json::Value::as_f64) {
      Some(load) => LoadAverage(load),
      None => LoadAverage(0.0),
    },
    who,
    what,
    waited,
    marks,
    notes,
  }))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  fn an_opening() -> Result<Entry, Never> {
    Ok(Entry {
      at: 1_756_761_123,
      up: UptimeSeconds(67_932.4),
      load: LoadAverage(0.31),
      who: "launcher".to_string(),
      what: "opening".to_string(),
      waited: Duration::from_millis(412),
      marks: vec![
        ("press".to_string(), Duration::from_micros(11_400)),
        ("gtk".to_string(), Duration::from_millis(128)),
      ],
      notes: vec![
        ("rows".to_string(), Value::Count(73)),
        ("door".to_string(), Value::Word("menu".to_string())),
      ],
    })
  }

  #[test]
  fn a_line_holds_the_wait_where_it_went_and_what_it_was_about() {
    let Ok(opening) = an_opening();
    let Ok(said) = serialize(&opening);

    assert_eq!(
      said,
      r#"{"at":1756761123,"up":67932.4,"load":0.31,"who":"launcher","what":"opening","waited":412.0,"press":11.4,"gtk":128.0,"with":{"rows":73,"door":"menu"}}"#
    );
  }

  #[test]
  fn a_name_with_a_quotation_mark_in_it_is_still_one_line_of_json() -> Result<(), Box<dyn Error>>
  {
    let Ok(mut entry) = an_opening();

    entry.notes = vec![(
      "folder".to_string(),
      Value::Word("she said \"go\"".to_string()),
    )];

    let Ok(said) = serialize(&entry);
    let held = decoded_line(&said)?;

    assert_eq!(
      held
        .notes
        .as_ref()
        .and_then(|notes| notes.get("folder"))
        .and_then(serde_json::Value::as_str),
      Some("she said \"go\"")
    );
    assert!(!said.contains('\n'), "a line is one line");

    Ok(())
  }

  #[test]
  fn the_stretches_stay_in_the_order_they_happened() {
    let Ok(mut entry) = an_opening();

    entry.marks = vec![
      ("gtk".to_string(), Duration::from_millis(1)),
      ("built".to_string(), Duration::from_millis(2)),
      ("frame".to_string(), Duration::from_millis(3)),
    ];
    let Ok(said) = serialize(&entry);

    let in_order = said
      .split_once("gtk")
      .and_then(|(_, rest)| rest.split_once("built"))
      .and_then(|(_, rest)| rest.split_once("frame"));
    assert!(in_order.is_some(), "{said}");
  }

  #[test]
  fn a_line_read_back_says_what_was_written() -> Result<(), Box<dyn Error>> {
    let Ok(entry) = an_opening();
    let Ok(said) = serialize(&entry);
    let Ok(recorded) = read(&said);
    let back = recorded.ok_or("a written line reads back")?;

    assert_eq!(back.who, entry.who);
    assert_eq!(back.what, entry.what);
    assert_eq!(milliseconds(back.waited), milliseconds(entry.waited));
    assert!(back.notes.contains(&("rows".to_string(), Value::Count(73))));
    assert!(
      back
        .notes
        .contains(&("door".to_string(), Value::Word("menu".to_string())))
    );

    let named: Vec<&str> = back.marks.iter().map(|(name, _)| name.as_str()).collect();
    assert!(named.contains(&"press") && named.contains(&"gtk"));

    Ok(())
  }

  #[test]
  fn what_the_line_is_about_is_never_read_as_a_stretch_of_the_wait() -> Result<(), Box<dyn Error>>
  {
    let Ok(opening) = an_opening();
    let Ok(said) = serialize(&opening);
    let Ok(back) = read(&said);
    let back_2 = back.ok_or("a line")?;

    let named: Vec<&str> = back_2.marks.iter().map(|(name, _)| name.as_str()).collect();
    assert!(!named.contains(&"up"));
    assert!(
      !named.contains(&"load"),
      "what else the machine was doing is not a stretch"
    );
    assert!(!named.contains(&"at"));
    assert!(!named.contains(&"waited"));
    assert!(!named.contains(&"rows"), "a count is not a stretch");

    Ok(())
  }

  #[test]
  fn a_half_written_line_is_stepped_over_rather_than_stopped_at() {
    assert_eq!(read(""), Ok(None));
    assert_eq!(
      read("{\"at\":1756761123,\"up\":1.0,\"who\":\"launc"),
      Ok(None)
    );
    assert_eq!(read("{\"who\":\"launcher\"}"), Ok(None));
  }

  #[test]
  fn valid_phases_survive_malformed_optional_values() {
    let Ok(entry) = read(
      r#"{"at":1,"up":null,"load":"busy","who":"launcher","what":"opening","waited":20,"screen":-1,"draw":5,"with":["old"]}"#,
    );

    assert_eq!(
      entry.map(|line| (line.up, line.load, line.marks, line.notes)),
      Some((
        UptimeSeconds(0.0),
        LoadAverage(0.0),
        vec![("draw".to_string(), Duration::from_millis(5))],
        Vec::new(),
      )),
    );
  }
}
