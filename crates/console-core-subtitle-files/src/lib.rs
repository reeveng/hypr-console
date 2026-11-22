//! A subtitle file, read into what it says and when.
//!
//! libass, inside ffmpeg, drew a film's subtitles into its frames, in the
//! fonts and places the file asked for. The frames come out of this desktop's
//! own decoders now, and the words go over them the way every other word on a
//! card does: in the card's own letters, at the foot of the picture. So what is
//! wanted out of a subtitle file is the lines and the moments between which
//! they show, and nothing of how they were styled -- a colour, a font, a place
//! on the screen and a karaoke sweep are all dropped, and every line keeps its
//! words and where it breaks.
//!
//! Three ways of writing them are what a film arrives with. SubRip is a
//! numbered block of two moments and some lines, WebVTT is the same block
//! with a header and its own markup, and ASS -- SSA before it -- is a table
//! whose `Format` line says which column is which, with the words last. A
//! Matroska film carries its tracks in the same three, one line to a block,
//! with the moments taken off and kept by the container, so [`said`] reads one
//! block's words without them.
//!
//! A file is read the way `console-core-text-encodings` reads one: as UTF-16
//! behind the byte-order mark Notepad writes when it saves as Unicode, as UTF-8
//! when it is UTF-8, and otherwise as windows-1252, which is what a subtitle
//! file written before UTF-8 nearly always is. Latin-1 was read here before,
//! and it agrees on every accented letter but turns the curly quotes a program
//! on Windows wrote into control characters that show nothing. A block out of a
//! film has no mark to look for and is read the same way without one, where
//! `String::from_utf8_lossy` had turned each accent into a question mark.

use std::path::Path;

use console_core_finite_numbers::FiniteF64;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_text_encodings::{decode, decode_file};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
  SubRip,
  WebVtt,
  Ass,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
  pub from: Seconds,
  pub until: Seconds,
  pub lines: Vec<String>,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this is the scalar representation of the Seconds type used for cue times and playback positions"
  )
)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inside {
  Yes,
  No,
}

const STANDARD_EVENTS: [&str; 10] = [
  "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv", "effect", "text",
];

const FIELDS_IN_A_BLOCK: u32 = 9;

impl Written {
  pub fn of(at: &Path) -> Result<Option<Written>, Never> {
    let ending = at
      .extension()
      .and_then(std::ffi::OsStr::to_str)
      .map(str::to_ascii_lowercase);

    Ok(match ending.as_deref() {
      Some("srt") => Some(Written::SubRip),
      Some("vtt") => Some(Written::WebVtt),
      Some("ass" | "ssa") => Some(Written::Ass),
      Some(_another) => None,
      None => None,
    })
  }
}

pub fn read(bytes: &[u8], written: Written) -> Result<Vec<Cue>, Never> {
  let Ok(decoded) = decode_file(bytes);
  let text = decoded.text.replace("\r\n", "\n").replace('\r', "\n");

  match written {
    Written::SubRip | Written::WebVtt => timed_blocks(&text, written),
    Written::Ass => events(&text),
  }
}

pub fn said(block: &[u8], written: Written) -> Result<Vec<String>, Never> {
  let Ok(decoded) = decode(block);
  let block_2 = decoded.text.as_ref();

  match written {
    Written::SubRip | Written::WebVtt => words(block_2, written),
    Written::Ass => {
      let Ok(fields) = index(FIELDS_IN_A_BLOCK);

      match block_2.splitn(fields, ',').last() {
        Some(text) => words(text, written),
        None => Ok(Vec::new()),
      }
    }
  }
}

pub fn showing(cues: &[Cue], at: Seconds) -> Result<Vec<String>, Never> {
  Ok(
    cues
      .iter()
      .filter(|cue| cue.from <= at && at < cue.until)
      .flat_map(|cue| cue.lines.iter().cloned())
      .collect(),
  )
}

fn timed_blocks(text: &str, written: Written) -> Result<Vec<Cue>, Never> {
  let lines: Vec<&str> = text.lines().collect();
  let mut cues = Vec::new();

  for block in lines.split(|line| line.trim().is_empty()) {
    let Ok(cue) = timed_block(block, written);

    cues.extend(cue);
  }

  Ok(cues)
}

