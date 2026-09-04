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

/// What a line can say about itself besides how long it took.
#[derive(Debug, Clone, PartialEq)]
pub enum Said {
    Count(u64),
    Word(String),
}

/// One thing that was waited for.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// When, in seconds since the epoch.
    pub at: u64,
    /// How long the machine had been up, so a cold opening is not read as a
    /// slow one.
    pub up: f64,
    /// What else the machine was doing, so an opening that happened while it
    /// was compiling itself is not read as what the menu is like.
    pub load: f64,
    /// The program.
    pub who: String,
    /// What it was that somebody waited for.
    pub what: String,
    /// The whole of the wait, which the stretches add up to.
    pub waited: Duration,
    /// Where it went, in the order it went.
    pub marks: Vec<(String, Duration)>,
    /// What the wait was about.
    pub notes: Vec<(String, Said)>,
}

/// Milliseconds, to a tenth.
///
/// A tenth of a millisecond is finer than anything here is measured to and
/// coarse enough that a line stays a line. Nanoseconds would be six digits of
/// noise per stretch, and a store people do not read is a store nobody keeps.
pub fn ms(took: Duration) -> f64 {
    (took.as_secs_f64() * 10_000.0).round() / 10.0
}

/// A word, quoted and escaped the way JSON wants it.
fn quoted(said: &str) -> String {
    serde_json::Value::String(said.to_string()).to_string()
}

/// The line.
pub fn written(entry: &Entry) -> String {
    let mut said = format!(
        "{{\"at\":{},\"up\":{:.1},\"load\":{:.2},\"who\":{},\"what\":{},\"waited\":{:.1}",
        entry.at,
        entry.up,
        entry.load,
        quoted(&entry.who),
        quoted(&entry.what),
        ms(entry.waited),
    );

    for (name, took) in &entry.marks {
        said.push_str(&format!(",{}:{:.1}", quoted(name), ms(*took)));
    }

    if !entry.notes.is_empty() {
        said.push_str(",\"with\":{");
        let mut first = true;

        for (name, note) in &entry.notes {
            if !first {
                said.push(',');
            }

            first = false;
            let value = match note {
                Said::Count(many) => many.to_string(),
                Said::Word(word) => quoted(word),
            };
            said.push_str(&format!("{}:{value}", quoted(name)));
        }

        said.push('}');
    }

    said.push('}');
    said
}

/// A line, read back.
///
/// Anything that is not a line is nothing rather than a failure. A store is
/// appended to by half a dozen programs and read by one, and the reader's job
/// is to say what the machine has been like -- not to stop at the first line
/// something wrote badly while it was being killed.
pub fn read(said: &str) -> Option<Entry> {
    let held: serde_json::Value = match serde_json::from_str(said) {
        Ok(v) => v,
        Err(_) => return None,
    };
    let object = held.as_object()?;
    let word = |name: &str| object.get(name)?.as_str().map(str::to_string);
    let number = |name: &str| object.get(name)?.as_f64();
    let at = match object.get("at")? {
        serde_json::Value::Number(n) => n.as_u64()?,
        _ => return None,
    };
    let up = number("up").unwrap_or(0.0);
    let load = number("load").unwrap_or(0.0);
    let entry = Entry {
        at,
        up,
        load,
        who: word("who")?,
        what: word("what")?,
        waited: Duration::from_secs_f64(number("waited")? / 1000.0),
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
                    serde_json::Value::Number(many) => Said::Count(many.as_u64()?),
                    serde_json::Value::String(word) => Said::Word(word.clone()),
                    _ => return None,
                };
                Some((name.clone(), note))
            })
            .collect(),
    };
    Some(entry)
}

/// The fields at the top of a line that are not stretches of the wait.
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

    /// The whole promise of the file: one line is one opening, and everything
    /// about it is on that line. Nothing to join, nothing to look up.
    #[test]
    fn a_line_holds_the_wait_where_it_went_and_what_it_was_about() {
        let said = written(&an_opening());
        assert_eq!(
            said,
            r#"{"at":1756761123,"up":67932.4,"load":0.31,"who":"launcher","what":"opening","waited":412.0,"press":11.4,"gtk":128.0,"with":{"rows":73,"door":"menu"}}"#
        );
    }

    /// Which is only worth anything if it is JSON, whatever the words in it
    /// were. A folder is named by whoever made it.
    #[test]
    fn a_name_with_a_quotation_mark_in_it_is_still_one_line_of_json() {
        let mut entry = an_opening();
        entry.notes = vec![("folder".to_string(), Said::Word("she said \"go\"".into()))];
        let held: serde_json::Value =
            serde_json::from_str(&written(&entry)).expect("a line is json");
        assert_eq!(held["with"]["folder"], "she said \"go\"");
        assert!(!written(&entry).contains('\n'), "a line is one line");
    }

    /// The order of the stretches is the order the time went in. A map would
    /// sort them and tell it backwards.
    #[test]
    fn the_stretches_stay_in_the_order_they_happened() {
        let mut entry = an_opening();
        entry.marks = vec![
            ("gtk".to_string(), Duration::from_millis(1)),
            ("built".to_string(), Duration::from_millis(2)),
            ("frame".to_string(), Duration::from_millis(3)),
        ];
        let said = written(&entry);
        let gtk = said.find("gtk").expect("gtk");
        let built = said.find("built").expect("built");
        let frame = said.find("frame").expect("frame");
        assert!(gtk < built && built < frame);
    }

    /// Written and read back is the same waiting, because the reader is what
    /// every question about the store goes through.
    #[test]
    fn a_line_read_back_says_what_was_written() {
        let entry = an_opening();
        let back = read(&written(&entry)).expect("a written line reads back");
        assert_eq!(back.who, entry.who);
        assert_eq!(back.what, entry.what);
        assert_eq!(ms(back.waited), ms(entry.waited));
        // Read back off a map, so what is asserted is that both notes came
        // through and said what they said, not the order they came out in: the
        // order of the stretches is the story and the order of these is not.
        assert!(back.notes.contains(&("rows".to_string(), Said::Count(73))));
        assert!(back.notes.contains(&("door".to_string(), Said::Word("menu".to_string()))));
        let named: Vec<&str> = back.marks.iter().map(|(name, _)| name.as_str()).collect();
        assert!(named.contains(&"press") && named.contains(&"gtk"));
    }

    /// Every number at the top of a line is a stretch, and nothing else is. A
    /// reader that took `up` for one would report the machine's whole session
    /// as part of the wait.
    #[test]
    fn what_the_line_is_about_is_never_read_as_a_stretch_of_the_wait() {
        let back = read(&written(&an_opening())).expect("a line");
        let named: Vec<&str> = back.marks.iter().map(|(name, _)| name.as_str()).collect();
        assert!(!named.contains(&"up"));
        assert!(!named.contains(&"load"), "what else the machine was doing is not a stretch");
        assert!(!named.contains(&"at"));
        assert!(!named.contains(&"waited"));
        assert!(!named.contains(&"rows"), "a count is not a stretch");
    }

    /// A store is appended to by half a dozen programs, one of which may be
    /// being killed as it writes. The reader steps over what it cannot read.
    #[test]
    fn a_half_written_line_is_stepped_over_rather_than_stopped_at() {
        assert_eq!(read(""), None);
        assert_eq!(read("{\"at\":1756761123,\"up\":1.0,\"who\":\"launc"), None);
        assert_eq!(read("{\"who\":\"launcher\"}"), None);
    }
}
