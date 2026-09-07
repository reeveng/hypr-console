//! One waiting, as a line, and the same line read back.
//!
//! Written by hand rather than derived, because the order of the fields is the
//! order the time went in and a map that sorted them would put `built` before
//! `gtk` and tell the story backwards. Escaping is still `serde_json`'s: the
//! words in a line are folder names and window titles, and one of them will
//! eventually have a quotation mark in it.
//!
//! The shape is flat where it is numbers and nested where it is not. Every
//! number at the top of a line is milliseconds, so anything reading this can
//! add up, sort and chart the whole of it without being told which fields mean
//! time; anything that says what the line was *about* -- how many rows, which
//! door, which folder -- is under `with`, where it cannot be mistaken for a
//! stretch of the wait.

use std::time::Duration;

use console_core_never::Never;

#[derive(Debug, Clone, PartialEq)]
pub enum Said {
    Count(u64),
    Word(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub at: u64,
    pub up: f64,
    pub load: f64,
    pub who: String,
    pub what: String,
    pub waited: Duration,
    pub marks: Vec<(String, Duration)>,
    pub notes: Vec<(String, Said)>,
}

pub fn ms(took: Duration) -> Result<f64, Never> {
    Ok((took.as_secs_f64() * 10_000.0).round() / 10.0)
}

fn quoted(said: &str) -> Result<String, Never> {
    Ok(serde_json::Value::String(said.to_string()).to_string())
}

pub fn written(entry: &Entry) -> Result<String, Never> {
    let Ok(who) = quoted(&entry.who);
    let Ok(what) = quoted(&entry.what);
    let Ok(waited) = ms(entry.waited);

    let mut said = format!(
        "{{\"at\":{},\"up\":{:.1},\"load\":{:.2},\"who\":{who},\"what\":{what},\"waited\":{waited:.1}",
        entry.at, entry.up, entry.load,
    );

    for (name, took) in &entry.marks {
        let Ok(name) = quoted(name);
        let Ok(took) = ms(*took);

        said.push_str(&format!(",{name}:{took:.1}"));
    }

    match entry.notes.is_empty() {
        true => {}
        false => {
            said.push_str(",\"with\":{");
            let mut first = true;

            for (name, note) in &entry.notes {
                match first {
                    true => {}
                    false => said.push(','),
                }

                first = false;

                let value = match note {
                    Said::Count(many) => many.to_string(),
                    Said::Word(word) => {
                        let Ok(word) = quoted(word);

                        word
                    }
                };
                let Ok(name) = quoted(name);

                said.push_str(&format!("{name}:{value}"));
            }

            said.push('}');
        }
    }

    said.push('}');

    Ok(said)
}

pub fn read(said: &str) -> Result<Option<Entry>, Never> {
    let held: serde_json::Value = match serde_json::from_str(said) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let Some(object) = held.as_object() else { return Ok(None) };

    let word = |name: &str| {
        let held = object.get(name)?;

        held.as_str().map(str::to_string)
    };
    let number = |name: &str| {
        let held = object.get(name)?;

        held.as_f64()
    };

    let Some(when) = object.get("at") else { return Ok(None) };

    let at = match when {
        serde_json::Value::Number(n) => match n.as_u64() {
            Some(at) => at,
            None => return Ok(None),
        },
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => return Ok(None),
    };
    let up = number("up").unwrap_or(0.0);
    let load = number("load").unwrap_or(0.0);

    let Some(who) = word("who") else { return Ok(None) };

    let Some(what) = word("what") else { return Ok(None) };

    let Some(waited) = number("waited") else { return Ok(None) };

    let waited = Duration::from_secs_f64(waited / 1000.0);
    let entry = Entry {
        at,
        up,
        load,
        who,
        what,
        waited,
        marks: object
            .iter()
            .filter(|(name, _)| !HEADS.contains(&name.as_str()))
            .filter_map(|(name, value)| {
                let took = value.as_f64()?;
                Some((name.clone(), Duration::from_secs_f64(took / 1000.0)))
            })
            .collect(),
        notes: object
            .get("with")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, value)| {
                let note = match value {
                    serde_json::Value::Number(many) => {
                        let many = many.as_u64()?;

                        Said::Count(many)
                    }
                    serde_json::Value::String(word) => Said::Word(word.clone()),
                    serde_json::Value::Null
                    | serde_json::Value::Bool(_)
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_) => return None,
                };
                Some((name.clone(), note))
            })
            .collect(),
    };

    Ok(Some(entry))
}