fn timed_block(block: &[&str], written: Written) -> Result<Option<Cue>, Never> {
  let mut after = block.iter().skip_while(|line| !line.contains("-->"));

  let timing = match after.next() {
    Some(timing) => timing,
    None => return Ok(None),
  };

  let (from, until) = match timing.split_once("-->") {
    Some((from, rest)) => (from, rest.split_whitespace().next()),
    None => return Ok(None),
  };

  let Ok(from_2) = moment(from);
  let Ok(until_2) = until.map_or(Ok(None), moment);
  let text: Vec<&str> = after.copied().collect();
  let Ok(lines) = words(&text.join("\n"), written);

  Ok(match (from_2, until_2) {
    (Some(from_3), Some(until_3)) => Some(Cue {
      from: from_3,
      until: until_3,
      lines,
    }),
    (None, _) | (_, None) => None,
  })
}

fn events(text: &str) -> Result<Vec<Cue>, Never> {
  let listed = match text.split_once("[Events]") {
    Some((_, after)) => after,
    None => return Ok(Vec::new()),
  };

  let listed_3 = match listed.split_once("\n[") {
    Some((listed_2, _the_next_section)) => listed_2,
    None => listed,
  };

  let named = listed_3
    .lines()
    .find_map(|line| line.trim().strip_prefix("Format:"));

  let names: Vec<String> = match named {
    Some(named_2) => named_2
      .split(',')
      .map(|name| name.trim().to_lowercase())
      .collect(),
    None => STANDARD_EVENTS
      .iter()
      .map(|name| (*name).to_string())
      .collect(),
  };

  let mut cues = Vec::new();

  for line in listed_3.lines() {
    let Ok(cue) = dialogue(line.trim(), &names);

    cues.extend(cue);
  }

  Ok(cues)
}

fn dialogue(line: &str, names: &[String]) -> Result<Option<Cue>, Never> {
  let fields = match line.strip_prefix("Dialogue:") {
    Some(fields) => fields,
    None => return Ok(None),
  };

  let (mut from, mut until, mut text) = (None, None, None);

  for (name, field) in names.iter().zip(fields.splitn(names.len(), ',')) {
    match name.as_str() {
      "start" => from = Some(field),
      "end" => until = Some(field),
      "text" => text = Some(field),
      _another_column => {}
    }
  }

  let Ok(from_2) = from.map_or(Ok(None), moment);
  let Ok(until_2) = until.map_or(Ok(None), moment);

  Ok(match (from_2, until_2, text) {
    (Some(from_3), Some(until_3), Some(text_2)) => {
      let Ok(lines) = words(text_2, Written::Ass);

      Some(Cue {
        from: from_3,
        until: until_3,
        lines,
      })
    }
    (None, _, _) | (_, None, _) | (_, _, None) => None,
  })
}

fn moment(said: &str) -> Result<Option<Seconds>, Never> {
  let said_2 = said.trim().replace(',', ".");
  let mut parts = said_2.rsplit(':').map(str::parse::<FiniteF64>);

  Ok(
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
      (Some(Ok(seconds)), Some(Ok(minutes)), None, None) => {
        Some(Seconds(f64::from(minutes) * 60.0 + f64::from(seconds)))
      }
      (Some(Ok(seconds)), Some(Ok(minutes)), Some(Ok(hours)), None) => Some(Seconds(
        f64::from(hours) * 3600.0 + f64::from(minutes) * 60.0 + f64::from(seconds),
      )),
      (_, _, _, _) => None,
    },
  )
}

fn words(text: &str, written: Written) -> Result<Vec<String>, Never> {
  let plain = match written {
    Written::SubRip | Written::WebVtt => {
      let Ok(untagged) = without(text, ('<', '>'));
      let Ok(unstyled) = without(&untagged, ('{', '}'));

      unstyled
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&lrm;", "")
        .replace("&rlm;", "")
        .replace("&amp;", "&")
    }
    Written::Ass => {
      let Ok(unstyled) = without(text, ('{', '}'));

      unstyled
        .replace("\\N", "\n")
        .replace("\\n", " ")
        .replace("\\h", " ")
    }
  };

  Ok(
    plain
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .map(str::to_string)
      .collect(),
  )
}

fn without(text: &str, marks: (char, char)) -> Result<String, Never> {
  let (open, close) = marks;

  let (kept, _) = text
    .chars()
    .fold((String::new(), Inside::No), |(mut kept, inside), letter| {
      let inside_2 = match (inside, letter == open, letter == close) {
        (Inside::No, true, _) => Inside::Yes,
        (Inside::No, false, _) => {
          kept.push(letter);

          Inside::No
        }
        (Inside::Yes, _, true) => Inside::No,
        (Inside::Yes, _, false) => Inside::Yes,
      };

      (kept, inside_2)
    });

  Ok(kept)
}