const HEADS: [&str; 7] = ["at", "up", "load", "who", "what", "waited", "with"];

#[cfg(test)]
mod tests {
    use super::*;

    fn an_opening() -> Entry {
        Entry {
            at: 1_756_761_123,
            up: 67_932.4,
            load: 0.31,
            who: "launcher".to_string(),
            what: "opening".to_string(),
            waited: Duration::from_millis(412),
            marks: vec![
                ("press".to_string(), Duration::from_micros(11_400)),
                ("gtk".to_string(), Duration::from_millis(128)),
            ],
            notes: vec![
                ("rows".to_string(), Said::Count(73)),
                ("door".to_string(), Said::Word("menu".to_string())),
            ],
        }
    }

    #[test]
    fn a_line_holds_the_wait_where_it_went_and_what_it_was_about() {
        let Ok(said) = written(&an_opening());

        assert_eq!(
            said,
            r#"{"at":1756761123,"up":67932.4,"load":0.31,"who":"launcher","what":"opening","waited":412.0,"press":11.4,"gtk":128.0,"with":{"rows":73,"door":"menu"}}"#
        );
    }

    #[test]
    fn a_name_with_a_quotation_mark_in_it_is_still_one_line_of_json() {
        let mut entry = an_opening();
        entry.notes = vec![("folder".to_string(), Said::Word("she said \"go\"".into()))];

        let Ok(said) = written(&entry);

        let held: serde_json::Value = serde_json::from_str(&said).expect("a line is json");
        assert_eq!(held["with"]["folder"], "she said \"go\"");
        assert!(!said.contains('\n'), "a line is one line");
    }

    #[test]
    fn the_stretches_stay_in_the_order_they_happened() {
        let mut entry = an_opening();
        entry.marks = vec![
            ("gtk".to_string(), Duration::from_millis(1)),
            ("built".to_string(), Duration::from_millis(2)),
            ("frame".to_string(), Duration::from_millis(3)),
        ];
        let Ok(said) = written(&entry);

        let gtk = said.find("gtk").expect("gtk");
        let built = said.find("built").expect("built");
        let frame = said.find("frame").expect("frame");
        assert!(gtk < built && built < frame);
    }

    #[test]
    fn a_line_read_back_says_what_was_written() {
        let entry = an_opening();

        let Ok(said) = written(&entry);
        let Ok(Some(back)) = read(&said) else { panic!("a written line reads back") };

        assert_eq!(back.who, entry.who);
        assert_eq!(back.what, entry.what);
        assert_eq!(ms(back.waited), ms(entry.waited));
        assert!(back.notes.contains(&("rows".to_string(), Said::Count(73))));
        assert!(back.notes.contains(&("door".to_string(), Said::Word("menu".to_string()))));
        let named: Vec<&str> = back.marks.iter().map(|(name, _)| name.as_str()).collect();
        assert!(named.contains(&"press") && named.contains(&"gtk"));
    }

    #[test]
    fn what_the_line_is_about_is_never_read_as_a_stretch_of_the_wait() {
        let Ok(said) = written(&an_opening());
        let Ok(Some(back)) = read(&said) else { panic!("a line") };

        let named: Vec<&str> = back.marks.iter().map(|(name, _)| name.as_str()).collect();
        assert!(!named.contains(&"up"));
        assert!(!named.contains(&"load"), "what else the machine was doing is not a stretch");
        assert!(!named.contains(&"at"));
        assert!(!named.contains(&"waited"));
        assert!(!named.contains(&"rows"), "a count is not a stretch");
    }

    #[test]
    fn a_half_written_line_is_stepped_over_rather_than_stopped_at() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read("{\"at\":1756761123,\"up\":1.0,\"who\":\"launc"), Ok(None));
        assert_eq!(read("{\"who\":\"launcher\"}"), Ok(None));
    }
}
